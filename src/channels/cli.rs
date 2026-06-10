use crate::agent::AgentLoop;
use crate::config::schema::AgentDefaults;
use crate::agent::style::*;
use std::io::{self, Write};
use std::sync::{OnceLock, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

static PENDING_NOTIFICATIONS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static IS_RAW_INPUT_ACTIVE: AtomicBool = AtomicBool::new(false);

fn get_pending_notifications() -> &'static Mutex<Vec<String>> {
    PENDING_NOTIFICATIONS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn send_notification(msg: &str) {
    if IS_RAW_INPUT_ACTIVE.load(Ordering::SeqCst) {
        if let Ok(mut pending) = get_pending_notifications().lock() {
            pending.push(msg.to_string());
        }
    } else {
        println!("{}", msg);
    }
}

struct RawInputGuard;

impl Drop for RawInputGuard {
    fn drop(&mut self) {
        IS_RAW_INPUT_ACTIVE.store(false, Ordering::SeqCst);
    }
}


fn handle_clipboard_paste(index: usize) -> anyhow::Result<std::path::PathBuf> {
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

fn char_display_width(c: char) -> usize {
    let cp = c as u32;
    if (cp >= 0x1F000 && cp <= 0x1FBF9) || c == '⬢' || c == '🗑' || c == '📊' {
        2
    } else {
        1
    }
}


fn print_colored_markdown(content: &str) {
    use crate::agent::style::*;
    let light_blue = "\x1b[38;2;135;206;250m";
    
    for line in content.lines() {
        if line.trim_start().starts_with("#") {
            println!("{}{}{}", HEADING_BLUE, line, COLOR_RESET);
        } else {
            let mut formatted = line.to_string();
            
            if let Ok(re_bold) = regex::Regex::new(r"\*\*(.*?)\*\*") {
                formatted = re_bold.replace_all(&formatted, &format!("{}{}$1{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET)).to_string();
            }
            if let Ok(re_code) = regex::Regex::new(r"`(.*?)`") {
                formatted = re_code.replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET)).to_string();
            }
            if let Ok(re_italic) = regex::Regex::new(r"\*(.*?)\*") {
                formatted = re_italic.replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET)).to_string();
            }
            
            println!("{}", formatted);
        }
    }
}

const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/clear", "Clear screen"),
    ("/exit", "Exit OpenZ"),
    ("/help", "List slash commands"),
    ("/history", "Restore/switch sessions using selection menu"),
    ("/mcps", "List configured MCP servers"),
    ("/memory", "View metadata memory"),
    ("/model", "Show or change active default model"),
    ("/new", "Start a new session"),
    ("/skill", "List active skills"),
];

