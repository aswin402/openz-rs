pub mod app;
pub mod theme;
pub mod ui;

use anyhow::Result;
use app::{ChatMessage, IS_RATATUI_ACTIVE, ModalState, RatatuiApp};
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::stdout;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

pub enum TurnEvent {
    SyncSession(Vec<ChatMessage>),
    SingleMessage(ChatMessage),
    Error(String),
}

// ── Scroll / render tuning ──────────────────────────────────────────────────
const TICK_MS: u64 = 50;
/// Mouse wheel and modifier-arrow scroll amount
const SCROLL_STEP: u32 = 3;
/// Plain Up/Down when input is empty
const SCROLL_JUMP: u32 = 2;
/// Ctrl+P / Ctrl+N line scroll
const SCROLL_STEP_EM: u32 = 4;
/// PageUp / PageDown
const SCROLL_PAGE: u32 = 8;
/// Ctrl+U / Ctrl+D half-page jump
const SCROLL_HALF: u32 = 10;

fn tui_marker_dir() -> std::path::PathBuf {
    crate::config::loader::runtime_data_dir().join("tui_instances")
}

fn tui_marker_path_in_dir(dir: &std::path::Path, pid: u32) -> std::path::PathBuf {
    dir.join(format!("{pid}.json"))
}

fn write_tui_marker_in_dir(
    dir: &std::path::Path,
    pid: u32,
    session_key: &str,
    model: &str,
    provider: &str,
) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let payload = serde_json::json!({
        "pid": pid,
        "session_key": session_key,
        "model": model,
        "provider": provider,
        "updated_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        tui_marker_path_in_dir(dir, pid),
        serde_json::to_string_pretty(&payload)?,
    )?;
    Ok(())
}

fn remove_tui_marker_in_dir(dir: &std::path::Path, pid: u32) {
    let _ = std::fs::remove_file(tui_marker_path_in_dir(dir, pid));
}

fn process_is_alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        if pid > i32::MAX as u32 {
            return false;
        }
        let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
        return result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn is_last_live_tui_in_dir(dir: &std::path::Path, current_pid: u32) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return true;
    };

    let mut found_other_live = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Ok(pid) = stem.parse::<u32>() else {
            continue;
        };
        if pid == current_pid {
            continue;
        }
        if process_is_alive(pid) {
            found_other_live = true;
        } else {
            let _ = std::fs::remove_file(path);
        }
    }
    !found_other_live
}

async fn save_session_model_override(
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    provider: &str,
    model: &str,
) -> Result<()> {
    let mut session = session_manager.get_or_create_async(session_key).await;
    session.metadata.insert(
        "provider".to_string(),
        serde_json::Value::String(provider.to_string()),
    );
    session.metadata.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    crate::channels::record_recent_model(provider, model);
    session_manager.save(&session).await
}

async fn save_session_streaming_override(
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    streaming: bool,
) -> Result<()> {
    let mut session = session_manager.get_or_create_async(session_key).await;
    session
        .metadata
        .insert("streaming".to_string(), serde_json::Value::Bool(streaming));
    session_manager.save(&session).await
}

fn save_default_model_selection(provider: &str, model: &str) -> Result<()> {
    let mut cfg = crate::config::loader::load_config()?;
    cfg.agents.defaults.provider = provider.to_string();
    cfg.agents.defaults.model = model.to_string();
    crate::channels::record_recent_model(provider, model);
    crate::config::loader::save_config(&cfg)
}

async fn apply_session_model_selection(
    agent_loop: &Arc<tokio::sync::Mutex<crate::agent::agent_loop::AgentLoop>>,
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    marker_dir: &std::path::Path,
    provider: &str,
    model: &str,
) -> Result<()> {
    let mut cfg = crate::config::loader::load_config()?;
    cfg.agents.defaults.provider = provider.to_string();
    cfg.agents.defaults.model = model.to_string();
    let resolved = crate::providers::resolver::resolve_provider_full(&cfg, model)?;
    save_session_model_override(session_manager, session_key, provider, model).await?;
    write_tui_marker_in_dir(marker_dir, std::process::id(), session_key, model, provider)?;
    match agent_loop.try_lock() {
        Ok(mut loop_lock) => loop_lock.update_model_and_provider(cfg, resolved.instance),
        Err(_) => {}
    }
    Ok(())
}

