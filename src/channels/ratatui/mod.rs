pub mod app;
pub mod ui;

use anyhow::Result;
use app::{ChatMessage, IS_RATATUI_ACTIVE};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
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
    let session_key = "default".to_string();

    let mut app = app::RatatuiApp::new(model, provider, session_key);

    loop {
        terminal.draw(|f| ui::render_ratatui_ui(f, &app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                        break;
                    }
                    match key.code {
                        KeyCode::Esc => {
                            app.typed_input.clear();
                            app.cursor_idx = 0;
                        }
                        KeyCode::Char(c) => {
                            app.typed_input.insert(app.cursor_idx, c);
                            app.cursor_idx += 1;
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
                        }
                        KeyCode::Up | KeyCode::PageUp => {
                            app.scroll_offset = app.scroll_offset.saturating_add(1);
                        }
                        KeyCode::Down | KeyCode::PageDown => {
                            app.scroll_offset = app.scroll_offset.saturating_sub(1);
                        }
                        KeyCode::Enter => {
                            let input_str: String = app.typed_input.iter().collect();
                            let trimmed = input_str.trim();
                            if !trimmed.is_empty() {
                                if trimmed == "/exit" || trimmed == "/quit" {
                                    break;
                                } else if trimmed == "/clear" {
                                    app.messages.clear();
                                    app.scroll_offset = 0;
                                } else {
                                    app.messages.push(ChatMessage {
                                        role: "user".to_string(),
                                        content: input_str,
                                        is_tool: false,
                                    });
                                }
                                app.typed_input.clear();
                                app.cursor_idx = 0;
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

