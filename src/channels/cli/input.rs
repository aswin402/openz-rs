use crate::agent::style::*;
use crate::print;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

pub static IS_RAW_INPUT_ACTIVE: AtomicBool = AtomicBool::new(false);

struct RawInputGuard;

impl Drop for RawInputGuard {
    fn drop(&mut self) {
        IS_RAW_INPUT_ACTIVE.store(false, Ordering::SeqCst);
    }
}

pub(super) fn handle_clipboard_paste(index: usize) -> Result<PathBuf> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new()?;
    let image = clipboard.get_image()?;

    let path = crate::config::resolve_path(&format!("~/.openz/clipboard_image_{}.png", index));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
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

pub fn read_line_raw(
    model: &str,
    provider: &str,
    session_manager: &crate::session::SessionManager,
    session_key: &str,
) -> Result<(String, Option<String>)> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    enable_raw_mode()?;
    let _ = crossterm::execute!(stdout(), crossterm::event::EnableBracketedPaste);
    IS_RAW_INPUT_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = RawInputGuard;

    let mut typed_input = Vec::<char>::new();
    let mut cursor_idx = 0;
    let mut viewport_start = 0;
    let mut selected_index: Option<usize> = None;
    let mut history_index: Option<usize> = None;
    let mut temp_typed_input = String::new();
    let mut pasted_images = Vec::new();
    let (width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let mut width_usize = width as usize;
    let mut lines_printed = 1;
    let mut autocomplete_visible = true;

    let typed_input_str: String = typed_input.iter().collect();
    super::render::render_box(
        model,
        provider,
        session_manager,
        session_key,
        &typed_input_str,
        cursor_idx,
        &mut viewport_start,
        selected_index,
        autocomplete_visible,
        width_usize,
        &mut lines_printed,
    )?;

    let mut last_mcp_done = super::mcp::is_mcp_done();

    loop {
        if let Some(rx) = crate::shutdown::receiver() {
            if *rx.borrow() {
                if lines_printed > 1 {
                    for _ in 0..(lines_printed - 1) {
                        print!("\r\n\x1b[2K");
                    }
                    print!("\x1b[{}A\r", lines_printed - 1);
                }
                let _ = crossterm::execute!(stdout(), crossterm::event::DisableBracketedPaste);
                let _ = disable_raw_mode();
                return Ok(("/exit".to_string(), None));
            }
        }
        // Process any pending notifications first
        let mut notifications = Vec::new();
        if let Ok(mut pending) = super::mcp::get_pending_notifications().lock() {
            if !pending.is_empty() {
                notifications = std::mem::take(&mut *pending);
            }
        }

        if !notifications.is_empty() {
            if lines_printed > 1 {
                for _ in 0..(lines_printed - 1) {
                    print!("\r\n\x1b[2K");
                }
                print!("\x1b[{}A\r", lines_printed - 1);
            }
            print!("\r\x1b[2K");
            let _ = stdout().flush();

            for notif in notifications {
                let formatted_notif = notif.replace("\n", "\r\n");
                print!("{}\r\n", formatted_notif);
            }
            let _ = stdout().flush();

            lines_printed = 1;
            let typed_input_str: String = typed_input.iter().collect();
            super::render::render_box(
                model,
                provider,
                session_manager,
                session_key,
                &typed_input_str,
                cursor_idx,
                &mut viewport_start,
                selected_index,
                autocomplete_visible,
                width_usize,
                &mut lines_printed,
            )?;
        }

        if let Some(inbox_msg) = crate::agent::activity::pop_inbox_message(session_key)
            .or_else(|| crate::agent::activity::pop_inbox_message("cli:direct"))
        {
            if lines_printed > 1 {
                for _ in 0..(lines_printed - 1) {
                    print!("\r\n\x1b[2K");
                }
                print!("\x1b[{}A\r", lines_printed - 1);
            }
            print!("\r\x1b[2K");
            let _ = stdout().flush();
            let _ = crossterm::execute!(stdout(), crossterm::event::DisableBracketedPaste);
            let _ = disable_raw_mode();
            println!(
                "\r\n{}🔌 [Remote Control] Received prompt: {}{}",
                AURA_BLUE, inbox_msg.message, COLOR_RESET
            );
            return Ok((inbox_msg.message, Some(inbox_msg.sender)));
        }

        let current_mcp_done = super::mcp::is_mcp_done();
        let mcp_status_changed = current_mcp_done != last_mcp_done;
        if mcp_status_changed {
            last_mcp_done = current_mcp_done;
        }

        if !event::poll(std::time::Duration::from_millis(100))? {
            if !current_mcp_done || mcp_status_changed {
                let typed_input_str: String = typed_input.iter().collect();
                super::render::render_box(
                    model,
                    provider,
                    session_manager,
                    session_key,
                    &typed_input_str,
                    cursor_idx,
                    &mut viewport_start,
                    selected_index,
                    autocomplete_visible,
                    width_usize,
                    &mut lines_printed,
                )?;
            }
            continue;
        }
        let ev = event::read()?;

        // Handle bracketed paste event
        if let Event::Paste(text) = &ev {
            let cleaned_text = text.replace('\r', "").replace('\n', " ");
            for c in cleaned_text.chars() {
                typed_input.insert(cursor_idx, c);
                cursor_idx += 1;
            }
            history_index = None;
            selected_index = None;
            if let Ok((w, _)) = crossterm::terminal::size() {
                width_usize = w as usize;
            }
            let typed_input_str: String = typed_input.iter().collect();
            super::render::render_box(
                model,
                provider,
                session_manager,
                session_key,
                &typed_input_str,
                cursor_idx,
                &mut viewport_start,
                selected_index,
                autocomplete_visible,
                width_usize,
                &mut lines_printed,
            )?;
            continue;
        }

        if let Event::Key(key_event) = ev {
            if key_event.kind == KeyEventKind::Release {
                continue;
            }

            let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
            let alt = key_event.modifiers.contains(KeyModifiers::ALT);
            let shift = key_event.modifiers.contains(KeyModifiers::SHIFT);

            // Ctrl+C or Ctrl+D to exit
            if ctrl
                && (key_event.code == KeyCode::Char('c') || key_event.code == KeyCode::Char('d'))
            {
                if lines_printed > 1 {
                    for _ in 0..(lines_printed - 1) {
                        print!("\r\n\x1b[2K");
                    }
                    print!("\x1b[{}A\r", lines_printed - 1);
                }
                let _ = crossterm::execute!(stdout(), crossterm::event::DisableBracketedPaste);
                let _ = disable_raw_mode();
                println!();
                return Ok(("/exit".to_string(), None));
            }

            // Ctrl+V or Alt+V to paste image (ONLY if shift is not pressed)
            let is_paste_image = (ctrl
                && !shift
                && (key_event.code == KeyCode::Char('v') || key_event.code == KeyCode::Char('V')))
                || (alt
                    && !shift
                    && (key_event.code == KeyCode::Char('v')
                        || key_event.code == KeyCode::Char('V')));
            if is_paste_image {
                if let Some(idx) = selected_index {
                    let typed_input_str: String = typed_input.iter().collect();
                    let matches: Vec<(&str, &str)> = super::render::SLASH_COMMANDS
                        .iter()
                        .filter(|&&(cmd, _)| cmd.starts_with(&typed_input_str))
                        .copied()
                        .collect();
                    if idx < matches.len() {
                        typed_input = matches[idx].0.chars().collect();
                        cursor_idx = typed_input.len();
                    }
                    selected_index = None;
                }
                history_index = None;

                let next_index = pasted_images.len();
                match handle_clipboard_paste(next_index) {
                    Ok(img_path) => {
                        pasted_images.push(img_path);
                        let placeholder = if next_index == 0 {
                            "[image]".to_string()
                        } else {
                            format!("[image{}]", next_index)
                        };
                        let space = if typed_input.is_empty() { "" } else { " " };
                        let to_add = format!("{space}{placeholder}");
                        for c in to_add.chars() {
                            typed_input.insert(cursor_idx, c);
                            cursor_idx += 1;
                        }
                        if let Ok((w, _)) = crossterm::terminal::size() {
                            width_usize = w as usize;
                        }
                        let typed_input_str: String = typed_input.iter().collect();
                        super::render::render_box(
                            model,
                            provider,
                            session_manager,
                            session_key,
                            &typed_input_str,
                            cursor_idx,
                            &mut viewport_start,
                            selected_index,
                            autocomplete_visible,
                            width_usize,
                            &mut lines_printed,
                        )?;
                    }
                    Err(e) => {
                        if lines_printed > 1 {
                            for _ in 0..(lines_printed - 1) {
                                print!("\r\n\x1b[2K");
                            }
                            print!("\x1b[{}A\r", lines_printed - 1);
                        }
                        print!(
                            "\r\n\x1b[2K{}✕ Error: No image found in clipboard: {}{}\r\n",
                            ERROR_RED, e, COLOR_RESET
                        );
                        lines_printed = 1;
                        if let Ok((w, _)) = crossterm::terminal::size() {
                            width_usize = w as usize;
                        }
                        let typed_input_str: String = typed_input.iter().collect();
                        super::render::render_box(
                            model,
                            provider,
                            session_manager,
                            session_key,
                            &typed_input_str,
                            cursor_idx,
                            &mut viewport_start,
                            selected_index,
                            autocomplete_visible,
                            width_usize,
                            &mut lines_printed,
                        )?;
                    }
                }
                continue;
            }

            if let Ok((w, _)) = crossterm::terminal::size() {
                width_usize = w as usize;
            }

            match key_event.code {
                KeyCode::Up => {
                    autocomplete_visible = true;
                    let mut autocomplete_active = false;
                    let typed_input_str: String = typed_input.iter().collect();
                    if typed_input_str.starts_with('/') && !typed_input_str.contains(' ') {
                        let matches: Vec<(&str, &str)> = super::render::SLASH_COMMANDS
                            .iter()
                            .filter(|&&(cmd, _)| cmd.starts_with(&typed_input_str))
                            .copied()
                            .collect();
                        if !matches.is_empty() {
                            autocomplete_active = true;
                            selected_index = match selected_index {
                                None => Some(matches.len() - 1),
                                Some(idx) => Some((idx + matches.len() - 1) % matches.len()),
                            };
                            super::render::render_box(
                                model,
                                provider,
                                session_manager,
                                session_key,
                                &typed_input_str,
                                cursor_idx,
                                &mut viewport_start,
                                selected_index,
                                autocomplete_visible,
                                width_usize,
                                &mut lines_printed,
                            )?;
                        }
                    }

                    if !autocomplete_active {
                        let session = session_manager.get_or_create(session_key);
                        let history_prompts: Vec<String> = session
                            .messages
                            .iter()
                            .filter(|m| m.role == "user")
                            .map(|m| m.content.clone())
                            .collect();
                        if !history_prompts.is_empty() {
                            if history_index.is_none() {
                                temp_typed_input = typed_input.iter().collect::<String>();
                                history_index = Some(history_prompts.len() - 1);
                            } else if let Some(idx) = history_index {
                                if idx > 0 {
                                    history_index = Some(idx - 1);
                                }
                            }
                            if let Some(idx) = history_index {
                                typed_input = history_prompts[idx].chars().collect();
                                cursor_idx = typed_input.len();
                                selected_index = None;
                                let typed_input_str: String = typed_input.iter().collect();
                                super::render::render_box(
                                    model,
                                    provider,
                                    session_manager,
                                    session_key,
                                    &typed_input_str,
                                    cursor_idx,
                                    &mut viewport_start,
                                    selected_index,
                                    autocomplete_visible,
                                    width_usize,
                                    &mut lines_printed,
                                )?;
                            }
                        }
                    }
                }
                KeyCode::Down => {
                    autocomplete_visible = true;
                    let mut autocomplete_active = false;
                    let typed_input_str: String = typed_input.iter().collect();
                    if typed_input_str.starts_with('/') && !typed_input_str.contains(' ') {
                        let matches: Vec<(&str, &str)> = super::render::SLASH_COMMANDS
                            .iter()
                            .filter(|&&(cmd, _)| cmd.starts_with(&typed_input_str))
                            .copied()
                            .collect();
                        if !matches.is_empty() {
                            autocomplete_active = true;
                            selected_index = match selected_index {
                                None => Some(0),
                                Some(idx) => Some((idx + 1) % matches.len()),
                            };
                            super::render::render_box(
                                model,
                                provider,
                                session_manager,
                                session_key,
                                &typed_input_str,
                                cursor_idx,
                                &mut viewport_start,
                                selected_index,
                                autocomplete_visible,
                                width_usize,
                                &mut lines_printed,
                            )?;
                        }
                    }

                    if !autocomplete_active {
                        let session = session_manager.get_or_create(session_key);
                        let history_prompts: Vec<String> = session
                            .messages
                            .iter()
                            .filter(|m| m.role == "user")
                            .map(|m| m.content.clone())
                            .collect();
                        if !history_prompts.is_empty() {
                            if let Some(idx) = history_index {
                                if idx < history_prompts.len() - 1 {
                                    history_index = Some(idx + 1);
                                    typed_input = history_prompts[idx + 1].chars().collect();
                                    cursor_idx = typed_input.len();
                                } else {
                                    history_index = None;
                                    typed_input = temp_typed_input.chars().collect();
                                    cursor_idx = typed_input.len();
                                }
                                selected_index = None;
                                let typed_input_str: String = typed_input.iter().collect();
                                super::render::render_box(
                                    model,
                                    provider,
                                    session_manager,
                                    session_key,
                                    &typed_input_str,
                                    cursor_idx,
                                    &mut viewport_start,
                                    selected_index,
                                    autocomplete_visible,
                                    width_usize,
                                    &mut lines_printed,
                                )?;
                            }
                        }
                    }
                }
                KeyCode::Left => {
                    if cursor_idx > 0 {
                        cursor_idx -= 1;
                        let typed_input_str: String = typed_input.iter().collect();
                        super::render::render_box(
                            model,
                            provider,
                            session_manager,
                            session_key,
                            &typed_input_str,
                            cursor_idx,
                            &mut viewport_start,
                            selected_index,
                            autocomplete_visible,
                            width_usize,
                            &mut lines_printed,
                        )?;
                    }
                }
                KeyCode::Right => {
                    if cursor_idx < typed_input.len() {
                        cursor_idx += 1;
                        let typed_input_str: String = typed_input.iter().collect();
                        super::render::render_box(
                            model,
                            provider,
                            session_manager,
                            session_key,
                            &typed_input_str,
                            cursor_idx,
                            &mut viewport_start,
                            selected_index,
                            autocomplete_visible,
                            width_usize,
                            &mut lines_printed,
                        )?;
                    }
                }
                KeyCode::Home => {
                    cursor_idx = 0;
                    let typed_input_str: String = typed_input.iter().collect();
                    super::render::render_box(
                        model,
                        provider,
                        session_manager,
                        session_key,
                        &typed_input_str,
                        cursor_idx,
                        &mut viewport_start,
                        selected_index,
                        autocomplete_visible,
                        width_usize,
                        &mut lines_printed,
                    )?;
                }
                KeyCode::End => {
                    cursor_idx = typed_input.len();
                    let typed_input_str: String = typed_input.iter().collect();
                    super::render::render_box(
                        model,
                        provider,
                        session_manager,
                        session_key,
                        &typed_input_str,
                        cursor_idx,
                        &mut viewport_start,
                        selected_index,
                        autocomplete_visible,
                        width_usize,
                        &mut lines_printed,
                    )?;
                }
                KeyCode::Delete => {
                    if cursor_idx < typed_input.len() {
                        typed_input.remove(cursor_idx);
                        selected_index = None;
                        history_index = None;
                        let typed_input_str: String = typed_input.iter().collect();
                        super::render::render_box(
                            model,
                            provider,
                            session_manager,
                            session_key,
                            &typed_input_str,
                            cursor_idx,
                            &mut viewport_start,
                            selected_index,
                            autocomplete_visible,
                            width_usize,
                            &mut lines_printed,
                        )?;
                    }
                }
                KeyCode::Char(c) => {
                    autocomplete_visible = true;
                    selected_index = None;
                    history_index = None;
                    typed_input.insert(cursor_idx, c);
                    cursor_idx += 1;
                    let typed_input_str: String = typed_input.iter().collect();
                    super::render::render_box(
                        model,
                        provider,
                        session_manager,
                        session_key,
                        &typed_input_str,
                        cursor_idx,
                        &mut viewport_start,
                        selected_index,
                        autocomplete_visible,
                        width_usize,
                        &mut lines_printed,
                    )?;
                }
                KeyCode::Backspace => {
                    autocomplete_visible = true;
                    selected_index = None;
                    history_index = None;
                    if cursor_idx > 0 {
                        typed_input.remove(cursor_idx - 1);
                        cursor_idx -= 1;
                        let typed_input_str: String = typed_input.iter().collect();
                        super::render::render_box(
                            model,
                            provider,
                            session_manager,
                            session_key,
                            &typed_input_str,
                            cursor_idx,
                            &mut viewport_start,
                            selected_index,
                            autocomplete_visible,
                            width_usize,
                            &mut lines_printed,
                        )?;
                    }
                }
                KeyCode::Tab => {
                    autocomplete_visible = true;
                    let typed_input_str: String = typed_input.iter().collect();
                    if typed_input_str.starts_with('/') && !typed_input_str.contains(' ') {
                        let matches: Vec<(&str, &str)> = super::render::SLASH_COMMANDS
                            .iter()
                            .filter(|&&(cmd, _)| cmd.starts_with(&typed_input_str))
                            .copied()
                            .collect();
                        if !matches.is_empty() {
                            let completed = if let Some(idx) = selected_index {
                                matches[idx].0.to_string()
                            } else {
                                matches[0].0.to_string()
                            };
                            typed_input = completed.chars().collect();
                            cursor_idx = typed_input.len();
                            selected_index = None;
                            history_index = None;
                            let typed_input_str: String = typed_input.iter().collect();
                            super::render::render_box(
                                model,
                                provider,
                                session_manager,
                                session_key,
                                &typed_input_str,
                                cursor_idx,
                                &mut viewport_start,
                                selected_index,
                                autocomplete_visible,
                                width_usize,
                                &mut lines_printed,
                            )?;
                        }
                    }
                }
                KeyCode::Esc => {
                    autocomplete_visible = false;
                    selected_index = None;
                    let typed_input_str: String = typed_input.iter().collect();
                    super::render::render_box(
                        model,
                        provider,
                        session_manager,
                        session_key,
                        &typed_input_str,
                        cursor_idx,
                        &mut viewport_start,
                        selected_index,
                        autocomplete_visible,
                        width_usize,
                        &mut lines_printed,
                    )?;
                }
                KeyCode::Enter => {
                    let typed_input_str: String = typed_input.iter().collect();
                    let mut final_cmd = typed_input_str.clone();
                    if let Some(idx) = selected_index {
                        let matches: Vec<(&str, &str)> = super::render::SLASH_COMMANDS
                            .iter()
                            .filter(|&&(cmd, _)| cmd.starts_with(&typed_input_str))
                            .copied()
                            .collect();
                        if idx < matches.len() {
                            final_cmd = matches[idx].0.to_string();
                        }
                    }
                    typed_input = final_cmd.chars().collect();
                    if lines_printed > 1 {
                        for _ in 0..(lines_printed - 1) {
                            print!("\r\n\x1b[2K");
                        }
                        print!("\x1b[{}A\r", lines_printed - 1);
                    }
                    print!("\r\x1b[2K");
                    let final_cmd_str: String = typed_input.iter().collect();
                    print!(
                        "{}{}> {}{}",
                        COLOR_BOLD, AURA_SLATE, final_cmd_str, COLOR_RESET
                    );
                    let _ = crossterm::execute!(stdout(), crossterm::event::DisableBracketedPaste);
                    let _ = disable_raw_mode();
                    print!("\r\n");
                    stdout().flush()?;
                    break;
                }
                _ => {}
            }
        }
    }

    let mut final_input = typed_input.iter().collect::<String>();
    for (i, path) in pasted_images.iter().enumerate() {
        let placeholder = if i == 0 {
            "[image]".to_string()
        } else {
            format!("[image{}]", i)
        };
        let replacement = format!("![](file://{})", path.to_string_lossy());
        final_input = final_input.replace(&placeholder, &replacement);
    }

    Ok((final_input, None))
}
