use super::app::RatatuiApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_ratatui_ui(f: &mut Frame, app: &RatatuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Conversation Scrollback + Header
            Constraint::Length(3), // Highlighted Input Box
            Constraint::Length(1), // Status Line with Embedded Pill
        ])
        .split(f.area());

    // 1. Chunk 0: Conversation Scrollback + Header
    let mut text_lines = Vec::new();

    // 3D OPENZ ASCII logo banner lines (White letters lines 0-1 and Red-Orange lines 2-5)
    text_lines.push(Line::from(vec![Span::styled(
        "  ██████╗ ██████╗ ███████╗███╗   ██╗███████╗",
        Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        " ██╔═══██╗██╔══██╗██╔════╝████╗  ██║╚══███╔╝",
        Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        " ██║   ██║██████╔╝█████╗  ██╔██╗ ██║  ███╔╝ ",
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        " ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║ ███╔╝  ",
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        " ╚██████╔╝██║     ███████╗██║ ╚████║███████╗",
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));
    text_lines.push(Line::from(vec![Span::styled(
        "  ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝╚══════╝",
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));

    // Version text: openz v0.0.115 in RED_ORANGE (Rgb(255, 85, 51))
    text_lines.push(Line::from(vec![Span::styled(
        format!(" openz v{}", env!("CARGO_PKG_VERSION")),
        Style::default().fg(Color::Rgb(255, 85, 51)).add_modifier(Modifier::BOLD),
    )]));

    // Provider & Model line: {} | {} in AURA_SLATE (Rgb(98, 114, 164))
    text_lines.push(Line::from(vec![Span::styled(
        format!(" {} | {}", app.provider, app.model),
        Style::default().fg(Color::Rgb(98, 114, 164)),
    )]));

    // Working directory line: ~ or path in AURA_SLATE
    text_lines.push(Line::from(vec![Span::styled(
        format!(" {}", app.cwd_display),
        Style::default().fg(Color::Rgb(98, 114, 164)),
    )]));

    // Top thin divider rule line
    text_lines.push(Line::from(vec![Span::styled(
        " ─────────────────────────────────────────────────────────────────────────────",
        Style::default().fg(Color::Rgb(62, 72, 104)),
    )]));

    // Conversation messages from app.messages
    for msg in &app.messages {
        if msg.role == "user" {
            let lines: Vec<&str> = if msg.content.is_empty() {
                vec![""]
            } else {
                msg.content.lines().collect()
            };
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
            let lines: Vec<&str> = if msg.content.is_empty() {
                vec![""]
            } else {
                msg.content.lines().collect()
            };
            for line in lines {
                if line.starts_with('⚠') {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled("⚠", Style::default().fg(Color::Rgb(241, 250, 140))),
                        Span::styled(
                            &line["⚠".len()..],
                            Style::default().fg(Color::Rgb(241, 250, 140)),
                        ),
                    ]));
                } else if line.contains("Warning:") {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled("⚠ ", Style::default().fg(Color::Rgb(241, 250, 140))),
                        Span::styled(
                            line,
                            Style::default().fg(Color::Rgb(241, 250, 140)),
                        ),
                    ]));
                } else if line.starts_with("- ") {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled("- ", Style::default().fg(Color::Rgb(80, 250, 123))),
                        Span::styled(
                            &line[2..],
                            Style::default().fg(Color::Rgb(248, 248, 242)),
                        ),
                    ]));
                } else if msg.is_tool || msg.role == "tool" {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            line,
                            Style::default().fg(Color::Rgb(189, 147, 249)), // AURA_PURPLE
                        ),
                    ]));
                } else {
                    text_lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            line,
                            Style::default().fg(Color::Rgb(248, 248, 242)), // LIGHT_WHITE
                        ),
                    ]));
                }
            }
        }
    }

    let conversation = Paragraph::new(Text::from(text_lines))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset as u16, 0));
    f.render_widget(conversation, chunks[0]);

    // 2. Chunk 1: Highlighted Input Box with Wrapping & Cursor Position
    let max_width = chunks[1].width.saturating_sub(3).max(1) as usize;
    let mut input_lines = Vec::new();
    if app.typed_input.is_empty() {
        input_lines.push(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Rgb(255, 85, 51))),
        ]));
    } else {
        let chunks_chars: Vec<&[char]> = app.typed_input.chunks(max_width).collect();
        for (i, chunk) in chunks_chars.iter().enumerate() {
            let prefix = if i == 0 { "> " } else { "- " };
            let prefix_color = if i == 0 { Color::Rgb(255, 85, 51) } else { Color::Rgb(98, 114, 164) };
            let line_content: String = chunk.iter().collect();
            input_lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(prefix_color)),
                Span::styled(line_content, Style::default().fg(Color::Rgb(255, 255, 255))),
            ]));
        }
    }

    let input_block = Block::default().style(Style::default().bg(Color::Rgb(25, 25, 35)));
    let input_p = Paragraph::new(Text::from(input_lines))
        .block(input_block);
    f.render_widget(input_p, chunks[1]);

    let cursor_row = app.cursor_idx / max_width;
    let cursor_col = app.cursor_idx % max_width;
    f.set_cursor_position((
        chunks[1].x + 2 + cursor_col as u16,
        chunks[1].y + cursor_row as u16,
    ));

    // 3. Chunk 2: Status Line with Embedded Pill
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
        Span::styled(rule_str, Style::default().fg(Color::Rgb(62, 72, 104))),
        Span::styled(
            format!("[ ◇ MCP {}✓ ", mcp_loaded),
            Style::default().fg(Color::Rgb(189, 147, 249)), // AURA_PURPLE
        ),
        Span::styled(
            format!("| {} | {} ", app.provider, app.model),
            Style::default().fg(Color::Rgb(255, 85, 51)), // RED_ORANGE
        ),
        Span::styled(
            format!("| {}/1M ]", app.approx_tokens),
            Style::default().fg(Color::Rgb(255, 85, 51)), // RED_ORANGE
        ),
    ]);

    let status_p = Paragraph::new(status_line);
    f.render_widget(status_p, chunks[2]);

    // 4. Slash Command Autocomplete Overlay
    let typed_str: String = app.typed_input.iter().collect();
    if typed_str.starts_with('/') {
        let matches = app.matching_slash_commands();
        if !matches.is_empty() {
            let popup_height = (matches.len() as u16 + 2).min(chunks[0].height);
            let popup_width = 30.min(chunks[1].width);
            let popup_area = Rect::new(
                chunks[1].x + 2,
                chunks[1].y.saturating_sub(popup_height),
                popup_width,
                popup_height,
            );

            f.render_widget(Clear, popup_area);

            let popup_block = Block::default()
                .title(Span::styled(
                    " Commands ",
                    Style::default()
                        .fg(Color::Rgb(189, 147, 249))
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(98, 114, 164)))
                .style(Style::default().bg(Color::Rgb(25, 25, 35)));

            let items: Vec<Line> = matches
                .iter()
                .enumerate()
                .map(|(idx, cmd)| {
                    let is_selected = app.selected_index == Some(idx);
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Rgb(255, 255, 255))
                            .bg(Color::Rgb(98, 114, 164))
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Rgb(80, 250, 123))
                    };
                    Line::from(Span::styled(format!(" {}", cmd), style))
                })
                .collect();

            let popup_p = Paragraph::new(items).block(popup_block);
            f.render_widget(popup_p, popup_area);
        }
    }
}
