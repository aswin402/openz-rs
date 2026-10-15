use crate::agent::style::colors::*;
use crate::agent::style::markdown::{horizontal_divider, terminal_width_or_default};
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::ExecutableCommand;
use std::io::{stdout, Write};

#[derive(Debug, Clone)]
pub struct HistoryItem {
    pub key: String,
    pub display_title: String,
    pub updated_at: DateTime<Utc>,
}

pub fn format_friendly_time(time: DateTime<Utc>) -> String {
    let now = Local::now();
    let local_time: DateTime<Local> = DateTime::from(time);

    match local_time >= now {
        true => return "0m".to_string(),
        false => {}
    }

    let duration = now.signed_duration_since(local_time);
    let secs = duration.num_seconds();
    match secs < 60 {
        true => return "0m".to_string(),
        false => {}
    }

    let mins = duration.num_minutes();
    match mins < 60 {
        true => return format!("{}m", mins),
        false => {}
    }

    let hours = duration.num_hours();
    match hours < 24 {
        true => return format!("{}h", hours),
        false => {}
    }

    format!("{}d", duration.num_days())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    MovePrev,
    MoveNext,
    QuickSelect(usize),
    Confirm,
    Cancel,
    Interrupt,
    Ignore,
}

fn classify_vertical_menu_key(key_event: &KeyEvent) -> MenuAction {
    match key_event.kind == KeyEventKind::Release {
        true => return MenuAction::Ignore,
        false => {}
    }
    let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
    match (key_event.code, ctrl) {
        (KeyCode::Char('c'), true) => MenuAction::Interrupt,
        (KeyCode::Up, _) | (KeyCode::Char('k'), false) => MenuAction::MovePrev,
        (KeyCode::Down, _) | (KeyCode::Char('j'), false) => MenuAction::MoveNext,
        (KeyCode::Enter, _) => MenuAction::Confirm,
        (KeyCode::Esc, _) | (KeyCode::Char('q'), false) => MenuAction::Cancel,
        (KeyCode::Char(d @ '1'..='9'), false) => MenuAction::QuickSelect((d as usize) - ('1' as usize)),
        _ => MenuAction::Ignore,
    }
}

fn classify_horizontal_menu_key(key_event: &KeyEvent) -> MenuAction {
    match key_event.kind == KeyEventKind::Release {
        true => return MenuAction::Ignore,
        false => {}
    }
    let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
    match (key_event.code, ctrl) {
        (KeyCode::Char('c'), true) => MenuAction::Interrupt,
        (KeyCode::Left, _) | (KeyCode::Char('h'), false) => MenuAction::MovePrev,
        (KeyCode::Right, _) | (KeyCode::Tab, _) | (KeyCode::Char('l'), false) => {
            MenuAction::MoveNext
        }
        (KeyCode::Enter, _) => MenuAction::Confirm,
        (KeyCode::Esc, _) | (KeyCode::Char('q'), false) => MenuAction::Cancel,
        (KeyCode::Char(d @ '1'..='9'), false) => MenuAction::QuickSelect((d as usize) - ('1' as usize)),
        _ => MenuAction::Ignore,
    }
}

fn compute_menu_viewport(selected_idx: usize, total: usize, max_display: usize) -> (usize, usize) {
    let start_idx = match selected_idx >= max_display {
        true => selected_idx + 1 - max_display,
        false => 0,
    };
    let end_idx = (start_idx + max_display).min(total);
    (start_idx, end_idx)
}

fn format_scroll_indicator(rem_above: usize, rem_below: usize) -> Option<String> {
    let mut parts = Vec::new();
    match rem_above > 0 {
        true => parts.push(format!("↑ {} more", rem_above)),
        false => {}
    }
    match rem_below > 0 {
        true => parts.push(format!("↓ {} more", rem_below)),
        false => {}
    }
    match parts.is_empty() {
        true => None,
        false => Some(parts.join(" / ")),
    }
}

fn clear_lines_above(num_lines: usize) {
    for _ in 0..num_lines {
        print!("\r\x1b[1A\x1b[2K");
    }
}

fn move_up_and_clear_block(move_up: usize, lines_below: usize) {
    match move_up > 0 {
        true => print!("\x1b[{}A\r", move_up),
        false => print!("\r"),
    }
    match lines_below > 0 {
        true => {
            for _ in 0..lines_below {
                print!("\r\n\x1b[2K");
            }
            print!("\x1b[{}A\r", lines_below);
        }
        false => {}
    }
    print!("\r\x1b[2K");
}

