use crate::agent::style::markdown::terminal_width_or_default;
use crate::agent::style::*;
use crate::print;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

pub static IS_RAW_INPUT_ACTIVE: AtomicBool = AtomicBool::new(false);

struct RawInputGuard;

impl Drop for RawInputGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(stdout(), crossterm::event::DisableBracketedPaste);
        let _ = crossterm::execute!(stdout(), crossterm::cursor::Show);
        let _ = crossterm::terminal::disable_raw_mode();
        IS_RAW_INPUT_ACTIVE.store(false, Ordering::SeqCst);
    }
}

pub(super) fn is_ctrl_exit_key(key_event: &crossterm::event::KeyEvent) -> bool {
    match key_event.kind == KeyEventKind::Release {
        true => false,
        false => match key_event.code {
            KeyCode::Char('\u{3}') | KeyCode::Char('\u{4}') => true,
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('d') | KeyCode::Char('D') => {
                key_event.modifiers.contains(KeyModifiers::CONTROL)
            }
            _ => false,
        },
    }
}

pub(super) fn is_turn_cancel_key(key_event: &crossterm::event::KeyEvent) -> bool {
    match key_event.kind == KeyEventKind::Release {
        true => false,
        false => key_event.code == KeyCode::Esc || is_ctrl_exit_key(key_event),
    }
}

fn is_paste_image_shortcut(key_event: &crossterm::event::KeyEvent) -> bool {
    let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key_event.modifiers.contains(KeyModifiers::ALT);
    let shift = key_event.modifiers.contains(KeyModifiers::SHIFT);
    let is_v = matches!(key_event.code, KeyCode::Char('v') | KeyCode::Char('V'));
    is_v && !shift && (ctrl || alt)
}

fn format_image_placeholder(index: usize) -> String {
    match index == 0 {
        true => "[image]".to_string(),
        false => format!("[image{}]", index),
    }
}

pub(super) fn handle_clipboard_paste(index: usize) -> Result<PathBuf> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new()?;
    let image = clipboard.get_image()?;

    let path = crate::config::runtime_data_dir().join(format!("clipboard_image_{}.png", index));
    match path.parent() {
        Some(parent) => std::fs::create_dir_all(parent)?,
        None => {}
    }

    image::save_buffer(
        &path,
        &image.bytes,
        image.width as u32,
        image.height as u32,
        image::ColorType::Rgba8,
    )?;

    Ok(path)
}

struct InputEditorState {
    typed_input: Vec<char>,
    cursor_idx: usize,
    viewport_start: usize,
    selected_index: Option<usize>,
    history_index: Option<usize>,
    temp_typed_input: String,
    pasted_images: Vec<PathBuf>,
    width_usize: usize,
    lines_printed: usize,
    autocomplete_visible: bool,
}

impl InputEditorState {
    fn new() -> Self {
        Self {
            typed_input: Vec::new(),
            cursor_idx: 0,
            viewport_start: 0,
            selected_index: None,
            history_index: None,
            temp_typed_input: String::new(),
            pasted_images: Vec::new(),
            width_usize: terminal_width_or_default(80),
            lines_printed: 1,
            autocomplete_visible: true,
        }
    }

    fn text(&self) -> String {
        self.typed_input.iter().collect()
    }

    fn set_text(&mut self, value: &str) {
        self.typed_input = value.chars().collect();
        self.cursor_idx = self.typed_input.len();
    }

    fn refresh_terminal_width(&mut self) {
        self.width_usize = terminal_width_or_default(80);
    }

    fn redraw(
        &mut self,
        model: &str,
        provider: &str,
        session_manager: &crate::session::SessionManager,
        session_key: &str,
    ) -> Result<()> {
        let typed_input_str = self.text();
        super::render::render_box(
            model,
            provider,
            session_manager,
            session_key,
            &typed_input_str,
            self.cursor_idx,
            &mut self.viewport_start,
            self.selected_index,
            self.autocomplete_visible,
            self.width_usize,
            &mut self.lines_printed,
        )
    }

    fn slash_matches(&self) -> Vec<(&'static str, &'static str)> {
        let typed_input_str = self.text();
        match typed_input_str.starts_with('/') && !typed_input_str.contains(' ') {
            true => super::render::SLASH_COMMANDS
                .iter()
                .filter(|&&(cmd, _)| cmd.starts_with(&typed_input_str))
                .copied()
                .collect(),
            false => Vec::new(),
        }
    }

