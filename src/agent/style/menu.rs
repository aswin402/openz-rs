use anyhow::Result;
use chrono::{DateTime, Utc, Local};
use crate::agent::style::colors::*;

#[derive(Debug, Clone)]
pub struct HistoryItem {
    pub key: String,
    pub display_title: String,
    pub updated_at: DateTime<Utc>,
}

pub fn format_friendly_time(time: DateTime<Utc>) -> String {
    let now = Local::now();
    let local_time: DateTime<Local> = DateTime::from(time);
    
    if local_time >= now {
        return "0m".to_string();
    }
    
    let duration = now.signed_duration_since(local_time);
    let secs = duration.num_seconds();
    if secs < 60 {
        return "0m".to_string();
    }
    
    let mins = duration.num_minutes();
    if mins < 60 {
        return format!("{}m", mins);
    }
    
    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{}h", hours);
    }
    
    let days = duration.num_days();
    format!("{}d", days)
}

pub fn select_menu_with_history(prompt: &str, history: &[HistoryItem]) -> Result<usize> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use std::io::stdout;
    use std::io::Write;

    enable_raw_mode()?;
    let mut selected = 0;
    let num_options = 1 + history.len();
    
    let max_history_display = 10;
    let has_scrolling = history.len() > max_history_display;
    let num_lines_to_clear = if has_scrolling {
        5 + max_history_display
    } else {
        4 + history.len()
    };

    print!("{}\r\n", prompt);
    
    let draw_menu = |selected_idx: usize| {
        if selected_idx == 0 {
            print!("▸ {}{}Start New{}\r\n", COLOR_BOLD, RED_ORANGE, COLOR_RESET);
        } else {
            print!("  Start New\r\n");
        }
        
        print!("\r\n");
        print!("Recent\r\n");
        print!("\r\n");
        
        let mut start_idx = 0;
        if selected_idx > 0 {
            let history_sel = selected_idx - 1;
            if history_sel >= max_history_display {
                start_idx = history_sel - max_history_display + 1;
            }
        }
        let end_idx = (start_idx + max_history_display).min(history.len());

        for (i, item) in history.iter().enumerate().take(end_idx).skip(start_idx) {
            let option_idx = i + 1;
            let friendly_time = format_friendly_time(item.updated_at);
            
            let truncated_title = if item.display_title.chars().count() > 40 {
                let truncated: String = item.display_title.chars().take(37).collect();
                format!("{}...", truncated)
            } else {
                item.display_title.clone()
            };
            let pad_len = 45_usize.saturating_sub(truncated_title.chars().count());
            let padding = " ".repeat(pad_len);
            
            if selected_idx == option_idx {
                print!("▸ {}{}{}{}{}{}\r\n", COLOR_BOLD, RED_ORANGE, truncated_title, padding, friendly_time, COLOR_RESET);
            } else {
                print!("  {}{}{}\r\n", truncated_title, padding, friendly_time);
            }
        }

        if has_scrolling {
            let rem_above = start_idx;
            let rem_below = history.len() - end_idx;
            let mut parts = Vec::new();
            if rem_above > 0 {
                parts.push(format!("↑ {} more", rem_above));
            }
            if rem_below > 0 {
                parts.push(format!("↓ {} more", rem_below));
            }
            if !parts.is_empty() {
                print!("  {}{}{}\r\n", AURA_SLATE, parts.join(" / "), COLOR_RESET);
            } else {
                print!("\r\n");
            }
        }
        let _ = stdout().flush();
    };

    draw_menu(selected);
    
    loop {
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == KeyEventKind::Release {
                    continue;
                }
                
                let mut changed = false;
                match key_event.code {
                    KeyCode::Up => {
                        if selected > 0 {
                            selected -= 1;
                        } else {
                            selected = num_options - 1;
                        }
                        changed = true;
                    }
                    KeyCode::Down => {
                        if selected < num_options - 1 {
                            selected += 1;
                        } else {
                            selected = 0;
                        }
                        changed = true;
                    }
                    KeyCode::Enter => {
                        for _ in 0..num_lines_to_clear {
                            print!("\r\x1b[1A\x1b[2K");
                        }
                        print!("\r\x1b[1A\x1b[2K");
                        print!("\r");
                        stdout().flush()?;
                        disable_raw_mode()?;
                        return Ok(selected);
                    }
                    KeyCode::Char('c') if key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        for _ in 0..num_lines_to_clear {
                            print!("\r\x1b[1A\x1b[2K");
                        }
                        print!("\r\x1b[1A\x1b[2K");
                        print!("\r");
                        let _ = stdout().flush();
                        disable_raw_mode()?;
                        return Err(anyhow::anyhow!("Ctrl-C"));
                    }
                    _ => {}
                }
                
                if changed {
                    for _ in 0..num_lines_to_clear {
                        print!("\r\x1b[1A\x1b[2K");
                    }
                    draw_menu(selected);
                }
            }
        }
    }
}