pub fn select_menu_with_history(prompt: &str, history: &[HistoryItem]) -> Result<usize> {
    select_menu_with_history_first_option(prompt, history, "Start New")
}

pub fn select_menu_with_history_first_option(
    prompt: &str,
    history: &[HistoryItem],
    first_option: &str,
) -> Result<usize> {
    let mut stdout = stdout();
    let _ = stdout.execute(crossterm::cursor::Hide);
    enable_raw_mode()?;
    drain_pending_key_events();
    let mut selected = 0usize;
    let num_options = 1 + history.len();

    let max_history_display = 10usize;
    let has_scrolling = history.len() > max_history_display;
    let num_lines_to_clear = match has_scrolling {
        true => 5 + max_history_display,
        false => 4 + history.len(),
    };

    print!("{}\r\n", prompt);

    let mut draw_menu = |selected_idx: usize| {
        match selected_idx == 0 {
            true => print!(
                "▸ {}{}{}{}\r\n",
                COLOR_BOLD, RED_ORANGE, first_option, COLOR_RESET
            ),
            false => print!("  {}\r\n", first_option),
        }

        print!("\r\nRecent\r\n\r\n");

        let history_sel = selected_idx.saturating_sub(1);
        let (start_idx, end_idx) = match selected_idx > 0 {
            true => compute_menu_viewport(history_sel, history.len(), max_history_display),
            false => (0, max_history_display.min(history.len())),
        };

        for (i, item) in history.iter().enumerate().take(end_idx).skip(start_idx) {
            let option_idx = i + 1;
            let friendly_time = format_friendly_time(item.updated_at);
            let char_count = item.display_title.chars().count();
            let truncated_title = match char_count > 40 {
                true => {
                    let truncated: String = item.display_title.chars().take(37).collect();
                    format!("{}...", truncated)
                }
                false => item.display_title.clone(),
            };
            let pad_len = 45_usize.saturating_sub(truncated_title.chars().count());
            let padding = " ".repeat(pad_len);

            match selected_idx == option_idx {
                true => print!(
                    "▸ {}{}{}{}{}{}\r\n",
                    COLOR_BOLD, RED_ORANGE, truncated_title, padding, friendly_time, COLOR_RESET
                ),
                false => print!("  {}{}{}\r\n", truncated_title, padding, friendly_time),
            }
        }

        match has_scrolling {
            true => match format_scroll_indicator(start_idx, history.len() - end_idx) {
                Some(ind) => print!("  {}{}{}\r\n", AURA_SLATE, ind, COLOR_RESET),
                None => print!("\r\n"),
            },
            false => {}
        }
        let _ = stdout.flush();
    };

    draw_menu(selected);

    loop {
        match event::poll(std::time::Duration::from_millis(50))? {
            true => match event::read()? {
                Event::Key(key_event) => match classify_vertical_menu_key(&key_event) {
                    MenuAction::MovePrev => {
                        selected = (selected + num_options - 1) % num_options;
                        clear_lines_above(num_lines_to_clear);
                        draw_menu(selected);
                    }
                    MenuAction::MoveNext => {
                        selected = (selected + 1) % num_options;
                        clear_lines_above(num_lines_to_clear);
                        draw_menu(selected);
                    }
                    MenuAction::QuickSelect(idx) => match idx < num_options {
                        true => {
                            clear_lines_above(num_lines_to_clear + 1);
                            print!("\r");
                            let _ = stdout.execute(crossterm::cursor::Show);
                            stdout.flush()?;
                            disable_raw_mode()?;
                            return Ok(idx);
                        }
                        false => {}
                    },
                    MenuAction::Confirm => {
                        clear_lines_above(num_lines_to_clear + 1);
                        print!("\r");
                        let _ = stdout.execute(crossterm::cursor::Show);
                        stdout.flush()?;
                        disable_raw_mode()?;
                        return Ok(selected);
                    }
                    MenuAction::Cancel => {
                        clear_lines_above(num_lines_to_clear + 1);
                        print!("\r");
                        let _ = stdout.execute(crossterm::cursor::Show);
                        let _ = stdout.flush();
                        disable_raw_mode()?;
                        return Err(anyhow::anyhow!("Cancelled"));
                    }
                    MenuAction::Interrupt => {
                        clear_lines_above(num_lines_to_clear + 1);
                        print!("\r");
                        let _ = stdout.execute(crossterm::cursor::Show);
                        let _ = stdout.flush();
                        disable_raw_mode()?;
                        return Err(anyhow::anyhow!("Ctrl-C"));
                    }
                    MenuAction::Ignore => {}
                },
                _ => {}
            },
            false => {}
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
    let mut stdout = stdout();
    let _ = stdout.execute(crossterm::cursor::Hide);

    enable_raw_mode()?;
    drain_pending_key_events();
    let mut selected = 0usize;
    let num_options = options.len();
    let max_display = 5usize;
    let width_usize = terminal_width_or_default(80);

    let mut lines_printed = 1usize;
    let divider_offset = usize::from(show_divider);
    let header_lines = header.map(|h| h.lines().count()).unwrap_or(0);
    let prompt_line_idx = 1 + divider_offset + header_lines;

    let mut draw_menu = |selected_idx: usize, first_draw: bool| -> Result<()> {
        match !first_draw && lines_printed > 1 {
            true => move_up_and_clear_block(prompt_line_idx.saturating_sub(1), lines_printed - 1),
            false => print!("\r\x1b[2K"),
        }

        let mut count: usize = 0;

        match show_divider {
            true => {
                let divider = horizontal_divider(60);
                print!("{}{}{}", LIGHT_WHITE, divider, COLOR_RESET);
                count += 1;
            }
            false => {}
        }

        match header {
            Some(h) => {
                for (idx, line) in h.lines().enumerate() {
                    match count > 0 || idx > 0 {
                        true => print!("\r\n\x1b[2K{}", line),
                        false => print!("{}", line),
                    }
                    count += 1;
                }
            }
            None => {}
        }

        match count > 0 {
            true => print!("\r\n\x1b[2K> {}{}{}", COLOR_BOLD, prompt, COLOR_RESET),
            false => print!("> {}{}{}", COLOR_BOLD, prompt, COLOR_RESET),
        }
        count += 1;

        let (start_idx, end_idx) = compute_menu_viewport(selected_idx, num_options, max_display);

        for (i, opt) in options.iter().enumerate().take(end_idx).skip(start_idx) {
            match selected_idx == i {
                true => print!("\r\n\x1b[2K> {}{}{}", RED_ORANGE, opt, COLOR_RESET),
                false => print!("\r\n\x1b[2K  {}", opt),
            }
            count += 1;
        }

        match format_scroll_indicator(start_idx, num_options.saturating_sub(end_idx)) {
            Some(ind) => {
                print!("\r\n\x1b[2K  {}{}{}", AURA_SLATE, ind, COLOR_RESET);
                count += 1;
            }
            None => {}
        }

        print!("\r\n\x1b[2K");
        count += 1;

        print!(
            "\r\n\x1b[2K  {}↑/↓ Navigate · enter Select{}",
            AURA_SLATE, COLOR_RESET
        );
        count += 1;

        let cancel_text = format!("  {}esc to cancel{}", AURA_SLATE, COLOR_RESET);
        let cancel_width = 15;
        let model_display = format!("{}{}{}", AURA_SLATE, model_name, COLOR_RESET);
        let model_width = model_name.chars().count();
        let spacing = width_usize.saturating_sub(cancel_width + model_width);
        let spaces: String = std::iter::repeat_n(' ', spacing).collect();

        print!("\r\n\x1b[2K{}{}{}", cancel_text, spaces, model_display);
        count += 1;

        let move_up = count.saturating_sub(prompt_line_idx);
        print!("\x1b[{}A\r", move_up);

        stdout.flush()?;
        lines_printed = count;
        Ok(())
    };

    draw_menu(selected, true)?;

    let finish_confirm = |chosen: usize, lines_printed: usize, stdout: &mut std::io::Stdout| -> Result<Option<usize>> {
        let _ = stdout.execute(crossterm::cursor::Show);
        let clear_start_idx = match show_divider {
            true => 2,
            false => 1,
        };
        move_up_and_clear_block(
            prompt_line_idx.saturating_sub(clear_start_idx),
            lines_printed.saturating_sub(clear_start_idx),
        );
        let clean_prompt = prompt.trim_end_matches(':');
        print!(
            "> {}: {}{}{}\r\n",
            clean_prompt, RED_ORANGE, options[chosen], COLOR_RESET
        );
        disable_raw_mode()?;
        let _ = stdout.flush();
        Ok(Some(chosen))
    };

    loop {
        match event::poll(std::time::Duration::from_millis(50))? {
            true => match event::read()? {
                Event::Key(key_event) => match classify_vertical_menu_key(&key_event) {
                    MenuAction::MovePrev => {
                        selected = (selected + num_options - 1) % num_options;
                        draw_menu(selected, false)?;
                    }
                    MenuAction::MoveNext => {
                        selected = (selected + 1) % num_options;
                        draw_menu(selected, false)?;
                    }
                    MenuAction::QuickSelect(idx) => match idx < num_options {
                        true => return finish_confirm(idx, lines_printed, &mut stdout),
                        false => {}
                    },
                    MenuAction::Confirm => {
                        return finish_confirm(selected, lines_printed, &mut stdout);
                    }
                    MenuAction::Cancel => {
                        let _ = stdout.execute(crossterm::cursor::Show);
                        move_up_and_clear_block(
                            prompt_line_idx.saturating_sub(1),
                            lines_printed.saturating_sub(1),
                        );
                        disable_raw_mode()?;
                        let _ = stdout.flush();
                        return Ok(None);
                    }
                    MenuAction::Interrupt => {
                        let _ = stdout.execute(crossterm::cursor::Show);
                        move_up_and_clear_block(
                            prompt_line_idx.saturating_sub(1),
                            lines_printed.saturating_sub(1),
                        );
                        disable_raw_mode()?;
                        let _ = stdout.flush();
                        return Err(anyhow::anyhow!("Ctrl-C"));
                    }
                    MenuAction::Ignore => {}
                },
                _ => {}
            },
            false => {}
        }
    }
}

fn drain_pending_key_events() {
    for _ in 0..64 {
        match event::poll(std::time::Duration::from_millis(0)) {
            Ok(true) => match event::read() {
                Ok(_) => continue,
                Err(_) => break,
            },
            _ => break,
        }
    }
}

pub fn select_menu_horizontal(options: &[String]) -> Result<Option<usize>> {
    let mut stdout = stdout();
    let _ = stdout.execute(crossterm::cursor::Hide);

    enable_raw_mode()?;
    drain_pending_key_events();
    let mut selected = 0usize;
    let num_options = options.len();

    let mut draw_menu = |selected_idx: usize| -> Result<()> {
        print!("\r\x1b[2K{}  L ", AURA_SLATE);

        for (i, option) in options.iter().enumerate().take(num_options) {
            match i > 0 {
                true => print!("{}  ·  {}", AURA_SLATE, COLOR_RESET),
                false => {}
            }
            match i == selected_idx {
                true => print!(
                    "{}{}{}▸ {}{}{}",
                    RED_ORANGE, COLOR_BOLD, COLOR_RESET, COLOR_BOLD, option, COLOR_RESET
                ),
                false => print!("{}{}{}", LIGHT_WHITE, option, COLOR_RESET),
            }
        }
        stdout.flush()?;
        Ok(())
    };

    draw_menu(selected)?;

    let finish_horizontal = |chosen: usize, stdout: &mut std::io::Stdout| -> Result<Option<usize>> {
        let _ = stdout.execute(crossterm::cursor::Show);
        print!("\r\x1b[2K");
        let outcome = match chosen {
            0 => format!("{}approved", AURA_GREEN),
            1 => format!("{}approved & trusted", AURA_GREEN),
            _ => format!("{}denied", AURA_ROSE),
        };
        print!("{}  L {}{}{}\r\n", AURA_SLATE, COLOR_RESET, outcome, COLOR_RESET);
        let _ = stdout.flush();
        disable_raw_mode()?;
        Ok(Some(chosen))
    };

    loop {
        match event::poll(std::time::Duration::from_millis(50))? {
            true => match event::read()? {
                Event::Key(key_event) => match classify_horizontal_menu_key(&key_event) {
                    MenuAction::MovePrev => {
                        selected = (selected + num_options - 1) % num_options;
                        draw_menu(selected)?;
                    }
                    MenuAction::MoveNext => {
                        selected = (selected + 1) % num_options;
                        draw_menu(selected)?;
                    }
                    MenuAction::QuickSelect(idx) => match idx < num_options {
                        true => return finish_horizontal(idx, &mut stdout),
                        false => {}
                    },
                    MenuAction::Confirm => {
                        return finish_horizontal(selected, &mut stdout);
                    }
                    MenuAction::Cancel => {
                        let _ = stdout.execute(crossterm::cursor::Show);
                        print!("\r\x1b[2K{}  L {}denied{}\r\n", AURA_SLATE, AURA_ROSE, COLOR_RESET);
                        let _ = stdout.flush();
                        disable_raw_mode()?;
                        return Ok(None);
                    }
                    MenuAction::Interrupt => {
                        let _ = stdout.execute(crossterm::cursor::Show);
                        print!("\r\x1b[2K");
                        disable_raw_mode()?;
                        return Err(anyhow::anyhow!("Ctrl-C"));
                    }
                    MenuAction::Ignore => {}
                },
                _ => {}
            },
            false => {}
        }
    }
}