fn render_box(
    model: &str,
    provider: &str,
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    typed_input: &str,
    cursor_idx: usize,
    viewport_start: &mut usize,
    selected_index: Option<usize>,
    autocomplete_visible: bool,
    width: usize,
    lines_printed: &mut usize,
) -> anyhow::Result<()> {
    use crate::agent::style::*;
    use std::io::{stdout, Write};
    
    // First, clear the previous rendering below the input line
    if *lines_printed > 1 {
        for _ in 0..(*lines_printed - 1) {
            print!("\r\n\x1b[2K");
        }
        // Move cursor back up to the input line
        print!("\x1b[{}A\r", *lines_printed - 1);
    }
    // Clear the input line itself
    print!("\r\x1b[2K");
    
    // 1. Calculate token usage
    let session = session_manager.get_or_create(session_key);
    let total_chars: usize = session.messages.iter().map(|m| m.content.len()).sum();
    let approx_tokens = total_chars / 4;
    
    let model_lower = model.to_lowercase();
    let limit_tokens = if model_lower.contains("claude-3-5") {
        200_000
    } else if model_lower.contains("gpt-4") {
        128_000
    } else if model_lower.contains("gemini") {
        1_048_576
    } else if model_lower.contains("minimax") {
        204_800
    } else if model_lower.contains("deepseek") {
        64_000
    } else {
        128_000
    };
    
    let limit_str = if limit_tokens >= 1_000_000 {
        format!("{}M", limit_tokens / 1_000_000)
    } else {
        format!("{}K", limit_tokens / 1000)
    };
    
    let approx_tokens_str = if approx_tokens >= 1000 {
        format!("{:.1}K", approx_tokens as f64 / 1000.0)
    } else {
        format!("{}", approx_tokens)
    };
    
    let provider_lower = provider.to_lowercase();
    let display_provider = match provider_lower.as_str() {
        "openai" => "OpenAI",
        "anthropic" => "Anthropic",
        "google" => "Google",
        "deepseek" => "DeepSeek",
        other => other,
    };

    let max_model_len = if width < 60 {
        10
    } else if width < 80 {
        16
    } else {
        30
    };

    let display_model = if model.len() > max_model_len {
        format!("{}…", &model[..max_model_len - 1])
    } else {
        model.to_string()
    };

    let visible_status_len = display_provider.chars().count()
        + display_model.chars().count()
        + approx_tokens_str.chars().count()
        + limit_str.chars().count()
        + 11; // spacing, vertical pipes, slash, brackets

    let fill_chars = width.saturating_sub(visible_status_len);
    let line_fill: String = std::iter::repeat('─').take(fill_chars).collect();

    let status_content = format!(
        " {}{}{} | {}{}{} | {}{}{}/{}{}",
        RED_ORANGE, display_provider, LIGHT_WHITE,
        RED_ORANGE, display_model, LIGHT_WHITE,
        RED_ORANGE, approx_tokens_str, LIGHT_WHITE,
        RED_ORANGE, limit_str
    );

    // Filter autocomplete dropdown suggestions
    let mut matches = Vec::new();
    if typed_input.starts_with('/') && !typed_input.contains(' ') {
        for &(cmd, desc) in SLASH_COMMANDS {
            if cmd.starts_with(typed_input) {
                matches.push((cmd, desc));
            }
        }
    }

    let status_line = if autocomplete_visible && !matches.is_empty() {
        let line_fill: String = std::iter::repeat('─').take(width).collect();
        format!("{}{}{}", LIGHT_WHITE, line_fill, COLOR_RESET)
    } else {
        format!(
            "{}{}{}[{}]{}{}",
            LIGHT_WHITE, line_fill,
            RED_ORANGE, status_content, LIGHT_WHITE,
            COLOR_RESET
        )
    };

    let display_text = if let Some(idx) = selected_index {
        if idx < matches.len() {
            matches[idx].0
        } else {
            typed_input
        }
    } else {
        typed_input
    };

    // Print input line prefix and input using cursor-aware viewport
    let display_chars: Vec<char> = display_text.chars().collect();
    let char_count = display_chars.len();
    let max_input_width = width.saturating_sub(3);

    let active_cursor_idx = if selected_index.is_some() {
        char_count
    } else {
        cursor_idx.min(char_count)
    };

    let mut v_start = *viewport_start;
    v_start = v_start.min(char_count);

    if active_cursor_idx < v_start {
        v_start = active_cursor_idx;
    }

    let mut cursor_offset_width = 0;
    for i in v_start..active_cursor_idx {
        cursor_offset_width += char_display_width(display_chars[i]);
    }

    while cursor_offset_width > max_input_width && v_start < active_cursor_idx {
        cursor_offset_width -= char_display_width(display_chars[v_start]);
        v_start += 1;
    }

    let mut total_width_from_v_start = cursor_offset_width;
    for i in active_cursor_idx..char_count {
        total_width_from_v_start += char_display_width(display_chars[i]);
    }
    while v_start > 0 {
        let prev_width = char_display_width(display_chars[v_start - 1]);
        if total_width_from_v_start + prev_width <= max_input_width {
            v_start -= 1;
            total_width_from_v_start += prev_width;
        } else {
            break;
        }
    }

    *viewport_start = v_start;

    let mut display_input = String::new();
    let mut display_width = 0;
    let mut cursor_col_offset = 0;

    for (idx, &c) in display_chars.iter().enumerate().skip(v_start) {
        let w = char_display_width(c);
        if display_width + w <= max_input_width {
            display_input.push(c);
            display_width += w;
            if idx < active_cursor_idx {
                cursor_col_offset += w;
            }
        } else {
            break;
        }
    }

    print!("{}> {}{}", LIGHT_WHITE, COLOR_RESET, display_input);
    let mut new_lines_printed = 1;

    // Status line immediately below the input line
    print!("\r\n\x1b[2K{}", status_line);
    new_lines_printed += 1;

    if autocomplete_visible && !matches.is_empty() {
        let max_display = 5;
        let mut start_idx = 0;
        if let Some(idx) = selected_index {
            if idx >= max_display {
                start_idx = idx - max_display + 1;
            }
        }
        let end_idx = (start_idx + max_display).min(matches.len());
        
        for i in start_idx..end_idx {
            let (cmd, desc) = matches[i];
            let is_selected = selected_index == Some(i);
            
            if is_selected {
                print!("\r\n\x1b[2K> {}{:<30}{}{}{}", RED_ORANGE, cmd, AURA_SLATE, desc, COLOR_RESET);
            } else {
                print!("\r\n\x1b[2K  {:<30}{}{}{}", cmd, AURA_SLATE, desc, COLOR_RESET);
            }
            new_lines_printed += 1;
        }
        
        let rem_below = matches.len() - end_idx;
        let rem_above = start_idx;
        if rem_below > 0 || rem_above > 0 {
            let mut parts = Vec::new();
            if rem_above > 0 {
                parts.push(format!("↑ {} more", rem_above));
            }
            if rem_below > 0 {
                parts.push(format!("↓ {} more", rem_below));
            }
            print!("\r\n\x1b[2K  {}{}{}", AURA_SLATE, parts.join(" / "), COLOR_RESET);
            new_lines_printed += 1;
        }

        // Print spacing empty line
        print!("\r\n\x1b[2K");
        new_lines_printed += 1;

        // Print help/navigation instructions at the bottom
        print!("\r\n\x1b[2K  {}↑/↓ Navigate · enter Select · tab Complete{}", AURA_SLATE, COLOR_RESET);
        
        let cancel_text = format!("  {}esc to cancel{}", AURA_SLATE, COLOR_RESET);
        let cancel_width = 15;
        let model_display = format!("{}{}{}", AURA_SLATE, model, COLOR_RESET);
        let model_width = model.chars().count();
        let spacing = width.saturating_sub(cancel_width + model_width);
        let spaces: String = std::iter::repeat(' ').take(spacing).collect();
        
        print!("\r\n\x1b[2K{}{}{}", cancel_text, spaces, model_display);
        new_lines_printed += 2;
    }

    // Move cursor back up to the input line and place it at the active cursor position
    let cursor_col = 3 + cursor_col_offset;
    print!("\x1b[{}A\x1b[{}G", new_lines_printed - 1, cursor_col);
    
    *lines_printed = new_lines_printed;
    stdout().flush()?;
    Ok(())
}

