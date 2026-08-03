pub mod app;
pub mod ui;

use anyhow::Result;
use app::{ChatMessage, RatatuiApp, IS_RATATUI_ACTIVE};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::atomic::Ordering;
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
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    IS_RATATUI_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = RatatuiGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let config = crate::config::loader::load_config().unwrap_or_default();
    let model = config.agents.defaults.model.clone();
    let provider = config.agents.defaults.provider.clone();
    let session_key = crate::config::loader::get_cli_session_key();

    let sessions_dir = crate::config::loader::resolve_path("~/.openz/sessions");
    let session_manager = crate::session::SessionManager::new(sessions_dir);

    let mut app = app::RatatuiApp::new(model, provider, session_key.clone());

    // Load past session history from SessionManager if present
    if let Ok(session) = session_manager.load(&session_key) {
        for msg in session.messages {
            let is_tool = msg.role == "tool";
            app.messages.push(ChatMessage {
                role: msg.role,
                content: msg.content,
                is_tool,
            });
        }
    }

    loop {
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
                                    app.messages.push(ChatMessage {
                                        role: "user".to_string(),
                                        content: input_str.clone(),
                                        is_tool: false,
                                    });
                                    let mut help_msg = String::from("Available Slash Commands:\n");
                                    for (cmd, desc) in app::SLASH_COMMANDS {
                                        help_msg.push_str(&format!("  {:<18} {}\n", cmd, desc));
                                    }
                                    app.messages.push(ChatMessage {
                                        role: "assistant".to_string(),
                                        content: help_msg,
                                        is_tool: false,
                                    });
                                } else if trimmed == "/mcps" {
                                    app.messages.push(ChatMessage {
                                        role: "user".to_string(),
                                        content: input_str.clone(),
                                        is_tool: false,
                                    });
                                    let (loaded, total, _) = crate::tools::mcp::get_mcp_stats();
                                    app.messages.push(ChatMessage {
                                        role: "assistant".to_string(),
                                        content: format!("MCP Servers Status: {}/{} connected & active.", loaded, total),
                                        is_tool: false,
                                    });
                                } else {
                                    app.messages.push(ChatMessage {
                                        role: "user".to_string(),
                                        content: input_str,
                                        is_tool: false,
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
