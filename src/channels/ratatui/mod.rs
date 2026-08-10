pub mod app;
pub mod theme;
pub mod ui;

use anyhow::Result;
use app::{ChatMessage, IS_RATATUI_ACTIVE};
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
use std::time::Duration;

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
        Err(_) => {
            // Busy loop keeps its current turn model; the saved session override applies next turn.
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_marker_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("openz-ratatui-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn tui_marker_prunes_dead_peers_before_last_check() {
        let dir = temp_marker_dir();
        let dead_pid = u32::MAX - 1;
        write_tui_marker_in_dir(&dir, dead_pid, "dead-session", "old-model", "openai").unwrap();

        assert!(is_last_live_tui_in_dir(&dir, std::process::id()));
        assert!(!tui_marker_path_in_dir(&dir, dead_pid).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_model_override_updates_session_metadata() {
        let dir = temp_marker_dir();
        let manager = crate::session::SessionManager::new(dir.clone());

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            save_session_model_override(&manager, "test-session", "deepseek", "deepseek-chat")
                .await
                .unwrap();
        });

        let session = manager.load("test-session").unwrap();
        assert_eq!(session.metadata["provider"], "deepseek");
        assert_eq!(session.metadata["model"], "deepseek-chat");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_streaming_override_updates_session_metadata() {
        let dir = temp_marker_dir();
        let manager = crate::session::SessionManager::new(dir.clone());

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            save_session_streaming_override(&manager, "test-session", false)
                .await
                .unwrap();
        });

        let session = manager.load("test-session").unwrap();
        assert_eq!(session.metadata["streaming"], false);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

pub async fn handle_ratatui_tui() -> Result<()> {
    let config = crate::config::loader::load_config().unwrap_or_default();
    let mut model = config.agents.defaults.model.clone();
    let mut provider = config.agents.defaults.provider.clone();
    let session_key = crate::config::loader::get_cli_session_key();

    let sessions_dir = crate::config::loader::resolve_path("~/.openz/sessions");
    let session_manager = crate::session::SessionManager::new(sessions_dir);

    // Build AgentLoop instance wrapped in a Mutex so it can be updated dynamically
    let agent_loop = Arc::new(tokio::sync::Mutex::new(
        crate::cli::builder::build_agent_loop(config.clone()).await?,
    ));

    // Interactive Session History Menu on startup (Start New vs Restore Recent Session)
    let history = crate::cli::load_session_history()?;
    if history.is_empty() {
        crate::cli::archive_current_session(&session_manager, &session_key).await?;
    } else {
        let selected = match crate::agent::style::select_menu_with_history(
            "Welcome to OpenZ! Select an option:",
            &history,
        ) {
            Ok(s) => s,
            Err(_) => {
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
                std::process::exit(0);
            }
        };
        if selected == 0 {
            crate::cli::archive_current_session(&session_manager, &session_key).await?;
        } else {
            let selected_item = &history[selected - 1];
            if selected_item.key != session_key {
                crate::cli::archive_current_session(&session_manager, &session_key).await?;
                if let Ok(mut session) = session_manager.load(&selected_item.key) {
                    session.key = session_key.clone();
                    let _ = session_manager.save(&session).await;
                }
            }
        }
    }

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    IS_RATATUI_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = RatatuiGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

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

    let mut app = app::RatatuiApp::new(model, provider, session_key.clone());
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
    // Channel for async model list fetching
    let (model_tx, mut model_rx) =
        tokio::sync::mpsc::unbounded_channel::<(String, String, Vec<String>)>();

    loop {
        // Drain any async model fetch results
        while let Ok((prov_name, prov_display, fetched_models)) = model_rx.try_recv() {
            if matches!(&app.model_select, app::ModelSelectState::FetchingModels { provider_name, .. } if provider_name == &prov_name)
            {
                let mut models_list = fetched_models;
                models_list.push("Type manually (Custom Model)".to_string());
                models_list.push("Exit".to_string());
                app.model_select = app::ModelSelectState::ChoosingModel {
                    provider_name: prov_name,
                    provider_display: prov_display,
                    models: models_list,
                    selected_idx: 0,
                };
            }
        }
        // Drain any incoming background responses from AgentLoop
        while let Ok(new_msg) = rx.try_recv() {
            app.is_thinking = false;
            if let Some(last) = app.messages.last_mut() {
                if last.role == "assistant" && last.content.starts_with("⏳") {
                    *last = new_msg;
                } else {
                    app.messages.push(new_msg);
                }
            } else {
                app.messages.push(new_msg);
            }
            // Auto-scroll to bottom when new message arrives
            app.scroll_offset = 0;
        }

        // Tick spinner animation
        app.spinner_idx = app.spinner_idx.wrapping_add(1);

        terminal.draw(|f| ui::render_ratatui_ui(f, &app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        if app.is_thinking {
                            crate::shutdown::trigger_cli_cancel();
                            app.messages.push(ChatMessage::simple(
                                "assistant",
                                "Cancelling current turn...".to_string(),
                            ));
                            continue;
                        }
                        break;
                    }

                    // Intercept key events for active interactive model selection menu
                    match &mut app.model_select {
                        app::ModelSelectState::ChoosingProvider {
                            providers,
                            selected_idx,
                        } => {
                            match key.code {
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
                                    app.model_select = app::ModelSelectState::Closed;
                                }
                                KeyCode::Enter => {
                                    let (prov_name, prov_display) =
                                        providers[*selected_idx].clone();

                                    // Set FetchingModels state and spawn async model fetch
                                    app.model_select = app::ModelSelectState::FetchingModels {
                                        provider_name: prov_name.clone(),
                                        provider_display: prov_display.clone(),
                                    };
                                    let fetch_tx = model_tx.clone();
                                    let fetch_prov = prov_name.clone();
                                    let fetch_display = prov_display.clone();
                                    tokio::spawn(async move {
                                        let config = crate::config::loader::load_config()
                                            .unwrap_or_default();
                                        let mut models = Vec::new();
                                        // Try live API fetch first
                                        if let Some(api_models) =
                                            crate::channels::fetch_provider_models(
                                                &fetch_prov,
                                                &config,
                                            )
                                            .await
                                        {
                                            models = api_models;
                                        }
                                        // Merge curated fallbacks (deduped)
                                        let curated = app::curated_models_for(&fetch_prov);
                                        for m in curated {
                                            if !models
                                                .iter()
                                                .any(|existing| existing.eq_ignore_ascii_case(&m))
                                            {
                                                models.push(m);
                                            }
                                        }
                                        // Also include custom default_model from config
                                        if let Some(dm) = config
                                            .get_provider_config(&fetch_prov)
                                            .and_then(|p| p.default_model.clone())
                                            .filter(|m| !m.trim().is_empty())
                                        {
                                            if !models
                                                .iter()
                                                .any(|existing| existing.eq_ignore_ascii_case(&dm))
                                            {
                                                models.push(dm);
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
                            }
                            continue;
                        }
                        app::ModelSelectState::FetchingModels { .. } => {
                            // While fetching, only allow Esc to cancel
                            if key.code == KeyCode::Esc {
                                app.model_select = app::ModelSelectState::Closed;
                            }
                            continue;
                        }
                        app::ModelSelectState::ChoosingModel {
                            provider_name,
                            provider_display,
                            models,
                            selected_idx,
                        } => {
                            match key.code {
                                KeyCode::Up => {
                                    if *selected_idx > 0 {
                                        *selected_idx -= 1;
                                    } else {
                                        *selected_idx = models.len().saturating_sub(1);
                                    }
                                }
                                KeyCode::Down => {
                                    if *selected_idx + 1 < models.len() {
                                        *selected_idx += 1;
                                    } else {
                                        *selected_idx = 0;
                                    }
                                }
                                KeyCode::Esc => {
                                    app.model_select = app::ModelSelectState::Closed;
                                }
                                KeyCode::Enter => {
                                    if *selected_idx == models.len() - 1 {
                                        // Exit selected
                                        app.model_select = app::ModelSelectState::Closed;
                                    } else if models[*selected_idx]
                                        == "★ Favorite/Unfavorite current model"
                                    {
                                        let _ = crate::channels::toggle_favorite_model(
                                            provider_name,
                                            &app.model,
                                        );
                                        app.messages.push(ChatMessage::simple(
                                            "assistant",
                                            format!("★ Favorite toggled for {}", app.model),
                                        ));
                                        app.model_select = app::ModelSelectState::Closed;
                                    } else if models[*selected_idx]
                                        == "Type manually (Custom Model)"
                                    {
                                        // Switch to custom model input mode — put prefix in input box
                                        let prov = provider_name.clone();
                                        app.model_select = app::ModelSelectState::Closed;
                                        let prefix = format!("/model {}/", prov);
                                        app.typed_input = prefix.chars().collect();
                                        app.cursor_idx = app.typed_input.len();
                                    } else {
                                        let selected_model =
                                            crate::channels::model_menu_model_name(
                                                &models[*selected_idx],
                                            )
                                            .to_string();
                                        let prov = provider_name.clone();
                                        let prov_display_str = provider_display.clone();

                                        match apply_session_model_selection(
                                            &agent_loop,
                                            &session_manager,
                                            &session_key,
                                            &marker_dir,
                                            &prov,
                                            &selected_model,
                                        )
                                        .await
                                        {
                                            Ok(()) => {
                                                app.model = selected_model.clone();
                                                app.provider = prov.clone();
                                                app.messages.push(ChatMessage::simple(
                                                    "assistant",
                                                    format!(
                                                        "✓ Switched this session to {} ({})",
                                                        selected_model, prov_display_str
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
                                        app.model_select = app::ModelSelectState::Closed;
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }
                        _ => {}
                    }

                    let matches = app.matching_slash_commands();
                    let has_matches = !matches.is_empty();

                    match key.code {
                        KeyCode::Esc => {
                            app.typed_input.clear();
                            app.cursor_idx = 0;
                            app.selected_index = None;
                            app.history_idx = None;
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
                                    }
                                } else {
                                    app.selected_index = Some(matches.len() - 1);
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
                                app.scroll_offset = app.scroll_offset.saturating_add(1);
                            }
                        }
                        KeyCode::Down => {
                            if has_matches {
                                if let Some(idx) = app.selected_index {
                                    if idx + 1 < matches.len() {
                                        app.selected_index = Some(idx + 1);
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
                                app.scroll_offset = app.scroll_offset.saturating_sub(1);
                            }
                        }
                        KeyCode::PageUp => {
                            app.scroll_offset = app.scroll_offset.saturating_add(5);
                        }
                        KeyCode::PageDown => {
                            app.scroll_offset = app.scroll_offset.saturating_sub(5);
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

                            // Handle autocomplete selection for model commands
                            let is_menu_selection = app.selected_index.is_some();
                            if is_menu_selection && (trimmed == "/model" || trimmed == "/models") {
                                use crate::config::loader::load_config;
                                let config = load_config().unwrap_or_default();
                                let configured = app::build_configured_providers(&config);
                                app.model_select = app::ModelSelectState::ChoosingProvider {
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
                                app.typed_input.clear();
                                app.cursor_idx = 0;
                                app.selected_index = None;
                                app.history_idx = None;
                                continue;
                            }

                            if is_menu_selection
                                && (trimmed.starts_with("/model ")
                                    || trimmed.starts_with("/models "))
                            {
                                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                if parts.len() == 2 {
                                    let prov = parts[1];
                                    if let Some(reg) =
                                        app::PROVIDER_REGISTRY.iter().find(|r| r.name == prov)
                                    {
                                        // Go to FetchingModels state and spawn async fetch
                                        let prov_name = reg.name.to_string();
                                        let prov_display = reg.display.to_string();
                                        app.model_select = app::ModelSelectState::FetchingModels {
                                            provider_name: prov_name.clone(),
                                            provider_display: prov_display.clone(),
                                        };
                                        let fetch_tx = model_tx.clone();
                                        tokio::spawn(async move {
                                            let config = crate::config::loader::load_config()
                                                .unwrap_or_default();
                                            let mut models = Vec::new();
                                            if let Some(api_models) =
                                                crate::channels::fetch_provider_models(
                                                    &prov_name, &config,
                                                )
                                                .await
                                            {
                                                models = api_models;
                                            }
                                            let curated = app::curated_models_for(&prov_name);
                                            for m in curated {
                                                if !models
                                                    .iter()
                                                    .any(|e| e.eq_ignore_ascii_case(&m))
                                                {
                                                    models.push(m);
                                                }
                                            }
                                            models = crate::channels::model_menu_options_with_prefs(
                                                &prov_name, models,
                                            );
                                            let _ =
                                                fetch_tx.send((prov_name, prov_display, models));
                                        });
                                        app.typed_input.clear();
                                        app.cursor_idx = 0;
                                        app.selected_index = None;
                                        app.history_idx = None;
                                        continue;
                                    } else {
                                        // Check custom providers
                                        let config = crate::config::loader::load_config()
                                            .unwrap_or_default();
                                        if config.is_provider_available(prov) {
                                            let prov_name = prov.to_string();
                                            let prov_display = format!("Custom: {}", prov);
                                            app.model_select =
                                                app::ModelSelectState::FetchingModels {
                                                    provider_name: prov_name.clone(),
                                                    provider_display: prov_display.clone(),
                                                };
                                            let fetch_tx = model_tx.clone();
                                            tokio::spawn(async move {
                                                let config = crate::config::loader::load_config()
                                                    .unwrap_or_default();
                                                let mut models = Vec::new();
                                                if let Some(api_models) =
                                                    crate::channels::fetch_provider_models(
                                                        &prov_name, &config,
                                                    )
                                                    .await
                                                {
                                                    models = api_models;
                                                }
                                                if models.is_empty() {
                                                    if let Some(dm) = config
                                                        .custom_provider_default_model(&prov_name)
                                                    {
                                                        models.push(dm);
                                                    }
                                                }
                                                let _ = fetch_tx.send((
                                                    prov_name,
                                                    prov_display,
                                                    models,
                                                ));
                                            });
                                            app.typed_input.clear();
                                            app.cursor_idx = 0;
                                            app.selected_index = None;
                                            app.history_idx = None;
                                            continue;
                                        }
                                    }
                                }
                            }

                            // Handle partial auto-completions for commands expecting arguments
                            let is_partial = (trimmed == "/load")
                                || (trimmed == "/sources")
                                || (trimmed == "/workflows");

                            if is_menu_selection && is_partial {
                                let mut completed = trimmed.to_string();
                                if !completed.ends_with(' ') {
                                    completed.push(' ');
                                }
                                app.typed_input = completed.chars().collect();
                                app.cursor_idx = app.typed_input.len();
                                app.selected_index = None;
                            } else if !trimmed.is_empty() {
                                app.prompt_history.push(input_str.clone());

                                if trimmed == "/exit" || trimmed == "/quit" {
                                    break;
                                } else if trimmed == "/clear" {
                                    app.messages.clear();
                                    app.scroll_offset = 0;
                                } else if trimmed == "/help" {
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    let mut help_msg = String::from("Available Slash Commands:\n");
                                    for (cmd, desc) in app::SLASH_COMMANDS {
                                        help_msg.push_str(&format!("  {:<18} {}\n", cmd, desc));
                                    }
                                    help_msg.push_str(
                                        "  /load <key>        Load a session from history\n",
                                    );
                                    app.messages
                                        .push(ChatMessage::simple("assistant", help_msg));
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
                                                "  • {} ({}) - {}\n",
                                                name, status, mcp_cfg.command
                                            ));
                                        }
                                    }
                                    app.messages.push(ChatMessage::simple("assistant", mcp_msg));
                                } else if trimmed == "/settings" {
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    let mut settings_msg = String::from("Active Settings:\n");
                                    settings_msg
                                        .push_str(&format!("  Model:          {}\n", app.model));
                                    settings_msg
                                        .push_str(&format!("  Provider:       {}\n", app.provider));
                                    settings_msg.push_str(&format!(
                                        "  CWD:            {}\n",
                                        app.cwd_display
                                    ));
                                    settings_msg.push_str(&format!(
                                        "  Session Key:    {}\n",
                                        app.session_key
                                    ));
                                    app.messages
                                        .push(ChatMessage::simple("assistant", settings_msg));
                                } else if trimmed == "/skill" {
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    let mut skill_msg = String::new();
                                    match crate::agent::skills::load_skills() {
                                        Ok(skills) => {
                                            if skills.is_empty() {
                                                skill_msg.push_str(
                                                    "No active skills found in ~/.openz/skills\n",
                                                );
                                            } else {
                                                skill_msg.push_str("Active skills:\n");
                                                for skill in skills {
                                                    skill_msg
                                                        .push_str(&format!("  • {}\n", skill.name));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            skill_msg.push_str(&format!(
                                                "Error loading skills: {}\n",
                                                e
                                            ));
                                        }
                                    }
                                    app.messages
                                        .push(ChatMessage::simple("assistant", skill_msg));
                                } else if trimmed == "/new-session" {
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    let _ = crate::cli::archive_current_session(
                                        &session_manager,
                                        &session_key,
                                    )
                                    .await;
                                    app.messages.clear();
                                    app.scroll_offset = 0;
                                    app.messages.push(ChatMessage::simple(
                                        "assistant",
                                        "Session reset. Started a new session.".to_string(),
                                    ));
                                } else if trimmed == "/memory" {
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    let mut memory_msg = String::new();
                                    if let Ok(session) = session_manager.load(&session_key) {
                                        memory_msg.push_str("Session Metadata & Memory:\n");
                                        if session.metadata.is_empty() {
                                            memory_msg.push_str("  No memory or metadata recorded for this session.\n");
                                        } else {
                                            for (k, v) in &session.metadata {
                                                memory_msg.push_str(&format!("  • {}: {}\n", k, v));
                                            }
                                        }
                                    } else {
                                        memory_msg.push_str("No active session found.\n");
                                    }
                                    app.messages
                                        .push(ChatMessage::simple("assistant", memory_msg));
                                } else if trimmed == "/streaming" {
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
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
                                                .map(|loop_lock| {
                                                    loop_lock.config.agents.defaults.streaming
                                                })
                                                .unwrap_or(config.agents.defaults.streaming)
                                        });
                                    let next_streaming = !current_streaming;
                                    let msg = match save_session_streaming_override(
                                        &session_manager,
                                        &session_key,
                                        next_streaming,
                                    )
                                    .await
                                    {
                                        Ok(()) => {
                                            if let Ok(mut loop_lock) = agent_loop.try_lock() {
                                                loop_lock.config.agents.defaults.streaming =
                                                    next_streaming;
                                                if let Some(ref mut tool_context) =
                                                    loop_lock.tools.context
                                                {
                                                    tool_context.0.agents.defaults.streaming =
                                                        next_streaming;
                                                }
                                            }
                                            format!(
                                                "Response streaming is now {} for this session.",
                                                if next_streaming {
                                                    "enabled"
                                                } else {
                                                    "disabled"
                                                }
                                            )
                                        }
                                        Err(e) => format!(
                                            "Failed to save session streaming preference: {}",
                                            e
                                        ),
                                    };
                                    app.messages.push(ChatMessage::simple("assistant", msg));
                                } else if trimmed == "/model" || trimmed == "/models" {
                                    use crate::config::loader::load_config;
                                    let config = load_config().unwrap_or_default();
                                    let configured = app::build_configured_providers(&config);
                                    app.model_select = app::ModelSelectState::ChoosingProvider {
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
                                } else if trimmed.starts_with("/model ")
                                    || trimmed.starts_with("/models ")
                                {
                                    let model_arg = trimmed
                                        .strip_prefix("/models")
                                        .or_else(|| trimmed.strip_prefix("/model"))
                                        .unwrap_or("")
                                        .trim();
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    if model_arg.is_empty() {
                                        app.messages.push(ChatMessage::simple("assistant", format!("Active Model: {}\nUse '/model' to open the interactive selector.", app.model)));
                                    } else {
                                        let (prov, mdl) = if let Some(idx) = model_arg.find('/') {
                                            (&model_arg[..idx], &model_arg[idx + 1..])
                                        } else {
                                            ("", model_arg)
                                        };

                                        if prov.is_empty() {
                                            app.messages.push(ChatMessage::simple("assistant", "Error: Please specify provider, e.g. '/model openai/gpt-4o'.".to_string()));
                                        } else {
                                            match apply_session_model_selection(
                                                &agent_loop,
                                                &session_manager,
                                                &session_key,
                                                &marker_dir,
                                                prov,
                                                mdl,
                                            )
                                            .await
                                            {
                                                Ok(()) => {
                                                    app.model = mdl.to_string();
                                                    app.provider = prov.to_string();
                                                    app.messages.push(ChatMessage::simple(
                                                        "assistant",
                                                        format!(
                                                            "✓ Switched this session to: {} ({})",
                                                            mdl, prov
                                                        ),
                                                    ));
                                                }
                                                Err(e) => {
                                                    app.messages.push(ChatMessage::simple(
                                                        "assistant",
                                                        format!("Error switching model: {}", e),
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                } else if trimmed == "/history" {
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    let mut hist_msg = String::from("Available Sessions:\n");
                                    match crate::cli::load_session_history() {
                                        Ok(history) => {
                                            if history.is_empty() {
                                                hist_msg.push_str("  No session history found.\n");
                                            } else {
                                                for item in history {
                                                    hist_msg.push_str(&format!(
                                                        "  • Key: {} | Title: {}\n",
                                                        item.key, item.display_title
                                                    ));
                                                }
                                                hist_msg.push_str("\nTo load a session, use: /load <session_key>\n");
                                            }
                                        }
                                        Err(e) => {
                                            hist_msg.push_str(&format!(
                                                "  Error loading history: {}\n",
                                                e
                                            ));
                                        }
                                    }
                                    app.messages
                                        .push(ChatMessage::simple("assistant", hist_msg));
                                } else if trimmed.starts_with("/load") {
                                    let key = trimmed.strip_prefix("/load").unwrap_or("").trim();
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    if key.is_empty() {
                                        app.messages.push(ChatMessage::simple(
                                            "assistant",
                                            "Usage: /load <session_key>".to_string(),
                                        ));
                                    } else {
                                        let _ = crate::cli::archive_current_session(
                                            &session_manager,
                                            &session_key,
                                        )
                                        .await;
                                        match session_manager.load(key) {
                                            Ok(session) => {
                                                app.messages.clear();
                                                app.scroll_offset = 0;
                                                for msg in session.messages {
                                                    app.messages.push(ChatMessage::simple(
                                                        &msg.role,
                                                        msg.content,
                                                    ));
                                                }
                                                app.messages.push(ChatMessage::simple(
                                                    "assistant",
                                                    format!("Loaded session: {}", key),
                                                ));
                                            }
                                            Err(e) => {
                                                app.messages.push(ChatMessage::simple(
                                                    "assistant",
                                                    format!("Error loading session {}: {}", key, e),
                                                ));
                                            }
                                        }
                                    }
                                } else if trimmed.starts_with("/sources") {
                                    let query =
                                        trimmed.strip_prefix("/sources").unwrap_or("").trim();
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    let mut sources_msg = String::new();
                                    match crate::tools::shared_memory::search_source_bookmarks(
                                        query, 10,
                                    )
                                    .await
                                    {
                                        Ok(items) if items.is_empty() => {
                                            sources_msg.push_str("No saved sources matched.\n");
                                        }
                                        Ok(items) => {
                                            sources_msg.push_str("Saved sources:\n");
                                            for item in items {
                                                sources_msg.push_str(&format!(
                                                    "  • {} [{}] {}\n",
                                                    item.label, item.kind, item.uri
                                                ));
                                                if !item.summary.trim().is_empty() {
                                                    sources_msg.push_str(&format!(
                                                        "    {}\n",
                                                        item.summary.trim()
                                                    ));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            sources_msg.push_str(&format!(
                                                "Error searching sources: {}\n",
                                                e
                                            ));
                                        }
                                    }
                                    app.messages
                                        .push(ChatMessage::simple("assistant", sources_msg));
                                } else if trimmed.starts_with("/workflows") {
                                    let query =
                                        trimmed.strip_prefix("/workflows").unwrap_or("").trim();
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    let mut workflows_msg = String::new();
                                    match crate::tools::shared_memory::search_workflow_cards(
                                        query, 10, false,
                                    )
                                    .await
                                    {
                                        Ok(items) if items.is_empty() => {
                                            workflows_msg
                                                .push_str("No reusable workflows matched.\n");
                                        }
                                        Ok(items) => {
                                            workflows_msg.push_str("Reusable workflows:\n");
                                            for item in items {
                                                workflows_msg.push_str(&format!(
                                                    "  • {} [{}] success={} failure={}\n    {}\n",
                                                    item.name,
                                                    item.status,
                                                    item.success_count,
                                                    item.failure_count,
                                                    item.summary.trim()
                                                ));
                                            }
                                        }
                                        Err(e) => {
                                            workflows_msg.push_str(&format!(
                                                "Error searching workflows: {}\n",
                                                e
                                            ));
                                        }
                                    }
                                    app.messages
                                        .push(ChatMessage::simple("assistant", workflows_msg));
                                } else if trimmed.starts_with('/') {
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    app.messages.push(ChatMessage::simple(
                                        "assistant",
                                        format!(
                                            "Executed slash command: {}. Type /help for options.",
                                            trimmed
                                        ),
                                    ));
                                } else {
                                    // Standard User Prompt -> Dispatch to AgentLoop
                                    app.messages
                                        .push(ChatMessage::simple("user", input_str.clone()));
                                    app.messages.push(ChatMessage::simple(
                                        "assistant",
                                        "⏳ OpenZ is thinking...".to_string(),
                                    ));
                                    app.is_thinking = true;

                                    let agent_loop_clone = agent_loop.clone();
                                    let session_key_clone = session_key.clone();
                                    let prompt_text = input_str.clone();
                                    let tx_clone = tx.clone();

                                    tokio::spawn(async move {
                                        let loop_guard = agent_loop_clone.lock().await;
                                        match loop_guard.run(&prompt_text, &session_key_clone).await
                                        {
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
        if let Err(err) = save_default_model_selection(&app.provider, &app.model) {
            tracing::warn!(error = ?err, "failed to persist last TUI model as default");
        }
    }

    Ok(())
}