fn read_line_raw(
    model: &str,
    provider: &str,
    session_manager: &crate::session::SessionManager,
    session_key: &str,
) -> anyhow::Result<String> {
    use crossterm::event::{self, Event, KeyCode, KeyModifiers, KeyEventKind};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use std::io::stdout;

    enable_raw_mode()?;
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
    render_box(
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

    loop {
        // Process any pending notifications first
        let mut notifications = Vec::new();
        if let Ok(mut pending) = get_pending_notifications().lock() {
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
            let _ = std::io::stdout().flush();

            for notif in notifications {
                let formatted_notif = notif.replace("\n", "\r\n");
                print!("{}\r\n", formatted_notif);
            }
            let _ = std::io::stdout().flush();

            lines_printed = 1;
            let typed_input_str: String = typed_input.iter().collect();
            render_box(
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

        if let Some(inbox_msg) = crate::agent::activity::pop_inbox_message("cli:direct") {
            disable_raw_mode()?;
            println!("\r\n{}🔌 [Remote Control] Received prompt: {}{}", AURA_BLUE, inbox_msg.message, COLOR_RESET);
            return Ok(inbox_msg.message);
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == KeyEventKind::Release {
                    continue;
                }

                let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key_event.modifiers.contains(KeyModifiers::ALT);

                // Ctrl+C or Ctrl+D to exit
                if ctrl && (key_event.code == KeyCode::Char('c') || key_event.code == KeyCode::Char('d')) {
                    if lines_printed > 1 {
                        for _ in 0..(lines_printed - 1) {
                            print!("\r\n\x1b[2K");
                        }
                        print!("\x1b[{}A\r", lines_printed - 1);
                    }
                    disable_raw_mode()?;
                    println!("\r\nGoodbye!");
                    std::process::exit(0);
                }

                // Ctrl+V or Alt+V to paste image
                let is_paste_image = (ctrl && key_event.code == KeyCode::Char('v')) || (alt && key_event.code == KeyCode::Char('v'));
                if is_paste_image {
                    if let Some(idx) = selected_index {
                        let typed_input_str: String = typed_input.iter().collect();
                        let matches: Vec<(&str, &str)> = SLASH_COMMANDS.iter()
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
                            render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                        }
                        Err(e) => {
                            if lines_printed > 1 {
                                for _ in 0..(lines_printed - 1) {
                                    print!("\r\n\x1b[2K");
                                }
                                print!("\x1b[{}A\r", lines_printed - 1);
                            }
                            print!("\r\n\x1b[2K{}✕ Error: No image found in clipboard: {}{}\r\n", ERROR_RED, e, COLOR_RESET);
                            lines_printed = 1;
                            if let Ok((w, _)) = crossterm::terminal::size() {
                                width_usize = w as usize;
                            }
                            let typed_input_str: String = typed_input.iter().collect();
                            render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
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
                            let matches: Vec<(&str, &str)> = SLASH_COMMANDS.iter()
                                .filter(|&&(cmd, _)| cmd.starts_with(&typed_input_str))
                                .copied()
                                .collect();
                            if !matches.is_empty() {
                                autocomplete_active = true;
                                selected_index = match selected_index {
                                    None => Some(matches.len() - 1),
                                    Some(idx) => Some((idx + matches.len() - 1) % matches.len()),
                                };
                                render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                            }
                        }

                        if !autocomplete_active {
                            let session = session_manager.get_or_create(session_key);
                            let history_prompts: Vec<String> = session.messages.iter()
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
                                    render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                                }
                            }
                        }
                    }
                    KeyCode::Down => {
                        autocomplete_visible = true;
                        let mut autocomplete_active = false;
                        let typed_input_str: String = typed_input.iter().collect();
                        if typed_input_str.starts_with('/') && !typed_input_str.contains(' ') {
                            let matches: Vec<(&str, &str)> = SLASH_COMMANDS.iter()
                                .filter(|&&(cmd, _)| cmd.starts_with(&typed_input_str))
                                .copied()
                                .collect();
                            if !matches.is_empty() {
                                autocomplete_active = true;
                                selected_index = match selected_index {
                                    None => Some(0),
                                    Some(idx) => Some((idx + 1) % matches.len()),
                                };
                                render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                            }
                        }

                        if !autocomplete_active {
                            let session = session_manager.get_or_create(session_key);
                            let history_prompts: Vec<String> = session.messages.iter()
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
                                    render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                                }
                            }
                        }
                    }
                    KeyCode::Left => {
                        if cursor_idx > 0 {
                            cursor_idx -= 1;
                            let typed_input_str: String = typed_input.iter().collect();
                            render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                        }
                    }
                    KeyCode::Right => {
                        if cursor_idx < typed_input.len() {
                            cursor_idx += 1;
                            let typed_input_str: String = typed_input.iter().collect();
                            render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                        }
                    }
                    KeyCode::Home => {
                        cursor_idx = 0;
                        let typed_input_str: String = typed_input.iter().collect();
                        render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                    }
                    KeyCode::End => {
                        cursor_idx = typed_input.len();
                        let typed_input_str: String = typed_input.iter().collect();
                        render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                    }
                    KeyCode::Delete => {
                        if cursor_idx < typed_input.len() {
                            typed_input.remove(cursor_idx);
                            selected_index = None;
                            history_index = None;
                            let typed_input_str: String = typed_input.iter().collect();
                            render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                        }
                    }
                    KeyCode::Char(c) => {
                        autocomplete_visible = true;
                        selected_index = None;
                        history_index = None;
                        typed_input.insert(cursor_idx, c);
                        cursor_idx += 1;
                        let typed_input_str: String = typed_input.iter().collect();
                        render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                    }
                    KeyCode::Backspace => {
                        autocomplete_visible = true;
                        selected_index = None;
                        history_index = None;
                        if cursor_idx > 0 {
                            typed_input.remove(cursor_idx - 1);
                            cursor_idx -= 1;
                            let typed_input_str: String = typed_input.iter().collect();
                            render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                        }
                    }
                    KeyCode::Tab => {
                        autocomplete_visible = true;
                        let typed_input_str: String = typed_input.iter().collect();
                        if typed_input_str.starts_with('/') && !typed_input_str.contains(' ') {
                            let matches: Vec<(&str, &str)> = SLASH_COMMANDS.iter()
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
                                render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                            }
                        }
                    }
                    KeyCode::Esc => {
                        autocomplete_visible = false;
                        selected_index = None;
                        let typed_input_str: String = typed_input.iter().collect();
                        render_box(model, provider, session_manager, session_key, &typed_input_str, cursor_idx, &mut viewport_start, selected_index, autocomplete_visible, width_usize, &mut lines_printed)?;
                    }
                    KeyCode::Enter => {
                        let typed_input_str: String = typed_input.iter().collect();
                        let mut final_cmd = typed_input_str.clone();
                        if let Some(idx) = selected_index {
                            let matches: Vec<(&str, &str)> = SLASH_COMMANDS.iter()
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
                        print!("{}{}> {}{}", COLOR_BOLD, AURA_SLATE, final_cmd_str, COLOR_RESET);
                        disable_raw_mode()?;
                        print!("\r\n");
                        stdout().flush()?;
                        break;
                    }
                    _ => {}
                }
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

    Ok(final_input)
}