    fn apply_selected_slash_command(&mut self) {
        match self.selected_index {
            Some(idx) => {
                let matches = self.slash_matches();
                match matches.get(idx) {
                    Some(&(cmd, _)) => self.set_text(cmd),
                    None => {}
                }
                self.selected_index = None;
            }
            None => {}
        }
    }

    fn step_history_up(
        &mut self,
        session_manager: &crate::session::SessionManager,
        session_key: &str,
    ) -> bool {
        let session = session_manager.get_or_create(session_key);
        let history_prompts: Vec<String> = session
            .messages
            .iter()
            .filter(|m| m.role == "user")
            .map(|m| m.content.clone())
            .collect();
        match history_prompts.is_empty() {
            true => false,
            false => {
                match self.history_index {
                    None => {
                        self.temp_typed_input = self.text();
                        self.history_index = Some(history_prompts.len() - 1);
                    }
                    Some(idx) => match idx > 0 {
                        true => self.history_index = Some(idx - 1),
                        false => {}
                    },
                }
                match self.history_index {
                    Some(idx) => {
                        self.set_text(&history_prompts[idx]);
                        self.selected_index = None;
                        true
                    }
                    None => false,
                }
            }
        }
    }

    fn step_history_down(
        &mut self,
        session_manager: &crate::session::SessionManager,
        session_key: &str,
    ) -> bool {
        let session = session_manager.get_or_create(session_key);
        let history_prompts: Vec<String> = session
            .messages
            .iter()
            .filter(|m| m.role == "user")
            .map(|m| m.content.clone())
            .collect();
        match (history_prompts.is_empty(), self.history_index) {
            (false, Some(idx)) => {
                match idx + 1 < history_prompts.len() {
                    true => {
                        self.history_index = Some(idx + 1);
                        self.set_text(&history_prompts[idx + 1]);
                    }
                    false => {
                        self.history_index = None;
                        let restored = self.temp_typed_input.clone();
                        self.set_text(&restored);
                    }
                }
                self.selected_index = None;
                true
            }
            _ => false,
        }
    }
}

fn check_shutdown_signal() -> bool {
    match crate::shutdown::receiver() {
        Some(rx) => *rx.borrow(),
        None => false,
    }
}

fn drain_notifications() -> Vec<String> {
    match super::mcp::get_pending_notifications().lock() {
        Ok(mut pending) => match pending.is_empty() {
            true => Vec::new(),
            false => std::mem::take(&mut *pending),
        },
        Err(_) => Vec::new(),
    }
}

fn pop_remote_inbox_message(session_key: &str) -> Option<crate::agent::activity::InboxMessage> {
    crate::agent::activity::pop_inbox_message(session_key).or_else(|| {
        match crate::agent::activity::direct_inbox_belongs_to(session_key) {
            true => crate::agent::activity::pop_inbox_message("cli:direct"),
            false => None,
        }
    })
}

fn print_submitted_prompt(final_cmd_str: &str, width_usize: usize) {
    let display_chars: Vec<char> = final_cmd_str.chars().collect();
    let max_line_content_width = width_usize.saturating_sub(2).max(1);

    let mut submit_lines = Vec::new();
    let mut cur_line = String::new();
    let mut cur_w = 0;

    for &c in &display_chars {
        let cw = super::render::char_display_width(c);
        match c == '\n' || (cur_w + cw > max_line_content_width && !cur_line.is_empty()) {
            true => {
                submit_lines.push(cur_line);
                cur_line = String::new();
                cur_w = 0;
            }
            false => {}
        }
        match c != '\n' {
            true => {
                cur_line.push(c);
                cur_w += cw;
            }
            false => {}
        }
    }
    submit_lines.push(cur_line);

    for (i, line) in submit_lines.iter().enumerate() {
        let prefix = match i == 0 {
            true => "> ",
            false => "- ",
        };
        match i > 0 {
            true => print!("\r\n"),
            false => {}
        }
        print!("{}{}{}{}", COLOR_BOLD, AURA_SLATE, prefix, line);
    }
    print!("{}\r\n", COLOR_RESET);
}