/// Archive the current session (unless already an archived history key) and
/// switch the active session key to `base_key`.
async fn reset_active_session(
    shared: &tokio::sync::RwLock<String>,
    session_manager: &crate::session::SessionManager,
    base_key: &str,
) {
    let cur = shared.read().await.clone();
    if !cur.starts_with("cli:history_") {
        let _ = crate::cli::archive_current_session(session_manager, &cur).await;
    }
    *shared.write().await = base_key.to_string();
}

struct RatatuiGuard;

impl Drop for RatatuiGuard {
    fn drop(&mut self) {
        let _ = stdout().execute(crossterm::event::DisableMouseCapture);
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
        let _ = stdout().execute(crossterm::cursor::Show);
        IS_RATATUI_ACTIVE.store(false, Ordering::SeqCst);
    }
}

pub async fn handle_ratatui_tui() -> Result<()> {
    let config = crate::config::loader::load_config().unwrap_or_default();
    let mut model = config.agents.defaults.model.clone();
    let mut provider = config.agents.defaults.provider.clone();
    let base_session_key = crate::config::loader::get_cli_session_key();
    // Active session key is mutable: /history restore and /new-session swap it.
    let session_key = Arc::new(tokio::sync::RwLock::new(base_session_key.clone()));
    let workspace = crate::config::loader::active_workspace_or_current_dir();

    let sessions_dir = crate::config::loader::resolve_path("~/.openz/sessions");
    let session_manager = Arc::new(crate::session::SessionManager::new(sessions_dir));

    // Build AgentLoop instance wrapped in a Mutex so it can be updated dynamically
    let agent_loop = Arc::new(tokio::sync::Mutex::new(
        crate::cli::builder::build_agent_loop(config.clone()).await?,
    ));

    // Interactive Session History Menu on startup if history exists
    let history = crate::cli::load_session_history()?;
    if history.is_empty() {
        crate::cli::archive_current_session(&session_manager, &base_session_key).await?;
    } else {
        let selected = match crate::agent::style::select_menu_with_history(
            "Welcome to OpenZ! Select an option:",
            &history,
        ) {
            Ok(s) => s,
            Err(_) => {
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
                return Ok(());
            }
        };
        if selected == 0 {
            reset_active_session(&session_key, &session_manager, &base_session_key).await;
        } else {
            // Adopt the chosen session in place — no copy, so /history shows no duplicates
            reset_active_session(&session_key, &session_manager, &base_session_key).await;
            *session_key.write().await = history[selected - 1].key.clone();
        }
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(crossterm::event::EnableMouseCapture)?;
    stdout.execute(crossterm::cursor::Show)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    IS_RATATUI_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = RatatuiGuard;

    let mut session_config = config.clone();
    let active_key = session_key.read().await.clone();
    if let Ok(session) = session_manager.load(&active_key) {
        crate::agent::agent_loop::apply_session_overrides(&mut session_config, &session.metadata);
        model = session_config.agents.defaults.model.clone();
        provider = session_config.agents.defaults.provider.clone();
        if session_config.agents.defaults.model != config.agents.defaults.model
            || session_config.agents.defaults.provider != config.agents.defaults.provider
        {
            if let Ok(resolved) = crate::providers::resolver::resolve_provider_full(
                &session_config,
                &session_config.agents.defaults.model,
            ) {
                if let Ok(mut loop_lock) = agent_loop.try_lock() {
                    loop_lock.update_model_and_provider(session_config.clone(), resolved.instance);
                }
            }
        }
    }

    let mut app = RatatuiApp::new(model, provider, active_key.clone());
    let marker_dir = tui_marker_dir();
    let _ = write_tui_marker_in_dir(
        &marker_dir,
        std::process::id(),
        &active_key,
        &app.model,
        &app.provider,
    );

    // Load selected session history into Ratatui conversation stream
    if let Ok(session) = session_manager.load(&active_key) {
        for msg in session.messages {
            app.messages.push(ChatMessage::from_session_message(&msg));
        }
    }

    let (turn_tx, mut turn_rx) = tokio::sync::mpsc::unbounded_channel::<TurnEvent>();
    let (model_tx, mut model_rx) =
        tokio::sync::mpsc::unbounded_channel::<(String, String, Vec<String>)>();

    loop {
        // Drain any async model fetch results
        while let Ok((prov_name, prov_display, fetched_models)) = model_rx.try_recv() {
            if matches!(&app.modal, ModalState::ModelSelect { provider_name, .. } if provider_name == &prov_name)
            {
                let count = fetched_models.len();
                app.modal = ModalState::ModelSelect {
                    provider_name: prov_name,
                    provider_display: prov_display,
                    models: fetched_models,
                    filtered_indices: (0..count).collect(),
                    selected_idx: 0,
                    filter: String::new(),
                    loading: false,
                };
            }
        }

        // Drain any incoming background responses from AgentLoop
        while let Ok(event) = turn_rx.try_recv() {
            app.is_thinking = false;
            app.work_start = None;
            match event {
                TurnEvent::SyncSession(msgs) => {
                    app.apply_sync_session(msgs);
                }
                TurnEvent::SingleMessage(msg) => {
                    app.messages.push(msg);
                }
                TurnEvent::Error(err) => {
                    app.messages
                        .push(ChatMessage::notice(format!("⚠ Error: {}", err)));
                }
            }
            app.scroll_to_bottom();
        }

        // Tick spinner animation
        app.spinner_idx = app.spinner_idx.wrapping_add(1);

        terminal.draw(|f| ui::render_ratatui_ui(f, &mut app))?;

        if event::poll(Duration::from_millis(TICK_MS))? {
            match event::read()? {
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        app.scroll_up(SCROLL_STEP);
                    }
                    MouseEventKind::ScrollDown => {
                        app.scroll_down(SCROLL_STEP);
                    }
                    _ => {}
                },
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        // Global interrupt: Ctrl+C
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('c')
                        {
                            if app.is_thinking {
                                crate::shutdown::trigger_cli_cancel();
                                app.is_thinking = false;
                                app.work_start = None;
                                app.messages.push(ChatMessage::notice(
                                    "Turn cancelled by user.".to_string(),
                                ));
                                continue;
                            }
                            if app.modal.is_active() {
                                app.modal = ModalState::None;
                                continue;
                            }
                            break;
                        }

                        // ── 1. Modal Key Interception ───────────────────────────
                        if app.modal.is_active() {
                            match &mut app.modal {
                                ModalState::ProviderSelect {
                                    providers,
                                    selected_idx,
                                } => match key.code {
                                    KeyCode::Up => {
                                        if *selected_idx > 0 {
                                            *selected_idx -= 1;
                                        } else {
                                            *selected_idx = providers.len().saturating_sub(1);
                                        }
                                    }
                                    KeyCode::Down => {
                                        if *selected_idx + 1 < providers.len() {
                                            *selected_idx += 1;
                                        } else {
                                            *selected_idx = 0;
                                        }
                                    }
                                    KeyCode::Esc => {
                                        app.modal = ModalState::None;
                                    }
                                    KeyCode::Enter => {
                                        let (prov_name, prov_display) =
                                            providers[*selected_idx].clone();

                                        app.modal = ModalState::ModelSelect {
                                            provider_name: prov_name.clone(),
                                            provider_display: prov_display.clone(),
                                            models: Vec::new(),
                                            filtered_indices: Vec::new(),
                                            selected_idx: 0,
                                            filter: String::new(),
                                            loading: true,
                                        };

                                        let fetch_tx = model_tx.clone();
                                        let fetch_prov = prov_name.clone();
                                        let fetch_display = prov_display.clone();

                                        tokio::spawn(async move {
                                            let config = crate::config::loader::load_config()
                                                .unwrap_or_default();
                                            let mut models = Vec::new();
                                            if let Some(api_models) =
                                                crate::channels::fetch_provider_models(
                                                    &fetch_prov,
                                                    &config,
                                                )
                                                .await
                                            {
                                                models = api_models;
                                            }
                                            let curated = app::curated_models_for(&fetch_prov);
                                            for m in curated {
                                                if !models.iter().any(|existing| {
                                                    existing.eq_ignore_ascii_case(&m)
                                                }) {
                                                    models.push(m);
                                                }
                                            }
                                            models = crate::channels::model_menu_options_with_prefs(
                                                &fetch_prov,
                                                models,
                                            );
                                            let _ =
                                                fetch_tx.send((fetch_prov, fetch_display, models));
                                        });
                                    }
                                    _ => {}
                                },
                                ModalState::ModelSelect {
                                    provider_name,
                                    models,
                                    filtered_indices,
                                    selected_idx,
                                    filter,
                                    loading,
                                    ..
                                } => {
                                    if *loading {
                                        if key.code == KeyCode::Esc {
                                            app.modal = ModalState::None;
                                        }
                                        continue;
                                    }

                                    match key.code {
                                        KeyCode::Up => {
                                            if *selected_idx > 0 {
                                                *selected_idx -= 1;
                                            } else {
                                                *selected_idx =
                                                    filtered_indices.len().saturating_sub(1);
                                            }
                                        }
                                        KeyCode::Down => {
                                            if *selected_idx + 1 < filtered_indices.len() {
                                                *selected_idx += 1;
                                            } else {
                                                *selected_idx = 0;
                                            }
                                        }
                                        KeyCode::Esc => {
                                            app.modal = ModalState::None;
                                        }
                                        KeyCode::Backspace => {
                                            filter.pop();
                                            app.modal.update_model_filter();
                                        }
                                        KeyCode::Char(c) => {
                                            filter.push(c);
                                            app.modal.update_model_filter();
                                        }
                                        KeyCode::Enter => {
                                            if !filtered_indices.is_empty() {
                                                let real_idx = filtered_indices[*selected_idx];
                                                let chosen_model =
                                                    crate::channels::model_menu_model_name(
                                                        &models[real_idx],
                                                    )
                                                    .to_string();
                                                let prov = provider_name.clone();

                                                match apply_session_model_selection(
                                                    &agent_loop,
                                                    &session_manager,
                                                    &session_key.read().await.clone(),
                                                    &marker_dir,
                                                    &prov,
                                                    &chosen_model,
                                                )
                                                .await
                                                {
                                                    Ok(()) => {
                                                        app.model = chosen_model.clone();
                                                        app.provider = prov.clone();
                                                        app.messages.push(ChatMessage::notice(
                                                            format!(
                                                            "✓ Switched this session to: {} ({})",
                                                            chosen_model, prov
                                                        ),
                                                        ));
                                                    }
                                                    Err(e) => {
                                                        app.messages.push(ChatMessage::notice(
                                                            format!(
                                                                "⚠ Failed to switch model: {}",
                                                                e
                                                            ),
                                                        ));
                                                    }
                                                }
                                            }
                                            app.modal = ModalState::None;
                                        }
                                        _ => {}
                                    }
                                }
                                ModalState::Help => match key.code {
                                    KeyCode::Esc | KeyCode::Enter => {
                                        app.modal = ModalState::None;
                                    }
                                    _ => {}
                                },
                                ModalState::History {
                                    sessions,
                                    selected_idx,
                                } => match key.code {
                                    KeyCode::Up => {
                                        if *selected_idx > 0 {
                                            *selected_idx -= 1;
                                        } else {
                                            *selected_idx = sessions.len().saturating_sub(1);
                                        }
                                    }
                                    KeyCode::Down => {
                                        if *selected_idx + 1 < sessions.len() {
                                            *selected_idx += 1;
                                        } else {
                                            *selected_idx = 0;
                                        }
                                    }
                                    KeyCode::Esc => {
                                        app.modal = ModalState::None;
                                    }
                                    KeyCode::Enter => {
                                        if app.is_thinking {
                                            app.messages.push(ChatMessage::notice(
                                                "⏳ Cannot switch sessions while a turn is running — Ctrl+C to cancel first.".to_string(),
                                            ));
                                            app.modal = ModalState::None;
                                            continue;
                                        }
                                        if !sessions.is_empty() {
                                            let (target_key, title, _) =
                                                sessions[*selected_idx].clone();
                                            if let Ok(loaded) = session_manager.load(&target_key) {
                                                // Archive whatever we're leaving, then adopt the
                                                // target key so future prompts append to it
                                                reset_active_session(
                                                    &session_key,
                                                    &session_manager,
                                                    &base_session_key,
                                                )
                                                .await;
                                                *session_key.write().await = target_key.clone();

                                                app.messages.clear();
                                                app.session_key = target_key.clone();
                                                for msg in loaded.messages {
                                                    app.messages.push(
                                                        ChatMessage::from_session_message(&msg),
                                                    );
                                                }
                                                app.scroll_to_bottom();
                                                app.messages.push(ChatMessage::notice(format!(
                                                    "✓ Restored session: {}",
                                                    title
                                                )));
                                                let _ = write_tui_marker_in_dir(
                                                    &marker_dir,
                                                    std::process::id(),
                                                    &target_key,
                                                    &app.model,
                                                    &app.provider,
                                                );
                                            }
                                        }
                                        app.modal = ModalState::None;
                                    }
                                    _ => {}
                                },
                                _ => {}
                            }
                            continue;
                        }

                        // ── 2. Standard View & Input Key Events ─────────────────
                        let matches = app.matching_slash_commands();
                        let has_matches = !matches.is_empty();

                        match key.code {
                            KeyCode::Esc => {
                                if app.is_thinking {
                                    crate::shutdown::trigger_cli_cancel();
                                    app.is_thinking = false;
                                    app.work_start = None;
                                    app.messages.push(ChatMessage::notice(
                                        "Turn interrupted by user.".to_string(),
                                    ));
                                } else {
                                    app.typed_input.clear();
                                    app.cursor_idx = 0;
                                    app.selected_index = None;
                                    app.history_idx = None;
                                }
                            }
                            KeyCode::Tab => {
                                if has_matches {
                                    let idx = app.selected_index.unwrap_or(0);
                                    if idx < matches.len() {
                                        let (cmd, _) = &matches[idx];
                                        app.typed_input = cmd.chars().collect();
                                        app.cursor_idx = app.typed_input.len();
                                        app.selected_index = None;
                                    }
                                }
                            }
                            KeyCode::PageUp => {
                                app.scroll_up(SCROLL_PAGE);
                            }
                            KeyCode::PageDown => {
                                app.scroll_down(SCROLL_PAGE);
                            }
                            KeyCode::Up => {
                                let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);
                                let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                                let has_alt = key.modifiers.contains(KeyModifiers::ALT);

                                if has_shift || has_ctrl || has_alt {
                                    app.scroll_up(SCROLL_STEP);
                                } else if has_matches {
                                    if let Some(idx) = app.selected_index {
                                        if idx > 0 {
                                            app.selected_index = Some(idx - 1);
                                        } else {
                                            app.selected_index =
                                                Some(matches.len().saturating_sub(1));
                                        }
                                    } else {
                                        app.selected_index = Some(matches.len().saturating_sub(1));
                                    }
                                } else if !app.prompt_history.is_empty()
                                    && (app.history_idx.is_some() || !app.typed_input.is_empty())
                                {
                                    let next_idx = match app.history_idx {
                                        None => app.prompt_history.len().saturating_sub(1),
                                        Some(i) => i.saturating_sub(1),
                                    };
                                    app.history_idx = Some(next_idx);
                                    if let Some(hist_str) = app.prompt_history.get(next_idx) {
                                        app.typed_input = hist_str.chars().collect();
                                        app.cursor_idx = app.typed_input.len();
                                    }
                                } else {
                                    // When input is empty, plain Up scrolls the timeline
                                    app.scroll_up(SCROLL_JUMP);
                                }
                            }
                            KeyCode::Down => {
                                let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);
                                let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                                let has_alt = key.modifiers.contains(KeyModifiers::ALT);

                                if has_shift || has_ctrl || has_alt {
                                    app.scroll_down(SCROLL_STEP);
                                } else if has_matches {
                                    if let Some(idx) = app.selected_index {
                                        if idx + 1 < matches.len() {
                                            app.selected_index = Some(idx + 1);
                                        } else {
                                            app.selected_index = Some(0);
                                        }
                                    } else {
                                        app.selected_index = Some(0);
                                    }
                                } else if let Some(i) = app.history_idx {
                                    if i + 1 < app.prompt_history.len() {
                                        let next_idx = i + 1;
                                        app.history_idx = Some(next_idx);
                                        if let Some(hist_str) = app.prompt_history.get(next_idx) {
                                            app.typed_input = hist_str.chars().collect();
                                            app.cursor_idx = app.typed_input.len();
                                        }
                                    } else {
                                        app.history_idx = None;
                                        app.typed_input.clear();
                                        app.cursor_idx = 0;
                                    }
                                } else {
                                    // When input is empty, plain Down scrolls the timeline
                                    app.scroll_down(SCROLL_JUMP);
                                }
                            }
                            KeyCode::Char(c) => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'p' {
                                    app.scroll_up(SCROLL_STEP_EM);
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'n'
                                {
                                    app.scroll_down(SCROLL_STEP_EM);
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'u'
                                {
                                    app.scroll_up(SCROLL_HALF);
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'd'
                                {
                                    app.scroll_down(SCROLL_HALF);
                                } else {
                                    app.typed_input.insert(app.cursor_idx, c);
                                    app.cursor_idx += 1;
                                    app.selected_index = None;
                                    app.history_idx = None;
                                }
                            }
                            KeyCode::Left => {
                                app.cursor_idx = app.cursor_idx.saturating_sub(1);
                            }
                            KeyCode::Right => {
                                if app.cursor_idx < app.typed_input.len() {
                                    app.cursor_idx += 1;
                                }
                            }
                            KeyCode::Home => {
                                if key.modifiers.contains(KeyModifiers::SHIFT)
                                    || key.modifiers.contains(KeyModifiers::CONTROL)
                                {
                                    app.scroll_to_top();
                                } else {
                                    app.cursor_idx = 0;
                                }
                            }
                            KeyCode::End => {
                                if key.modifiers.contains(KeyModifiers::SHIFT)
                                    || key.modifiers.contains(KeyModifiers::CONTROL)
                                {
                                    app.scroll_to_bottom();
                                } else {
                                    app.cursor_idx = app.typed_input.len();
                                }
                            }
                            KeyCode::Backspace => {
                                if app.cursor_idx > 0 {
                                    app.typed_input.remove(app.cursor_idx - 1);
                                    app.cursor_idx -= 1;
                                }
                                app.selected_index = None;
                                app.history_idx = None;
                            }
                            KeyCode::Enter => {
                                let input_str = if let Some(idx) = app.selected_index {
                                    if idx < matches.len() {
                                        matches[idx].0.to_string()
                                    } else {
                                        app.typed_input.iter().collect::<String>()
                                    }
                                } else {
                                    app.typed_input.iter().collect::<String>()
                                };

                                let trimmed = input_str.trim();

                                if !trimmed.is_empty() {
                                    app.prompt_history.push(input_str.clone());

                                    if trimmed == "/exit" || trimmed == "/quit" {
                                        break;
                                    } else if trimmed == "/clear" {
                                        app.messages.clear();
                                        app.scroll_to_top();
                                    } else if trimmed == "/model" || trimmed == "/models" {
                                        let cfg = crate::config::loader::load_config()
                                            .unwrap_or_default();
                                        let configured = app::build_configured_providers(&cfg);
                                        app.modal = ModalState::ProviderSelect {
                                            providers: if configured.is_empty() {
                                                app::PROVIDER_REGISTRY
                                                    .iter()
                                                    .map(|p| {
                                                        (p.name.to_string(), p.display.to_string())
                                                    })
                                                    .collect()
                                            } else {
                                                configured
                                            },
                                            selected_idx: 0,
                                        };
                                    } else if trimmed == "/help" {
                                        app.modal = ModalState::Help;
                                    } else if trimmed == "/history" {
                                        let sessions_data: Vec<(String, String, String)> =
                                            match crate::cli::load_session_history() {
                                                Ok(hist) => hist
                                                    .into_iter()
                                                    .map(|item| {
                                                        let time = item
                                                            .updated_at
                                                            .format("%Y-%m-%d %H:%M")
                                                            .to_string();
                                                        (item.key, item.display_title, time)
                                                    })
                                                    .collect(),
                                                Err(_) => Vec::new(),
                                            };

                                        app.modal = ModalState::History {
                                            sessions: sessions_data,
                                            selected_idx: 0,
                                        };
                                    } else if trimmed == "/new-session" {
                                        if app.is_thinking {
                                            app.messages.push(ChatMessage::notice(
                                                "⏳ Cannot start a new session while a turn is running — Ctrl+C to cancel first.".to_string(),
                                            ));
                                            continue;
                                        }
                                        reset_active_session(
                                            &session_key,
                                            &session_manager,
                                            &base_session_key,
                                        )
                                        .await;
                                        app.session_key = base_session_key.clone();
                                        let _ = write_tui_marker_in_dir(
                                            &marker_dir,
                                            std::process::id(),
                                            &base_session_key,
                                            &app.model,
                                            &app.provider,
                                        );
                                        app.messages.clear();
                                        app.scroll_to_top();
                                        app.messages.push(ChatMessage::notice(
                                            "Started a fresh conversation session.".to_string(),
                                        ));
                                    } else if trimmed == "/mcps" {
                                        app.messages
                                            .push(ChatMessage::simple("user", input_str.clone()));
                                        let mut mcp_msg = String::from("Configured MCP Servers:\n");
                                        let loop_guard = agent_loop.lock().await;
                                        if loop_guard.config.mcp_servers.is_empty() {
                                            mcp_msg.push_str("  No MCP servers configured.\n");
                                        } else {
                                            for (name, mcp_cfg) in &loop_guard.config.mcp_servers {
                                                let status = if mcp_cfg.enabled {
                                                    "enabled"
                                                } else {
                                                    "disabled"
                                                };
                                                mcp_msg.push_str(&format!(
                                                    "  • {} [{}] - {}\n",
                                                    name, status, mcp_cfg.command
                                                ));
                                            }
                                        }
                                        app.messages
                                            .push(ChatMessage::simple("assistant", mcp_msg));
                                    } else if trimmed == "/streaming" {
                                        app.messages
                                            .push(ChatMessage::simple("user", input_str.clone()));
                                        let key_snapshot = session_key.read().await.clone();
                                        let current_streaming = session_manager
                                            .load(&key_snapshot)
                                            .ok()
                                            .and_then(|session| {
                                                session
                                                    .metadata
                                                    .get("streaming")
                                                    .and_then(|v| v.as_bool())
                                            })
                                            .unwrap_or_else(|| {
                                                agent_loop
                                                    .try_lock()
                                                    .map(|l| l.config.agents.defaults.streaming)
                                                    .unwrap_or(config.agents.defaults.streaming)
                                            });
                                        let next_streaming = !current_streaming;
                                        let _ = save_session_streaming_override(
                                            &session_manager,
                                            &key_snapshot,
                                            next_streaming,
                                        )
                                        .await;
                                        app.messages.push(ChatMessage::simple(
                                            "assistant",
                                            format!(
                                                "Response streaming is now {} for this session.",
                                                if next_streaming {
                                                    "enabled"
                                                } else {
                                                    "disabled"
                                                }
                                            ),
                                        ));
                                    } else if trimmed.starts_with('/') {
                                        app.messages
                                            .push(ChatMessage::simple("user", input_str.clone()));
                                        app.messages.push(ChatMessage::simple(
                                            "assistant",
                                            format!("Command {} executed. Type /help for all available commands.", trimmed),
                                        ));
                                    } else if app.is_thinking {
                                        // A turn is already running — don't queue invisibly
                                        app.messages.push(ChatMessage::notice(
                                            "⏳ A turn is still running — Ctrl+C to cancel. Your input was kept.".to_string(),
                                        ));
                                        continue;
                                    } else {
                                        // Standard User Prompt -> Dispatch to AgentLoop
                                        app.messages
                                            .push(ChatMessage::simple("user", input_str.clone()));
                                        app.is_thinking = true;
                                        app.work_start = Some(Instant::now());
                                        app.scroll_to_bottom();

                                        let agent_loop_clone = agent_loop.clone();
                                        let turn_session_key = session_key.read().await.clone();
                                        let session_manager_clone = session_manager.clone();
                                        let prompt_text = input_str.clone();
                                        let turn_tx_clone = turn_tx.clone();
                                        let workspace_clone = workspace.clone();

                                        tokio::spawn(async move {
                                            let loop_guard = agent_loop_clone.lock().await;
                                            let run_result =
                                                crate::config::loader::ACTIVE_WORKSPACE
                                                    .scope(workspace_clone, async {
                                                        loop_guard
                                                            .run(&prompt_text, &turn_session_key)
                                                            .await
                                                    })
                                                    .await;
                                            match run_result {
                                                Ok(_res) => {
                                                    if let Ok(session) = session_manager_clone
                                                        .load(&turn_session_key)
                                                    {
                                                        let mut msgs = Vec::new();
                                                        for m in session.messages {
                                                            msgs.push(
                                                                ChatMessage::from_session_message(
                                                                    &m,
                                                                ),
                                                            );
                                                        }
                                                        let _ = turn_tx_clone
                                                            .send(TurnEvent::SyncSession(msgs));
                                                    } else {
                                                        let _ = turn_tx_clone.send(
                                                            TurnEvent::SingleMessage(
                                                                ChatMessage::simple(
                                                                    "assistant",
                                                                    _res.content,
                                                                ),
                                                            ),
                                                        );
                                                    }
                                                }
                                                Err(err) => {
                                                    let _ = turn_tx_clone
                                                        .send(TurnEvent::Error(err.to_string()));
                                                }
                                            }
                                        });
                                    }

                                    app.typed_input.clear();
                                    app.cursor_idx = 0;
                                    app.selected_index = None;
                                    app.history_idx = None;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        if app.should_exit {
            break;
        }
    }

    remove_tui_marker_in_dir(&marker_dir, std::process::id());
    if is_last_live_tui_in_dir(&marker_dir, std::process::id()) {
        let _ = save_default_model_selection(&app.provider, &app.model);
    }

    Ok(())
}