pub struct CliChannel {
    agent_loop: tokio::sync::Mutex<AgentLoop>,
    defaults: tokio::sync::Mutex<AgentDefaults>,
}

impl CliChannel {
    pub fn new(agent_loop: AgentLoop, defaults: AgentDefaults) -> Self {
        CliChannel {
            agent_loop: tokio::sync::Mutex::new(agent_loop),
            defaults: tokio::sync::Mutex::new(defaults),
        }
    }
}

#[async_trait::async_trait]
impl super::Channel for CliChannel {
    fn name(&self) -> &'static str {
        "cli"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let session_key = "cli:direct";
        
        let white = "\x1b[38;2;240;240;240m";
        let slate = "\x1b[38;2;107;122;153m";
        
        println!("{}     ██████╗ ██████╗ ███████╗███╗   ██╗{}███████╗", white, RED_ORANGE);
        println!("{}    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║{}╚══███╔╝", white, RED_ORANGE);
        println!("{}    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║{}  ███╔╝", white, RED_ORANGE);
        println!("{}    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║{} ███╔╝", white, RED_ORANGE);
        println!("{}    ╚██████╔╝██║     ███████╗██║ ╚████║{}███████╗", white, RED_ORANGE);
        println!("{}     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝{}╚══════╝\r", white, RED_ORANGE);
        
