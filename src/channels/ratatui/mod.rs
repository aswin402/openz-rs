pub mod app;
pub mod ui;
pub mod theme;

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
    let model = config.agents.defaults.model.clone();
    let provider = config.agents.defaults.provider.clone();
    let session_key = crate::config::loader::get_cli_session_key();

    let sessions_dir = crate::config::loader::resolve_path("~/.openz/sessions");
    let session_manager = crate::session::SessionManager::new(sessions_dir);

    // Build AgentLoop instance wired with full native tool registry and LLM provider
    let agent_loop = Arc::new(crate::cli::builder::build_agent_loop(config.clone()).await?);

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

    let mut app = app::RatatuiApp::new(model, provider, session_key.clone());

    // Load selected session history into Ratatui conversation stream
    if let Ok(session) = session_manager.load(&session_key) {
        for msg in session.messages {
            app.messages.push(ChatMessage::simple(&msg.role, msg.content));
        }
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ChatMessage>();

    loop {
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
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                        break;
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
                                    let (cmd, _) = matches[idx];
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
                            if !trimmed.is_empty() {
                                app.prompt_history.push(input_str.clone());

                                if trimmed == "/exit" || trimmed == "/quit" {
                                    break;
                                } else if trimmed == "/clear" {
                                    app.messages.clear();
                                    app.scroll_offset = 0;
                                } else if trimmed == "/help" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let mut help_msg = String::from("Available Slash Commands:\n");
                                    for (cmd, desc) in app::SLASH_COMMANDS {
                                        help_msg.push_str(&format!("  {:<18} {}\n", cmd, desc));
                                    }
                                    help_msg.push_str("  /load <key>        Load a session from history\n");
                                    app.messages.push(ChatMessage::simple("assistant", help_msg));
                                } else if trimmed == "/mcps" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let mut mcp_msg = String::from("Configured MCP Servers:\n");
                                    if agent_loop.config.mcp_servers.is_empty() {
                                        mcp_msg.push_str("  No MCP servers configured.\n");
                                    } else {
                                        for (name, mcp_cfg) in &agent_loop.config.mcp_servers {
                                            let status = if mcp_cfg.enabled { "enabled" } else { "disabled" };
                                            mcp_msg.push_str(&format!("  • {} ({}) - {}\n", name, status, mcp_cfg.command));
                                        }
                                    }
                                    app.messages.push(ChatMessage::simple("assistant", mcp_msg));
                                } else if trimmed == "/settings" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let mut settings_msg = String::from("Active Settings:\n");
                                    settings_msg.push_str(&format!("  Model:          {}\n", app.model));
                                    settings_msg.push_str(&format!("  Provider:       {}\n", app.provider));
                                    settings_msg.push_str(&format!("  CWD:            {}\n", app.cwd_display));
                                    settings_msg.push_str(&format!("  Session Key:    {}\n", app.session_key));
                                    app.messages.push(ChatMessage::simple("assistant", settings_msg));
                                } else if trimmed == "/skill" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let mut skill_msg = String::new();
                                    match crate::agent::skills::load_skills() {
                                        Ok(skills) => {
                                            if skills.is_empty() {
                                                skill_msg.push_str("No active skills found in ~/.openz/skills\n");
                                            } else {
                                                skill_msg.push_str("Active skills:\n");
                                                for skill in skills {
                                                    skill_msg.push_str(&format!("  • {}\n", skill.name));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            skill_msg.push_str(&format!("Error loading skills: {}\n", e));
                                        }
                                    }
                                    app.messages.push(ChatMessage::simple("assistant", skill_msg));
                                } else if trimmed == "/new-session" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let _ = crate::cli::archive_current_session(&session_manager, &session_key).await;
                                    app.messages.clear();
                                    app.scroll_offset = 0;
                                    app.messages.push(ChatMessage::simple("assistant", "Session reset. Started a new session.".to_string()));
                                } else if trimmed == "/memory" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
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
                                    app.messages.push(ChatMessage::simple("assistant", memory_msg));
                                } else if trimmed == "/streaming" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let mut msg = String::new();
                                    match crate::config::loader::load_config() {
                                        Ok(mut cfg) => {
                                            cfg.agents.defaults.streaming = !cfg.agents.defaults.streaming;
                                            match crate::config::loader::save_config(&cfg) {
                                                Ok(()) => {
                                                    msg.push_str(&format!("Response streaming is now {}.", if cfg.agents.defaults.streaming { "enabled" } else { "disabled" }));
                                                }
                                                Err(e) => {
                                                    msg.push_str(&format!("Failed to save config: {}", e));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            msg.push_str(&format!("Failed to load config: {}", e));
                                        }
                                    }
                                    app.messages.push(ChatMessage::simple("assistant", msg));
                                } else if trimmed.starts_with("/model") {
                                    let model_arg = trimmed.strip_prefix("/model").unwrap_or("").trim();
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    if model_arg.is_empty() {
                                        app.messages.push(ChatMessage::simple("assistant", format!("Active Model: {}\nUse '/model <name>' to switch it.", app.model)));
                                    } else {
                                        app.model = model_arg.to_string();
                                        app.messages.push(ChatMessage::simple("assistant", format!("Switched active model to: {}", model_arg)));
                                    }
                                } else if trimmed == "/history" {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let mut hist_msg = String::from("Available Sessions:\n");
                                    match crate::cli::load_session_history() {
                                        Ok(history) => {
                                            if history.is_empty() {
                                                hist_msg.push_str("  No session history found.\n");
                                            } else {
                                                for item in history {
                                                    hist_msg.push_str(&format!("  • Key: {} | Title: {}\n", item.key, item.display_title));
                                                }
                                                hist_msg.push_str("\nTo load a session, use: /load <session_key>\n");
                                            }
                                        }
                                        Err(e) => {
                                            hist_msg.push_str(&format!("  Error loading history: {}\n", e));
                                        }
                                    }
                                    app.messages.push(ChatMessage::simple("assistant", hist_msg));
                                } else if trimmed.starts_with("/load") {
                                    let key = trimmed.strip_prefix("/load").unwrap_or("").trim();
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    if key.is_empty() {
                                        app.messages.push(ChatMessage::simple("assistant", "Usage: /load <session_key>".to_string()));
                                    } else {
                                        let _ = crate::cli::archive_current_session(&session_manager, &session_key).await;
                                        match session_manager.load(key) {
                                            Ok(session) => {
                                                app.messages.clear();
                                                app.scroll_offset = 0;
                                                for msg in session.messages {
                                                    app.messages.push(ChatMessage::simple(&msg.role, msg.content));
                                                }
                                                app.messages.push(ChatMessage::simple("assistant", format!("Loaded session: {}", key)));
                                            }
                                            Err(e) => {
                                                app.messages.push(ChatMessage::simple("assistant", format!("Error loading session {}: {}", key, e)));
                                            }
                                        }
                                    }
                                } else if trimmed.starts_with("/sources") {
                                    let query = trimmed.strip_prefix("/sources").unwrap_or("").trim();
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let mut sources_msg = String::new();
                                    match crate::tools::shared_memory::search_source_bookmarks(query, 10).await {
                                        Ok(items) if items.is_empty() => {
                                            sources_msg.push_str("No saved sources matched.\n");
                                        }
                                        Ok(items) => {
                                            sources_msg.push_str("Saved sources:\n");
                                            for item in items {
                                                sources_msg.push_str(&format!("  • {} [{}] {}\n", item.label, item.kind, item.uri));
                                                if !item.summary.trim().is_empty() {
                                                    sources_msg.push_str(&format!("    {}\n", item.summary.trim()));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            sources_msg.push_str(&format!("Error searching sources: {}\n", e));
                                        }
                                    }
                                    app.messages.push(ChatMessage::simple("assistant", sources_msg));
                                } else if trimmed.starts_with("/workflows") {
                                    let query = trimmed.strip_prefix("/workflows").unwrap_or("").trim();
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    let mut workflows_msg = String::new();
                                    match crate::tools::shared_memory::search_workflow_cards(query, 10, false).await {
                                        Ok(items) if items.is_empty() => {
                                            workflows_msg.push_str("No reusable workflows matched.\n");
                                        }
                                        Ok(items) => {
                                            workflows_msg.push_str("Reusable workflows:\n");
                                            for item in items {
                                                workflows_msg.push_str(&format!(
                                                    "  • {} [{}] success={} failure={}\n    {}\n",
                                                    item.name, item.status, item.success_count, item.failure_count, item.summary.trim()
                                                ));
                                            }
                                        }
                                        Err(e) => {
                                            workflows_msg.push_str(&format!("Error searching workflows: {}\n", e));
                                        }
                                    }
                                    app.messages.push(ChatMessage::simple("assistant", workflows_msg));
                                } else if trimmed.starts_with('/') {
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    app.messages.push(ChatMessage::simple("assistant", format!("Executed slash command: {}. Type /help for options.", trimmed)));
                                } else {
                                    // Standard User Prompt -> Dispatch to AgentLoop
                                    app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                    app.messages.push(ChatMessage::simple("assistant", "⏳ OpenZ is thinking...".to_string()));
                                    app.is_thinking = true;

                                    let agent_loop_clone = agent_loop.clone();
                                    let session_key_clone = session_key.clone();
                                    let prompt_text = input_str.clone();
                                    let tx_clone = tx.clone();

                                    tokio::spawn(async move {
                                        match agent_loop_clone.run(&prompt_text, &session_key_clone).await {
                                            Ok(res) => {
                                                let _ = tx_clone.send(ChatMessage::simple("assistant", res.content));
                                            }
                                            Err(err) => {
                                                let _ = tx_clone.send(ChatMessage::simple("assistant", format!("⚠ Error: {}", err)));
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

    Ok(())
}
