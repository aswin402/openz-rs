pub mod app;
pub mod theme;
pub mod ui;

use anyhow::Result;
use app::{ChatMessage, ModalState, RatatuiApp, IS_RATATUI_ACTIVE};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

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

struct RatatuiGuard;

impl Drop for RatatuiGuard {
    fn drop(&mut self) {
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
    let session_key = crate::config::loader::get_cli_session_key();
    let workspace = crate::config::loader::active_workspace_or_current_dir();

    let sessions_dir = crate::config::loader::resolve_path("~/.openz/sessions");
    let session_manager = crate::session::SessionManager::new(sessions_dir);

    // Build AgentLoop instance wrapped in a Mutex so it can be updated dynamically
    let agent_loop = Arc::new(tokio::sync::Mutex::new(
        crate::cli::builder::build_agent_loop(config.clone()).await?,
    ));

    // Interactive Session History Menu on startup if history exists
    let history = crate::cli::load_session_history()?;
    if history.is_empty() {
        crate::cli::archive_current_session(&session_manager, &session_key).await?;
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(crossterm::cursor::Show)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    IS_RATATUI_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = RatatuiGuard;

    let mut session_config = config.clone();
    if let Ok(session) = session_manager.load(&session_key) {
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

    let mut app = RatatuiApp::new(model, provider, session_key.clone());
    let marker_dir = tui_marker_dir();
    let _ = write_tui_marker_in_dir(
        &marker_dir,
        std::process::id(),
        &session_key,
        &app.model,
        &app.provider,
    );

    // Load selected session history into Ratatui conversation stream
    if let Ok(session) = session_manager.load(&session_key) {
        for msg in session.messages {
            app.messages
                .push(ChatMessage::simple(&msg.role, msg.content));
        }
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ChatMessage>();
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
        while let Ok(new_msg) = rx.try_recv() {
            app.is_thinking = false;
            app.work_start = None;
            if let Some(last) = app.messages.last_mut() {
                if last.role == "assistant" && last.content.starts_with("⏳") {
                    *last = new_msg;
                } else {
                    app.messages.push(new_msg);
                }
            } else {
                app.messages.push(new_msg);
            }
            app.scroll_offset = 0;
            app.auto_scroll = true;
        }

        // Tick spinner animation
        app.spinner_idx = app.spinner_idx.wrapping_add(1);

        terminal.draw(|f| ui::render_ratatui_ui(f, &app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Global interrupt: Ctrl+C
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        if app.is_thinking {
                            crate::shutdown::trigger_cli_cancel();
                            app.is_thinking = false;
                            app.work_start = None;
                            app.messages.push(ChatMessage::simple(
                                "assistant",
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
                                            if !models
                                                .iter()
                                                .any(|existing| existing.eq_ignore_ascii_case(&m))
                                            {
                                                models.push(m);
                                            }
                                        }
                                        models = crate::channels::model_menu_options_with_prefs(
                                            &fetch_prov,
                                            models,
                                        );
                                        let _ = fetch_tx.send((fetch_prov, fetch_display, models));
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
                                            *selected_idx = filtered_indices.len().saturating_sub(1);
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
                                                crate::channels::model_menu_model_name(&models[real_idx])
                                                    .to_string();
                                            let prov = provider_name.clone();

                                            match apply_session_model_selection(
                                                &agent_loop,
                                                &session_manager,
                                                &session_key,
                                                &marker_dir,
                                                &prov,
                                                &chosen_model,
                                            )
                                            .await
                                            {
                                                Ok(()) => {
                                                    app.model = chosen_model.clone();
                                                    app.provider = prov.clone();
                                                    app.messages.push(ChatMessage::simple(
                                                        "assistant",
                                                        format!(
                                                            "✓ Switched this session to: {} ({})",
                                                            chosen_model, prov
                                                        ),
                                                    ));
                                                }
                                                Err(e) => {
                                                    app.messages.push(ChatMessage::simple(
                                                        "assistant",
                                                        format!("⚠ Failed to switch model: {}", e),
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
                                    if !sessions.is_empty() {
                                        let (target_key, title, _) = sessions[*selected_idx].clone();
                                        if let Ok(loaded) = session_manager.load(&target_key) {
                                            app.messages.clear();
                                            app.session_key = target_key.clone();
                                            for msg in loaded.messages {
                                                app.messages.push(ChatMessage::simple(
                                                    &msg.role, msg.content,
                                                ));
                                            }
                                            app.messages.push(ChatMessage::simple(
                                                "assistant",
                                                format!("✓ Restored session: {}", title),
                                            ));
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
                                app.messages.push(ChatMessage::simple(
                                    "assistant",
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
                        KeyCode::Up => {
                            if has_matches {
                                if let Some(idx) = app.selected_index {
                                    if idx > 0 {
                                        app.selected_index = Some(idx - 1);
                                    } else {
                                        app.selected_index = Some(matches.len().saturating_sub(1));
                                    }
                                } else {
                                    app.selected_index = Some(matches.len().saturating_sub(1));
                                }
                            } else if !app.prompt_history.is_empty() {
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
                                app.auto_scroll = false;
                                app.scroll_offset = app.scroll_offset.saturating_add(1);
                            }
                        }
                        KeyCode::Down => {
                            if has_matches {
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
                                app.auto_scroll = false;
                                app.scroll_offset = app.scroll_offset.saturating_sub(1);
                            }
                        }
                        KeyCode::PageUp => {
                            app.auto_scroll = false;
                            app.scroll_offset = app.scroll_offset.saturating_add(6);
                        }
                        KeyCode::PageDown => {
                            app.auto_scroll = false;
                            app.scroll_offset = app.scroll_offset.saturating_sub(6);
                        }
                        KeyCode::Char(c) => {
                            app.typed_input.insert(app.cursor_idx, c);
                            app.cursor_idx += 1;
                            app.selected_index = None;
                            app.history_idx = None;
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
                            app.cursor_idx = 0;
                        }
                        KeyCode::End => {
                            app.cursor_idx = app.typed_input.len();
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
                                    app.scroll_offset = 0;
                                    app.auto_scroll = true;
                                } else if trimmed == "/model" || trimmed == "/models" {
                                    let cfg = crate::config::loader::load_config().unwrap_or_default();
                                    let configured = app::build_configured_providers(&cfg);
                                    app.modal = ModalState::ProviderSelect {
                                        providers: if configured.is_empty() {
                                            app::PROVIDER_REGISTRY
                                                .iter()
                                                .map(|p| (p.name.to_string(), p.display.to_string()))
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
                                    let _ = crate::cli::archive_current_session(
                                        &session_manager,
                                        &session_key,
                                    )
                                    .await;
                                    app.messages.clear();
                                    app.scroll_offset = 0;
                                    app.auto_scroll = true;
                                    app.messages.push(ChatMessage::simple(
                                        "assistant",
                                        "Started a fresh conversation session.".to_string(),
                                    ));
                                } else if trimmed == "/mcps" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
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
                                    app.messages.push(ChatMessage::simple("assistant", mcp_msg));
                                } else if trimmed == "/streaming" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let current_streaming = session_manager
                                        .load(&session_key)
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
                                        &session_key,
                                        next_streaming,
                                    )
                                    .await;
                                    app.messages.push(ChatMessage::simple(
                                        "assistant",
                                        format!(
                                            "Response streaming is now {} for this session.",
                                            if next_streaming { "enabled" } else { "disabled" }
                                        ),
                                    ));
                                } else if trimmed.starts_with('/') {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    app.messages.push(ChatMessage::simple(
                                        "assistant",
                                        format!("Command {} executed. Type /help for all available commands.", trimmed),
                                    ));
                                } else {
                                    // Standard User Prompt -> Dispatch to AgentLoop
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    app.messages.push(ChatMessage::simple(
                                        "assistant",
                                        "⏳ OpenZ is thinking...".to_string(),
                                    ));
                                    app.is_thinking = true;
                                    app.work_start = Some(Instant::now());
                                    app.auto_scroll = true;

                                    let agent_loop_clone = agent_loop.clone();
                                    let session_key_clone = session_key.clone();
                                    let prompt_text = input_str.clone();
                                    let tx_clone = tx.clone();
                                    let workspace_clone = workspace.clone();

                                    tokio::spawn(async move {
                                        let loop_guard = agent_loop_clone.lock().await;
                                        let run_result = crate::config::loader::ACTIVE_WORKSPACE
                                            .scope(workspace_clone, async {
                                                loop_guard
                                                    .run(&prompt_text, &session_key_clone)
                                                    .await
                                            })
                                            .await;
                                        match run_result {
                                            Ok(res) => {
                                                let _ = tx_clone.send(ChatMessage::simple(
                                                    "assistant",
                                                    res.content,
                                                ));
                                            }
                                            Err(err) => {
                                                let _ = tx_clone.send(ChatMessage::simple(
                                                    "assistant",
                                                    format!("⚠ Error: {}", err),
                                                ));
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