        println!("{}openz v{}{}", COLOR_BOLD, env!("CARGO_PKG_VERSION"), COLOR_RESET);
        {
            let defaults = self.defaults.lock().await;
            println!("{}{}{}", slate, format!("{} | {}", defaults.provider, defaults.model), COLOR_RESET);
        }
        
        if let Ok(current_dir) = std::env::current_dir() {
            let path_str = if let Some(home) = dirs::home_dir() {
                if current_dir == home {
                    "~".to_string()
                } else if let Ok(stripped) = current_dir.strip_prefix(&home) {
                    format!("~/{}", stripped.display())
                } else {
                    current_dir.display().to_string()
                }
            } else {
                current_dir.display().to_string()
            };
            println!("{}{}{}", slate, path_str, COLOR_RESET);
        }
        
        println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
        
        loop {
            let (model, provider, session_manager) = {
                let defaults = self.defaults.lock().await;
                let agent_loop = self.agent_loop.lock().await;
                (defaults.model.clone(), defaults.provider.clone(), agent_loop.session_manager.clone())
            };
            
            let input = match read_line_raw(
                &model,
                &provider,
                &session_manager,
                session_key,
            ) {
                Ok(inp) => inp,
                Err(e) => {
                    eprintln!("Error reading input: {}", e);
                    continue;
                }
            };
            let trimmed = input.trim();
            
            if trimmed.is_empty() {
                continue;
            }
 
            if trimmed == "/exit" || trimmed == "exit" || trimmed == "quit" {
                println!("Goodbye!");
                break;
            }
 
            if trimmed == "/clear" {
                use crossterm::ExecutableCommand;
                let mut stdout = io::stdout();
                let _ = stdout.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                let _ = stdout.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::Purge));
                let _ = stdout.execute(crossterm::cursor::MoveTo(0, 0));
                let _ = stdout.flush();
                continue;
            }

            if trimmed == "/help" {
                println!("{}Available commands:{}", COLOR_BOLD, COLOR_RESET);
                for &(cmd, desc) in SLASH_COMMANDS {
                    println!("  {}{:<12}{} - {}", RED_ORANGE, cmd, COLOR_RESET, desc);
                }
                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                continue;
            }

            if trimmed.starts_with("/model") {
                let arg = trimmed["/model".len()..].trim();
                if arg.is_empty() {
                    use crate::config::loader::load_config;
                    let config = match load_config() {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("{}✕ Error: Failed to load config: {}{}", ERROR_RED, e, COLOR_RESET);
                            continue;
                        }
                    };

                    struct ProviderModels {
                        name: &'static str,
                        display: &'static str,
                        models: &'static [&'static str],
                    }

                    let provider_list = &[
                        ProviderModels {
                            name: "openai",
                            display: "OpenAI (5)",
                            models: &["gpt-4o", "gpt-4o-mini", "o1", "o1-mini", "o3-mini"],
                        },
                        ProviderModels {
                            name: "anthropic",
                            display: "Anthropic (3)",
                            models: &["claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022", "claude-3-opus-20240229"],
                        },
                        ProviderModels {
                            name: "openrouter",
                            display: "OpenRouter (5)",
                            models: &[
                                "google/gemini-2.5-pro",
                                "google/gemini-2.5-flash",
                                "anthropic/claude-3.5-sonnet",
                                "meta-llama/llama-3.3-70b-instruct",
                                "deepseek/deepseek-r1",
                            ],
                        },
                        ProviderModels {
                            name: "deepseek",
                            display: "DeepSeek (2)",
                            models: &["deepseek-chat", "deepseek-reasoner"],
                        },
                        ProviderModels {
                            name: "groq",
                            display: "Groq (5)",
                            models: &[
                                "deepseek-r1-distill-llama-70b",
                                "llama-3.3-70b-versatile",
                                "llama-3.1-8b-instant",
                                "mixtral-8x7b-32768",
                                "gemma2-9b-it",
                            ],
                        },
                        ProviderModels {
                            name: "ollama",
                            display: "Ollama (5)",
                            models: &["llama3", "mistral", "phi3", "qwen2.5", "deepseek-r1"],
                        },
                        ProviderModels {
                            name: "minimax",
                            display: "minimax.io (6)",
                            models: &[
                                "MiniMax-M3",
                                "MiniMax-M2.7",
                                "MiniMax-M2.5",
                                "MiniMax-M2.1",
                                "MiniMax-M2",
                                "MiniMax-M1",
                            ],
                        },
                        ProviderModels {
                            name: "mistral",
                            display: "Mistral AI (5)",
                            models: &[
                                "mistral-large-latest",
                                "pixtral-large-latest",
                                "mistral-moderation-latest",
                                "codestral-latest",
                                "mistral-small-latest",
                            ],
                        },
                        ProviderModels {
                            name: "z.ai",
                            display: "z.ai (Zhipu GLM) (5)",
                            models: &[
                                "glm-5.1",
                                "glm-5",
                                "glm-5v-turbo",
                                "glm-4.7",
                                "glm-4.7-flash",
                            ],
                        },
                        ProviderModels {
                            name: "nvidia",
                            display: "NVIDIA NIM (5)",
                            models: &[
                                "meta/llama3-70b-instruct",
                                "nvidia/llama-3.1-nemotron-70b-instruct",
                                "meta/llama-3.1-70b-instruct",
                                "mistralai/mixtral-8x22b-instruct-v0.1",
                                "google/gemma-2-27b-it",
                            ],
                        },
                        ProviderModels {
                            name: "opencode_zen",
                            display: "OpenCode Zen (4)",
                            models: &[
                                "gpt-5.5-pro",
                                "gpt-5.5",
                                "gpt-5.4-pro",
                                "gpt-5.4",
                            ],
                        },
                        ProviderModels {
                            name: "cerebres",
                            display: "Cerebras (3)",
                            models: &[
                                "llama-3.3-70b",
                                "llama3.1-8b",
                                "llama3.1-70b",
                            ],
                        },
                        ProviderModels {
                            name: "google_ai_studio",
                            display: "Google AI Studio (Gemini) (4)",
                            models: &[
                                "gemini-2.5-pro",
                                "gemini-2.5-flash",
                                "gemini-2.0-flash",
                                "gemini-1.5-pro",
                            ],
                        },
                    ];

                    let filtered_providers: Vec<&ProviderModels> = provider_list
                        .iter()
                        .filter(|p| config.is_provider_configured(p.name))
                        .collect();

                    if filtered_providers.is_empty() {
                        println!("{}⚠️ No LLM providers configured! Please run 'openz configure' first.{}", crate::agent::style::colors::AURA_GOLD, crate::agent::style::colors::COLOR_RESET);
                        println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                        continue;
                    }

                    let mut provider_options: Vec<String> = filtered_providers.iter().map(|p| p.display.to_string()).collect();
                    provider_options.push("Exit".to_string());
                    let (active_mdl, current_active_header) = {
                        let defaults = self.defaults.lock().await;
                        (
                            defaults.model.clone(),
                            format!("Current active model: {} | Provider: {}", defaults.model, defaults.provider)
                        )
                    };
                    match crate::agent::style::select_menu_custom("Choose an LLM provider:", &provider_options, &active_mdl, Some(&current_active_header), false) {
                        Ok(Some(selected_idx)) => {
                            if selected_idx == filtered_providers.len() {
                                println!("Model selection cancelled.");
                                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                                continue;
                            }
                            let prov_info = filtered_providers[selected_idx];
                            let mut model_options: Vec<String> = prov_info.models.iter().map(|&m| m.to_string()).collect();
                            model_options.push("Exit".to_string());
                            match crate::agent::style::select_menu_custom(&format!("Choose a model from {}:", prov_info.display), &model_options, &active_mdl, None, false) {
                                Ok(Some(selected_model_idx)) => {
                                    if selected_model_idx == prov_info.models.len() {
                                        println!("Model selection cancelled.");
                                        println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                                        continue;
                                    }
                                    let prov = prov_info.name;
                                    let mdl = prov_info.models[selected_model_idx];
                                    
                                    use crate::config::loader::{load_config, save_config};
                                    match load_config() {
                                        Ok(mut config) => {
                                            config.agents.defaults.provider = prov.to_string();
                                            config.agents.defaults.model = mdl.to_string();
                                            if let Err(e) = save_config(&config) {
                                                eprintln!("{}✕ Error: Failed to save config: {}{}", ERROR_RED, e, COLOR_RESET);
                                            } else {
                                                match crate::cli::build_agent_loop(config.clone()).await {
                                                    Ok(new_loop) => {
                                                        *self.agent_loop.lock().await = new_loop;
                                                        *self.defaults.lock().await = config.agents.defaults;
                                                        println!("{}✓ Model updated to {} (provider: {}){}", EMERALD_GREEN, mdl, prov, COLOR_RESET);
                                                    }
                                                    Err(e) => {
                                                        eprintln!("{}✕ Error: Failed to initialize new model: {}{}", ERROR_RED, e, COLOR_RESET);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("{}✕ Error: Failed to load config: {}{}", ERROR_RED, e, COLOR_RESET);
                                        }
                                    }
                                }
                                Ok(None) => {
                                    println!("Model selection cancelled.");
                                }
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                }
                            }
                        }
                        Ok(None) => {
                            println!("Provider selection cancelled.");
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                        }
                    }
                } else {
                    let (prov, mdl) = if let Some(idx) = arg.find('/') {
                        (&arg[..idx], &arg[idx + 1..])
                    } else {
                        ("auto", arg)
                    };
                    
                    use crate::config::loader::{load_config, save_config};
                    match load_config() {
                        Ok(mut config) => {
                            config.agents.defaults.provider = prov.to_string();
                            config.agents.defaults.model = mdl.to_string();
                            if let Err(e) = save_config(&config) {
                                eprintln!("{}✕ Error: Failed to save config: {}{}", ERROR_RED, e, COLOR_RESET);
                            } else {
                                match crate::cli::build_agent_loop(config.clone()).await {
                                    Ok(new_loop) => {
                                        *self.agent_loop.lock().await = new_loop;
                                        *self.defaults.lock().await = config.agents.defaults;
                                        println!("{}✓ Model updated to {} (provider: {}){}", EMERALD_GREEN, mdl, prov, COLOR_RESET);
                                    }
                                    Err(e) => {
                                        eprintln!("{}✕ Error: Failed to initialize new model: {}{}", ERROR_RED, e, COLOR_RESET);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("{}✕ Error: Failed to load config: {}{}", ERROR_RED, e, COLOR_RESET);
                        }
                    }
                }
                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                continue;
            }

            if trimmed == "/new" {
                let session_manager = {
                    let agent_loop = self.agent_loop.lock().await;
                    agent_loop.session_manager.clone()
                };
                if let Ok(mut current_session) = session_manager.load(session_key) {
                    if !current_session.messages.is_empty() {
                        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                        let archive_key = format!("cli:history_{}", timestamp);
                        current_session.key = archive_key;
                        let _ = session_manager.save(&current_session);
                        
                        let empty_session = crate::session::Session::new(session_key);
                        let _ = session_manager.save(&empty_session);
                    }
                }
                println!("{}✓ Session reset. Starting a new session.{}", EMERALD_GREEN, COLOR_RESET);
                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                continue;
            }

            if trimmed == "/skill" {
                match crate::agent::skills::load_skills() {
                    Ok(skills) => {
                        if skills.is_empty() {
                            println!("No active skills found in ~/.openz/skills");
                        } else {
                            println!("{}Active skills:{}", COLOR_BOLD, COLOR_RESET);
                            for skill in skills {
                                println!("  • {}", skill.name);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}✕ Error loading skills: {}{}", ERROR_RED, e, COLOR_RESET);
                    }
                }
                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                continue;
            }

            if trimmed == "/mcps" {
                let agent_loop = self.agent_loop.lock().await;
                println!("{}Configured MCP Servers:{}", COLOR_BOLD, COLOR_RESET);
                if agent_loop.config.mcp_servers.is_empty() {
                    println!("  No MCP servers configured.");
                } else {
                    for (name, mcp_cfg) in &agent_loop.config.mcp_servers {
                        let status = if mcp_cfg.enabled {
                            format!("{}enabled{}", EMERALD_GREEN, COLOR_RESET)
                        } else {
                            format!("{}disabled{}", AURA_SLATE, COLOR_RESET)
                        };
                        println!("  • {} ({}) - {}", name, status, mcp_cfg.command);
                    }
                }
                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                continue;
            }

            if trimmed == "/history" {
                let session_manager = {
                    let agent_loop = self.agent_loop.lock().await;
                    agent_loop.session_manager.clone()
                };
                match crate::cli::load_session_history() {
                    Ok(history) => {
                        if history.is_empty() {
                            println!("No session history found.");
                        } else {
                            match crate::agent::style::select_menu_with_history("Select a session to load:", &history) {
                                Ok(selected) => {
                                    if selected == 0 {
                                        let _ = crate::cli::archive_current_session(&session_manager);
                                        println!("{}✓ Started new session.{}", EMERALD_GREEN, COLOR_RESET);
                                    } else {
                                        let selected_item = &history[selected - 1];
                                        let _ = crate::cli::archive_current_session(&session_manager);
                                        if let Ok(mut session) = session_manager.load(&selected_item.key) {
                                            session.key = session_key.to_string();
                                            let _ = session_manager.save(&session);
                                            println!("{}✓ Loaded session: {}{}", EMERALD_GREEN, selected_item.display_title, COLOR_RESET);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("{}✕ Error running selection menu: {}{}", ERROR_RED, e, COLOR_RESET);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}✕ Error loading session history: {}{}", ERROR_RED, e, COLOR_RESET);
                    }
                }
                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                continue;
            }

            if trimmed == "/memory" {
                let session_manager = {
                    let agent_loop = self.agent_loop.lock().await;
                    agent_loop.session_manager.clone()
                };
                if let Ok(session) = session_manager.load(session_key) {
                    println!("{}Session Metadata & Memory:{}", COLOR_BOLD, COLOR_RESET);
                    if session.metadata.is_empty() {
                        println!("  No memory or metadata recorded for this session.");
                    } else {
                        for (k, v) in &session.metadata {
                            println!("  • {}: {}", k, v);
                        }
                    }
                } else {
                    println!("No active session found.");
                }
                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                continue;
            }
 
            if trimmed == "/paste" || trimmed == "/clip" {
                match handle_clipboard_paste(0) {
                    Ok(img_path) => {
                        println!("{}✓ Image captured from clipboard and saved to: {}{}", EMERALD_GREEN, img_path.display(), COLOR_RESET);
                        print!("Enter query/instructions for this image: ");
                        let _ = io::stdout().flush();
                        let mut query = String::new();
                        let _ = io::stdin().read_line(&mut query);
                        let combined_query = format!("{} ![](file://{})", query.trim(), img_path.to_string_lossy());
                        
                        let agent_loop = self.agent_loop.lock().await;
                        match agent_loop.run(&combined_query, session_key).await {
                            Ok(res) => {
                                println!();
                                print_colored_markdown(&res.content);
                                println!();
                                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                            }
                            Err(e) => {
                                eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET);
                                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}✕ Error: Failed to retrieve image from clipboard: {}{}", ERROR_RED, e, COLOR_RESET);
                    }
                }
                continue;
            }
 
            let runner = self.agent_loop.lock().await;
            
            use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
            let _ = enable_raw_mode();
            
            let run_fut = runner.run(trimmed, session_key);
            let esc_fut = async {
                use crossterm::event::{self, Event, KeyCode, KeyEventKind};
                loop {
                    let has_event = tokio::task::spawn_blocking(|| {
                        event::poll(std::time::Duration::from_millis(50))
                    }).await.unwrap_or(Ok(false)).unwrap_or(false);

                    if has_event {
                        let is_esc = tokio::task::spawn_blocking(|| {
                            if let Ok(Event::Key(key_event)) = event::read() {
                                key_event.kind != KeyEventKind::Release && key_event.code == KeyCode::Esc
                            } else {
                                false
                            }
                        }).await.unwrap_or(false);

                        if is_esc {
                            return;
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            };
            
            tokio::pin!(run_fut);
            tokio::pin!(esc_fut);
            
            let run_res = tokio::select! {
                res = &mut run_fut => Some(res),
                _ = &mut esc_fut => None,
            };
            
            let _ = disable_raw_mode();
            
            match run_res {
                Some(Ok(res)) => {
                    println!();
                    print_colored_markdown(&res.content);
                    println!();
                    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                }
                Some(Err(e)) => {
                    eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET);
                    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                }
                None => {
                    println!("\r\n{}✕ Conversation interrupted by user.{}", ERROR_RED, COLOR_RESET);
                    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                }
            }
        }
        
        Ok(())
    }
}
