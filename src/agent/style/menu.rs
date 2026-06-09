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
        return "Just now".to_string();
    }
    
    let duration = now.signed_duration_since(local_time);
    let secs = duration.num_seconds();
    if secs < 60 {
        return "Just now".to_string();
    }
    
    let mins = duration.num_minutes();
    if mins < 60 {
        return format!("{}m ago", mins);
    }
    
    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{}h ago", hours);
    }
    
    let yesterday = now.date_naive().pred_opt().unwrap_or(now.date_naive());
    if local_time.date_naive() == yesterday {
        return format!("Yesterday {}", local_time.format("%H:%M"));
    }
    
    local_time.format("%Y-%m-%d %H:%M").to_string()
}

pub fn select_menu_with_history(prompt: &str, history: &[HistoryItem]) -> Result<usize> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use std::io::stdout;
    use std::io::Write;

    enable_raw_mode()?;
    let mut selected = 0;
    let num_options = 1 + history.len();
    let num_lines_to_clear = 4 + history.len() * 3;

    print!("{}\r\n", prompt);
    
    let draw_menu = |selected_idx: usize| {
        if selected_idx == 0 {
            print!("▶ {}{}Start New{}\r\n", COLOR_BOLD, AURA_PURPLE, COLOR_RESET);
        } else {
            print!("  Start New\r\n");
        }
        
        print!("\r\n");
        print!("Recent\r\n");
        print!("\r\n");
        
        for (i, item) in history.iter().enumerate() {
            let option_idx = i + 1;
            let friendly_time = format_friendly_time(item.updated_at);
            
            if selected_idx == option_idx {
                print!("▶ {}{}{}\r\n", COLOR_BOLD, item.display_title, COLOR_RESET);
                print!("  \x1b[38;2;107;122;153m{}\x1b[0m\r\n", friendly_time);
            } else {
                print!("  {}\r\n", item.display_title);
                print!("  \x1b[38;2;107;122;153m{}\x1b[0m\r\n", friendly_time);
            }
            print!("\r\n");
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
                        disable_raw_mode()?;
                        println!("Goodbye!");
                        std::process::exit(0);
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