pub fn read_line_raw(
    model: &str,
    provider: &str,
    session_manager: &crate::session::SessionManager,
    session_key: &str,
) -> Result<(String, Option<String>)> {
    enable_raw_mode()?;
    let _ = crossterm::execute!(
        stdout(),
        crossterm::event::EnableBracketedPaste,
        crossterm::cursor::Show
    );
    super::render::LAST_CURSOR_LINE.store(0, Ordering::Relaxed);
    IS_RAW_INPUT_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = RawInputGuard;

    let mut state = InputEditorState::new();
    state.redraw(model, provider, session_manager, session_key)?;

    let mut last_mcp_done = super::mcp::is_mcp_done();

    loop {
        match check_shutdown_signal() {
            true => {
                super::render::clear_input_rendering(state.lines_printed);
                let _ = crossterm::execute!(stdout(), crossterm::event::DisableBracketedPaste);
                let _ = disable_raw_mode();
                return Ok(("/exit".to_string(), None));
            }
            false => {}
        }

        let notifications = drain_notifications();
        match notifications.is_empty() {
            true => {}
            false => {
                super::render::clear_input_rendering(state.lines_printed);
                let _ = stdout().flush();

                for notif in notifications {
                    let formatted_notif = notif.replace('\n', "\r\n");
                    print!("{}\r\n", formatted_notif);
                }
                let _ = stdout().flush();

                state.lines_printed = 1;
                super::render::LAST_CURSOR_LINE.store(0, Ordering::Relaxed);
                state.redraw(model, provider, session_manager, session_key)?;
            }
        }

        match pop_remote_inbox_message(session_key) {
            Some(inbox_msg) => {
                super::render::clear_input_rendering(state.lines_printed);
                let _ = stdout().flush();
                let _ = crossterm::execute!(stdout(), crossterm::event::DisableBracketedPaste);
                let _ = disable_raw_mode();
                println!(
                    "\r\n{}🔌 [Remote Control] Received prompt: {}{}",
                    AURA_BLUE, inbox_msg.message, COLOR_RESET
                );
                return Ok((inbox_msg.message, Some(inbox_msg.sender)));
            }
            None => {}
        }

        let current_mcp_done = super::mcp::is_mcp_done();
        let mcp_status_changed = current_mcp_done != last_mcp_done;
        last_mcp_done = current_mcp_done;

        match event::poll(std::time::Duration::from_millis(100))? {
            false => {
                match !current_mcp_done || mcp_status_changed {
                    true => state.redraw(model, provider, session_manager, session_key)?,
                    false => {}
                }
                continue;
            }
            true => {}
        }

        match event::read()? {
            Event::Paste(text) => {
                let cleaned_text = text.replace('\r', "").replace('\n', " ");
                for c in cleaned_text.chars() {
                    state.typed_input.insert(state.cursor_idx, c);
                    state.cursor_idx += 1;
                }
                state.history_index = None;
                state.selected_index = None;
                state.refresh_terminal_width();
                state.redraw(model, provider, session_manager, session_key)?;
            }
            Event::Key(key_event) => {
                match key_event.kind == KeyEventKind::Release {
                    true => continue,
                    false => {}
                }

                match is_ctrl_exit_key(&key_event) {
                    true => {
                        super::render::clear_input_rendering(state.lines_printed);
                        let _ =
                            crossterm::execute!(stdout(), crossterm::event::DisableBracketedPaste);
                        let _ = disable_raw_mode();
                        println!();
                        return Ok(("/exit".to_string(), None));
                    }
                    false => {}
                }

                match is_paste_image_shortcut(&key_event) {
                    true => {
                        state.apply_selected_slash_command();
                        state.history_index = None;
                        let next_index = state.pasted_images.len();
                        match handle_clipboard_paste(next_index) {
                            Ok(img_path) => {
                                state.pasted_images.push(img_path);
                                let placeholder = format_image_placeholder(next_index);
                                let space = match state.typed_input.is_empty() {
                                    true => "",
                                    false => " ",
                                };
                                let to_add = format!("{space}{placeholder}");
                                for c in to_add.chars() {
                                    state.typed_input.insert(state.cursor_idx, c);
                                    state.cursor_idx += 1;
                                }
                            }
                            Err(e) => {
                                super::render::clear_input_rendering(state.lines_printed);
                                print!(
                                    "{}✕ Error: No image found in clipboard: {}{}\r\n",
                                    ERROR_RED, e, COLOR_RESET
                                );
                                state.lines_printed = 1;
                                super::render::LAST_CURSOR_LINE.store(0, Ordering::Relaxed);
                            }
                        }
                        state.refresh_terminal_width();
                        state.redraw(model, provider, session_manager, session_key)?;
                        continue;
                    }
                    false => {}
                }

                state.refresh_terminal_width();

                match key_event.code {
                    KeyCode::Up => {
                        state.autocomplete_visible = true;
                        let matches = state.slash_matches();
                        match matches.is_empty() {
                            false => {
                                state.selected_index = match state.selected_index {
                                    None => Some(matches.len() - 1),
                                    Some(idx) => Some((idx + matches.len() - 1) % matches.len()),
                                };
                                state.redraw(model, provider, session_manager, session_key)?;
                            }
                            true => match state.step_history_up(session_manager, session_key) {
                                true => {
                                    state.redraw(model, provider, session_manager, session_key)?
                                }
                                false => {}
                            },
                        }
                    }
                    KeyCode::Down => {
                        state.autocomplete_visible = true;
                        let matches = state.slash_matches();
                        match matches.is_empty() {
                            false => {
                                state.selected_index = match state.selected_index {
                                    None => Some(0),
                                    Some(idx) => Some((idx + 1) % matches.len()),
                                };
                                state.redraw(model, provider, session_manager, session_key)?;
                            }
                            true => match state.step_history_down(session_manager, session_key) {
                                true => {
                                    state.redraw(model, provider, session_manager, session_key)?
                                }
                                false => {}
                            },
                        }
                    }
                    KeyCode::Left => match state.cursor_idx > 0 {
                        true => {
                            state.cursor_idx -= 1;
                            state.redraw(model, provider, session_manager, session_key)?;
                        }
                        false => {}
                    },
                    KeyCode::Right => match state.cursor_idx < state.typed_input.len() {
                        true => {
                            state.cursor_idx += 1;
                            state.redraw(model, provider, session_manager, session_key)?;
                        }
                        false => {}
                    },
                    KeyCode::Home => {
                        state.cursor_idx = 0;
                        state.redraw(model, provider, session_manager, session_key)?;
                    }
                    KeyCode::End => {
                        state.cursor_idx = state.typed_input.len();
                        state.redraw(model, provider, session_manager, session_key)?;
                    }
                    KeyCode::Delete => match state.cursor_idx < state.typed_input.len() {
                        true => {
                            state.typed_input.remove(state.cursor_idx);
                            state.selected_index = None;
                            state.history_index = None;
                            state.redraw(model, provider, session_manager, session_key)?;
                        }
                        false => {}
                    },
                    KeyCode::Char(c) => {
                        state.autocomplete_visible = true;
                        state.selected_index = None;
                        state.history_index = None;
                        state.typed_input.insert(state.cursor_idx, c);
                        state.cursor_idx += 1;
                        state.redraw(model, provider, session_manager, session_key)?;
                    }
                    KeyCode::Backspace => {
                        state.autocomplete_visible = true;
                        state.selected_index = None;
                        state.history_index = None;
                        match state.cursor_idx > 0 {
                            true => {
                                state.typed_input.remove(state.cursor_idx - 1);
                                state.cursor_idx -= 1;
                                state.redraw(model, provider, session_manager, session_key)?;
                            }
                            false => {}
                        }
                    }
                    KeyCode::Tab => {
                        state.autocomplete_visible = true;
                        let matches = state.slash_matches();
                        match matches.is_empty() {
                            false => {
                                let completed = match state.selected_index {
                                    Some(idx) => matches.get(idx).unwrap_or(&matches[0]).0,
                                    None => matches[0].0,
                                };
                                state.set_text(completed);
                                state.selected_index = None;
                                state.history_index = None;
                                state.redraw(model, provider, session_manager, session_key)?;
                            }
                            true => {}
                        }
                    }
                    KeyCode::Esc => match state.typed_input.is_empty() {
                        true => {
                            super::render::clear_input_rendering(state.lines_printed);
                            let _ = crossterm::execute!(
                                stdout(),
                                crossterm::event::DisableBracketedPaste
                            );
                            let _ = disable_raw_mode();
                            println!();
                            return Ok(("/exit".to_string(), None));
                        }
                        false => {
                            state.autocomplete_visible = false;
                            state.selected_index = None;
                            state.redraw(model, provider, session_manager, session_key)?;
                        }
                    },
                    KeyCode::Enter => {
                        state.apply_selected_slash_command();
                        super::render::clear_input_rendering(state.lines_printed);
                        let final_cmd_str = state.text();
                        print_submitted_prompt(&final_cmd_str, state.width_usize);

                        let _ =
                            crossterm::execute!(stdout(), crossterm::event::DisableBracketedPaste);
                        let _ = disable_raw_mode();
                        stdout().flush()?;
                        break;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    let mut final_input = state.text();
    for (i, path) in state.pasted_images.iter().enumerate() {
        let placeholder = format_image_placeholder(i);
        let replacement = format!("![](file://{})", path.to_string_lossy());
        final_input = final_input.replace(&placeholder, &replacement);
    }

    Ok((final_input, None))
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
