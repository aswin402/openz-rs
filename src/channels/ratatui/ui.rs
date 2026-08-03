use super::app::RatatuiApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_ratatui_ui(f: &mut Frame, app: &RatatuiApp) {
    let matches = app.matching_slash_commands();
    let has_popup = !app.typed_input.is_empty() && app.typed_input[0] == '/' && !matches.is_empty();

    let popup_lines_count = if has_popup {
        (matches.len().min(5) + 3) as u16
    } else {
        0
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),                     // Conversation Scrollback + Header
            Constraint::Length(1),                  // Input Line
            Constraint::Length(1),                  // Status Line with Pill
            Constraint::Length(popup_lines_count),  // Autocomplete Menu Popup
        ])
        .split(f.area());

    // 1. Chunk 0: Conversation Scrollback + Header
    let mut text_lines = Vec::new();

    // OPENZ 3D ASCII Logo
    text_lines.push(Line::from(vec![Span::styled(
        "     ██████╗ ██████╗ ███████╗███╗   ██╗███████╗",
        Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        "    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║╚══███╔╝",
        Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![
        Span::styled(
            "    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║  ",
            Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "███╔╝ ",
            Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
        ),
    ]));
    text_lines.push(Line::from(vec![
        Span::styled(
            "    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║ ",
            Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "███╔╝  ",
            Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
        ),
    ]));
    text_lines.push(Line::from(vec![
        Span::styled(
            "    ╚██████╔╝██║     ███████╗██║ ╚████║",
            Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "███████╗",
            Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
        ),
    ]));
    text_lines.push(Line::from(vec![Span::styled(
        "     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝╚══════╝",
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));

    // Version text: openz v0.0.116
    text_lines.push(Line::from(vec![Span::styled(
        format!(" openz v{}", env!("CARGO_PKG_VERSION")),
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));

    // Provider & Model line: opencode_zen | deepseek-v4-flash-free
    text_lines.push(Line::from(vec![Span::styled(
        format!(" {} | {}", app.provider, app.model),
        Style::default().fg(Color::Rgb(98, 114, 164)),
    )]));

    // Working directory line
    text_lines.push(Line::from(vec![Span::styled(
        format!(" {}", app.cwd_display),
        Style::default().fg(Color::Rgb(98, 114, 164)),
    )]));

    // Top thin divider rule line
    let term_width = chunks[0].width as usize;
    let divider_width = term_width.min(60);
    text_lines.push(Line::from(vec![Span::styled(
        "─".repeat(divider_width),
        Style::default().fg(Color::Rgb(98, 114, 164)),
    )]));

    // Render message stream
    for msg in &app.messages {
        let lines: Vec<&str> = if msg.content.is_empty() {
            vec![""]
        } else {
            msg.content.lines().collect()
        };

        if msg.role == "user" {
            for (i, line) in lines.into_iter().enumerate() {
                if i == 0 {
                    text_lines.push(Line::from(vec![
                        Span::styled("> ", Style::default().fg(Color::Rgb(255, 85, 51))),
                        Span::styled(
                            line,
                            Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
                        ),
                    ]));
                } else {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            line,
                            Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
            }
        } else {
            for line in lines {
                if line.starts_with('⚠') || line.contains("Warning:") {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled("⚠ ", Style::default().fg(Color::Rgb(241, 250, 140))),
                        Span::styled(line, Style::default().fg(Color::Rgb(241, 250, 140))),
                    ]));
                } else if line.starts_with("- ") {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled("- ", Style::default().fg(Color::Rgb(80, 250, 123))),
                        Span::styled(&line[2..], Style::default().fg(Color::Rgb(248, 248, 242))),
                    ]));
                } else if msg.is_tool || msg.role == "tool" {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(line, Style::default().fg(Color::Rgb(189, 147, 249))),
                    ]));
                } else {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(line, Style::default().fg(Color::Rgb(248, 248, 242))),
                    ]));
                }
            }
        }
    }

    let conversation = Paragraph::new(Text::from(text_lines))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset as u16, 0));
    f.render_widget(conversation, chunks[0]);

    // 2. Chunk 1: Input Line (Transparent terminal background matching openz agent)
    let max_width = chunks[1].width.saturating_sub(3).max(1) as usize;
    let mut input_lines = Vec::new();
    if app.typed_input.is_empty() {
        input_lines.push(Line::from(vec![Span::styled(
            "> ",
            Style::default().fg(Color::Rgb(255, 255, 255)),
        )]));
    } else {
        let chunks_chars: Vec<&[char]> = app.typed_input.chunks(max_width).collect();
        for (i, chunk) in chunks_chars.iter().enumerate() {
            let prefix = if i == 0 { "> " } else { "- " };
            let line_content: String = chunk.iter().collect();
            input_lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Rgb(255, 255, 255))),
                Span::styled(line_content, Style::default().fg(Color::Rgb(255, 255, 255))),
            ]));
        }
    }

    let input_p = Paragraph::new(Text::from(input_lines));
    f.render_widget(input_p, chunks[1]);

    let cursor_row = app.cursor_idx / max_width;
    let cursor_col = app.cursor_idx % max_width;
    f.set_cursor_position((
        chunks[1].x + 2 + cursor_col as u16,
        chunks[1].y + cursor_row as u16,
    ));

    // 3. Chunk 2: Status Line with Embedded Right-Aligned Pill
    let (mcp_loaded, _, _) = crate::tools::mcp::get_mcp_stats();
    let pill_text = format!(
        "[ ◇ MCP {}✓ | {} | {} | {}/1M ]",
        mcp_loaded, app.provider, app.model, app.approx_tokens
    );
    let total_width = chunks[2].width as usize;
    let pill_len = pill_text.chars().count();
    let rule_len = total_width.saturating_sub(pill_len);
    let rule_str: String = "─".repeat(rule_len);

    let status_line = Line::from(vec![
        Span::styled(rule_str, Style::default().fg(Color::Rgb(98, 114, 164))),
        Span::styled(
            format!("[ ◇ MCP {}✓ ", mcp_loaded),
            Style::default().fg(Color::Rgb(189, 147, 249)),
        ),
        Span::styled(
            format!("| {} | {} ", app.provider, app.model),
            Style::default().fg(Color::Rgb(255, 85, 51)),
        ),
        Span::styled(
            format!("| {}/1M ]", app.approx_tokens),
            Style::default().fg(Color::Rgb(255, 85, 51)),
        ),
    ]);

    let status_p = Paragraph::new(status_line);
    f.render_widget(status_p, chunks[2]);

    // 4. Chunk 3: Autocomplete Overlay matching classic CLI menu
    if has_popup {
        let mut popup_lines = Vec::new();
        let display_limit = 5;
        let start_idx = if let Some(idx) = app.selected_index {
            if idx >= display_limit {
                idx - display_limit + 1
            } else {
                0
            }
        } else {
            0
        };
        let end_idx = (start_idx + display_limit).min(matches.len());

        for (i, (cmd, desc)) in matches.iter().enumerate().take(end_idx).skip(start_idx) {
            let is_selected = app.selected_index == Some(i);
            if is_selected {
                popup_lines.push(Line::from(vec![
                    Span::styled("> ", Style::default().fg(Color::Rgb(255, 85, 51))),
                    Span::styled(
                        format!("{:<30}", cmd),
                        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(Color::Rgb(98, 114, 164))),
                ]));
            } else {
                popup_lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        format!("{:<30}", cmd),
                        Style::default().fg(Color::Rgb(248, 248, 242)),
                    ),
                    Span::styled(*desc, Style::default().fg(Color::Rgb(98, 114, 164))),
                ]));
            }
        }

        popup_lines.push(Line::from(Span::styled(
            "  ↑/↓ Navigate · enter Select · tab Complete",
            Style::default().fg(Color::Rgb(98, 114, 164)),
        )));

        let help_p = Paragraph::new(Text::from(popup_lines));
        f.render_widget(help_p, chunks[3]);
    }
}
