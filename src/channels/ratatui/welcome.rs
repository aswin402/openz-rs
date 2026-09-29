use super::app::RatatuiApp;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Renders the Zen Welcome Screen when the conversation timeline is empty.
pub fn render_welcome_screen(
    frame: &mut Frame,
    app: &RatatuiApp,
    area: Rect,
) {
    let theme = &app.theme;

    // 1. Fill entire background
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(theme.bg_primary));
    frame.render_widget(bg_block, area);

    // Fallback for constrained terminal dimensions
    if area.height < 12 || area.width < 34 {
        return;
    }

    // Vertical layout hierarchy
    let vert_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // 0: Top spacer
            Constraint::Length(4), // 1: Brand logo lockup
            Constraint::Length(1), // 2: Spacer
            Constraint::Length(3), // 3: Centered Dynamic Input Dock
            Constraint::Length(2), // 4: Recommended command hints
            Constraint::Min(2),    // 5: Bottom spacer
            Constraint::Length(1), // 6: Bottom Edge Bar
        ])
        .split(area);

    // ── 1. Brand Logo Lockup ────────────────────────────────────────────────
    let version_str = format!("OpenZ v{} • Ready", env!("CARGO_PKG_VERSION"));
    let slogan = "High-Performance Local-First AI Agent";
    let max_text_len = slogan.chars().count().max(version_str.chars().count());
    let lockup_width = (18 + max_text_len) as u16;
    let lockup_area = if vert_chunks[1].width > lockup_width {
        let offset_x = (vert_chunks[1].width - lockup_width) / 2;
        Rect {
            x: vert_chunks[1].x + offset_x,
            y: vert_chunks[1].y,
            width: lockup_width,
            height: 4,
        }
    } else {
        vert_chunks[1]
    };

    let line1 = Line::from(vec![
        Span::styled("████", Style::default().fg(theme.brand_accent)),
        Span::raw("    "),
        Span::styled("████", Style::default().fg(theme.info)),
        Span::raw("   "),
        Span::styled(
            "OpenZ 🦊",
            Style::default()
                .fg(theme.brand_white)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let line2 = Line::from(vec![
        Span::styled("████", Style::default().fg(theme.brand_accent)),
        Span::raw("    "),
        Span::styled("████", Style::default().fg(theme.info)),
        Span::raw("   "),
        Span::styled(slogan, Style::default().fg(theme.muted)),
    ]);

    let line3 = Line::from(vec![
        Span::styled("██  ", Style::default().fg(theme.brand_accent)),
        Span::raw("    "),
        Span::styled("  ██", Style::default().fg(theme.info)),
        Span::raw("   "),
        Span::styled(version_str, Style::default().fg(theme.brand_accent)),
    ]);

    let logo_para = Paragraph::new(vec![line1, line2, line3]);
    frame.render_widget(logo_para, lockup_area);

    // ── 2. Centered Input Dock ──────────────────────────────────────────────
    let input_width = (vert_chunks[3].width * 65 / 100)
        .clamp(42, 76)
        .min(vert_chunks[3].width.saturating_sub(4));
    let input_x = vert_chunks[3].x + (vert_chunks[3].width.saturating_sub(input_width)) / 2;
    let centered_input_rect = Rect {
        x: input_x,
        y: vert_chunks[3].y,
        width: input_width,
        height: vert_chunks[3].height,
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.bg_input));

    let inner = input_block.inner(centered_input_rect);
    frame.render_widget(input_block, centered_input_rect);

    let max_width = inner.width.saturating_sub(3).max(1) as usize;

    let input_line = if app.typed_input.is_empty() {
        Line::from(vec![
            Span::styled(
                "› ",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Ask OpenZ anything or type '/' for commands...",
                Style::default().fg(theme.muted),
            ),
        ])
    } else {
        let input_str: String = app.typed_input.iter().collect();
        let display = if input_str.len() > max_width {
            let start = input_str.len().saturating_sub(max_width);
            format!("…{}", &input_str[start..])
        } else {
            input_str
        };
        Line::from(vec![
            Span::styled(
                "› ",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(display, Style::default().fg(theme.brand_white)),
        ])
    };

    frame.render_widget(Paragraph::new(input_line), inner);

    // Set cursor position inside the centered input box
    let cursor_col = if app.typed_input.is_empty() {
        2
    } else {
        let visible_len = app.typed_input.len().min(max_width);
        2 + if app.typed_input.len() > max_width {
            visible_len
        } else {
            app.cursor_idx
        }
    };
    frame.set_cursor_position((inner.x + cursor_col as u16, inner.y));

    // ── 3. Quick Command Starter Chips ──────────────────────────────────────
    let hints_line = Line::from(vec![
        Span::styled(" [Ctrl+L] ", Style::default().fg(theme.warning)),
        Span::styled("/model  ", Style::default().fg(theme.muted)),
        Span::styled(" [Ctrl+H] ", Style::default().fg(theme.warning)),
        Span::styled("/history  ", Style::default().fg(theme.muted)),
        Span::styled(" [Ctrl+N] ", Style::default().fg(theme.warning)),
        Span::styled("/new-session  ", Style::default().fg(theme.muted)),
        Span::styled(" [F1] ", Style::default().fg(theme.info)),
        Span::styled("/commands", Style::default().fg(theme.muted)),
    ]);
    let hints_para = Paragraph::new(hints_line).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(hints_para, vert_chunks[4]);

    // ── 4. Bottom Edge Bar ──────────────────────────────────────────────────
    let mut left_spans = vec![
        Span::raw(" "),
        Span::styled(
            app.cwd_display.clone(),
            Style::default().fg(theme.info),
        ),
    ];

    if let Some(branch) = RatatuiApp::get_git_branch(&app.workspace_root) {
        left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        left_spans.push(Span::styled(
            format!("git:{}", branch),
            Style::default().fg(theme.success),
        ));
    }

    let (mcp_loaded, _, _) = crate::tools::mcp::get_mcp_stats();
    if mcp_loaded > 0 {
        left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        left_spans.push(Span::styled(
            format!("mcp:{} active", mcp_loaded),
            Style::default().fg(theme.brand_accent),
        ));
    }

    let provider_model = format!("{}:{}", app.provider, app.model);
    let right_spans = vec![
        Span::styled(
            provider_model,
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];

    let bottom_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(35)])
        .split(vert_chunks[6]);

    frame.render_widget(Paragraph::new(Line::from(left_spans)), bottom_layout[0]);
    frame.render_widget(
        Paragraph::new(Line::from(right_spans)).alignment(ratatui::layout::Alignment::Right),
        bottom_layout[1],
    );
}
