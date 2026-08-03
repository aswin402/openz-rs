use super::app::RatatuiApp;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Wrap},
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

    // 3D OPENZ ASCII logo banner lines (White letters and Red-Orange Z / accents)
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
        " ╚██████╔╝██║     ███████╗██║ ╚████║███████╗",
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
            text_lines.push(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Rgb(255, 85, 51))),
                Span::styled(
                    &msg.content,
                    Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD),
                ),
            ]));
        } else if msg.is_tool || msg.role == "tool" {
            text_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    &msg.content,
                    Style::default().fg(Color::Rgb(189, 147, 249)), // AURA_PURPLE
                ),
            ]));
        } else {
            text_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    &msg.content,
                    Style::default().fg(Color::Rgb(248, 248, 242)), // LIGHT_WHITE
                ),
            ]));
        }
    }

    let conversation = Paragraph::new(Text::from(text_lines))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset as u16, 0));
    f.render_widget(conversation, chunks[0]);

    // 2. Chunk 1: Highlighted Input Box
    let typed_str: String = app.typed_input.iter().collect();
    let input_block = Block::default().style(Style::default().bg(Color::Rgb(25, 25, 35)));
    let input_p = Paragraph::new(format!("\n> {}", typed_str))
        .block(input_block)
        .style(Style::default().fg(Color::Rgb(255, 255, 255)));
    f.render_widget(input_p, chunks[1]);

    // Cursor position at (chunks[1].x + 2 + app.cursor_idx, chunks[1].y + 1)
    let cursor_x = chunks[1].x + 2 + (app.cursor_idx as u16);
    let cursor_y = chunks[1].y + 1;
    f.set_cursor_position((cursor_x, cursor_y));

    // 3. Chunk 2: Status Line with Embedded Pill
    let (mcp_loaded, _, _) = crate::channels::cli::get_mcp_stats();
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
}