pub fn select_menu_custom(
    prompt: &str,
    options: &[String],
    model_name: &str,
    header: Option<&str>,
    show_divider: bool,
) -> Result<Option<usize>> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use crossterm::ExecutableCommand;
    use std::io::{stdout, Write};
    
    let mut stdout = stdout();
    let _ = stdout.execute(crossterm::cursor::Hide);

    enable_raw_mode()?;
    let mut selected = 0;
    let num_options = options.len();
    let max_display = 5;
    let (width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width_usize = width as usize;

    let mut lines_printed = 1;

    let mut prompt_line_idx = 1;
    if show_divider {
        prompt_line_idx += 1;
    }
    if let Some(h) = header {
        prompt_line_idx += h.lines().count();
    }

    let mut draw_menu = |selected_idx: usize, first_draw: bool| -> Result<()> {
        // Clear previous draw if not the first time
        if !first_draw && lines_printed > 1 {
            // Move cursor to the top of the menu
            let move_up_to_top = prompt_line_idx - 1;
            if move_up_to_top > 0 {
                print!("\x1b[{}A\r", move_up_to_top);
            } else {
                print!("\r");
            }

            for _ in 0..(lines_printed - 1) {
                print!("\r\n\x1b[2K");
            }
            print!("\x1b[{}A\r", lines_printed - 1);
        }
        print!("\r\x1b[2K");
        
        let mut count: usize = 0;

        // 1. Print divider line first at the top if enabled
        if show_divider {
            let divider = "────────────────────────────────────────────────────────────";
            print!("{}{}{}", LIGHT_WHITE, divider, COLOR_RESET);
            count += 1;
        }

        // 2. Print header if present
        if let Some(h) = header {
            for (idx, line) in h.lines().enumerate() {
                if count > 0 || idx > 0 {
                    print!("\r\n\x1b[2K{}", line);
                } else {
                    print!("{}", line);
                }
                count += 1;
            }
        }

        // 3. Print prompt
        if count > 0 {
            print!("\r\n\x1b[2K> {}{}{}", COLOR_BOLD, prompt, COLOR_RESET);
        } else {
            print!("> {}{}{}", COLOR_BOLD, prompt, COLOR_RESET);
        }
        count += 1;

        let mut start_idx = 0;
        if selected_idx >= max_display {
            start_idx = selected_idx - max_display + 1;
        }
        let end_idx = (start_idx + max_display).min(num_options);

        // Print visible items
        for (i, opt) in options.iter().enumerate().take(end_idx).skip(start_idx) {
            let is_selected = selected_idx == i;
            if is_selected {
                print!("\r\n\x1b[2K> {}{}{}", RED_ORANGE, opt, COLOR_RESET);
            } else {
                print!("\r\n\x1b[2K  {}", opt);
            }
            count += 1;
        }

        // Print rem indicators
        let rem_below = num_options - end_idx;
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
            count += 1;
        }

        // Print empty spacing line
        print!("\r\n\x1b[2K");
        count += 1;

        // Print help instructions at the bottom
        print!("\r\n\x1b[2K  {}↑/↓ Navigate · enter Select{}", AURA_SLATE, COLOR_RESET);
        count += 1;

        let cancel_text = format!("  {}esc to cancel{}", AURA_SLATE, COLOR_RESET);
        let cancel_width = 15;
        let model_display = format!("{}{}{}", AURA_SLATE, model_name, COLOR_RESET);
        let model_width = model_name.chars().count();
        let spacing = width_usize.saturating_sub(cancel_width + model_width);
        let spaces: String = std::iter::repeat_n(' ', spacing).collect();

        print!("\r\n\x1b[2K{}{}{}", cancel_text, spaces, model_display);
        count += 1;

        // Move cursor back up to the prompt line
        let move_up = count.saturating_sub(prompt_line_idx);
        print!("\x1b[{}A\r", move_up);

        stdout.flush()?;
        lines_printed = count;
        Ok(())
    };

    draw_menu(selected, true)?;

    loop {
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == KeyEventKind::Release {
                    continue;
                }

                match key_event.code {
                    KeyCode::Up => {
                        selected = (selected + num_options - 1) % num_options;
                        draw_menu(selected, false)?;
                    }
                    KeyCode::Down => {
                        selected = (selected + 1) % num_options;
                        draw_menu(selected, false)?;
                    }
                    KeyCode::Enter => {
                        let _ = stdout.execute(crossterm::cursor::Show);
                        // If show_divider is true, we want to KEEP the divider line.
                        // So we clear starting from Line 2 (one line below the divider).
                        // If show_divider is false, we clear starting from Line 1.
                        let clear_start_idx = if show_divider { 2 } else { 1 };
                        
                        let move_up_to_clear_start = prompt_line_idx.saturating_sub(clear_start_idx);
                        if move_up_to_clear_start > 0 {
                            print!("\x1b[{}A\r", move_up_to_clear_start);
                        } else {
                            print!("\r");
                        }

                        // Clear the menu lines
                        if lines_printed > clear_start_idx {
                            for _ in 0..(lines_printed - clear_start_idx) {
                                print!("\r\n\x1b[2K");
                            }
                            print!("\x1b[{}A\r", lines_printed - clear_start_idx);
                        }
                        print!("\r\x1b[2K");
                        
                        let clean_prompt = prompt.trim_end_matches(':');
                        print!("> {}: {}{}{}\r\n", clean_prompt, RED_ORANGE, options[selected], COLOR_RESET);
                        disable_raw_mode()?;
                        let _ = stdout.flush();
                        return Ok(Some(selected));
                    }
                    KeyCode::Esc => {
                        let _ = stdout.execute(crossterm::cursor::Show);
                        // Move cursor to the top of the menu (the divider line)
                        let move_up_to_top = prompt_line_idx - 1;
                        if move_up_to_top > 0 {
                            print!("\x1b[{}A\r", move_up_to_top);
                        } else {
                            print!("\r");
                        }

                        // Clear all lines of the menu
                        if lines_printed > 1 {
                            for _ in 0..(lines_printed - 1) {
                                print!("\r\n\x1b[2K");
                            }
                            print!("\x1b[{}A\r", lines_printed - 1);
                        }
                        print!("\r\x1b[2K");
                        disable_raw_mode()?;
                        let _ = stdout.flush();
                        return Ok(None);
                    }
                    KeyCode::Char('c') if key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        let _ = stdout.execute(crossterm::cursor::Show);
                        // Move cursor to the top of the menu (the divider line)
                        let move_up_to_top = prompt_line_idx - 1;
                        if move_up_to_top > 0 {
                            print!("\x1b[{}A\r", move_up_to_top);
                        } else {
                            print!("\r");
                        }

                        // Clear all lines of the menu
                        if lines_printed > 1 {
                            for _ in 0..(lines_printed - 1) {
                                print!("\r\n\x1b[2K");
                            }
                            print!("\x1b[{}A\r", lines_printed - 1);
                        }
                        print!("\r\x1b[2K");
                        disable_raw_mode()?;
                        let _ = stdout.flush();
                        return Err(anyhow::anyhow!("Ctrl-C"));
                    }
                    _ => {}
                }
            }
        }
    }
}
