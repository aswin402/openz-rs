use super::app::RatatuiApp;
use super::modals::render_modal_overlay;
use super::theme;
use super::timeline::render_timeline;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

// ── Main Layout Renderer ────────────────────────────────────────────────────

pub fn render_ratatui_ui(f: &mut Frame, app: &mut RatatuiApp) {
    let theme = &app.theme;

    // Background fill
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(theme.bg_primary));
    f.render_widget(bg_block, f.area());

    let matches = app.matching_slash_commands();
    let has_popup = !matches.is_empty();

    let popup_lines_count = if has_popup {
        (matches.len().min(5) + 2) as u16
    } else {
        0
    };

    // Layout: Conversation (flex) -> Slash suggestions (if typing /) -> Input Box (3) -> Bottom Status Bar (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(popup_lines_count),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    // 1. Conversation Timeline
    render_timeline(f, app, chunks[0]);

    // 2. Autocomplete Suggestions (placed right above input box)
    if has_popup {
        render_autocomplete_dock(f, app, &matches, chunks[1]);
    }

    // 3. Elevated Input Dock
    render_input_dock(f, app, chunks[2]);

    // 4. Minimal Bottom Status Line
    render_status_bar(f, app, chunks[3]);

    // 5. Modal Dialogs (Overlay)
    if app.modal.is_active() {
        render_modal_overlay(f, app, f.area());
    }
}

// ── Elevated Input Dock (minicode style) ────────────────────────────────────

fn render_input_dock(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.bg_input));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let max_width = inner.width.saturating_sub(3).max(1) as usize;

    let line = if app.typed_input.is_empty() {
        Line::from(vec![
            Span::styled(
                "› ",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Ask OpenZ anything or type '/' for slash commands...",
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

    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, inner);

    // Set cursor position inside the input box
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
    f.set_cursor_position((inner.x + cursor_col as u16, inner.y));
}

// ── Autocomplete Suggestions Dock ───────────────────────────────────────────

fn render_autocomplete_dock(
    f: &mut Frame,
    app: &RatatuiApp,
    matches: &[(String, String)],
    area: Rect,
) {
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(" Commands ")
        .title_alignment(Alignment::Left)
        .style(Style::default().bg(theme.bg_elevated));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let display_limit = 5;
    let selected_idx = app.selected_index.unwrap_or(0);
    let start_idx = if selected_idx >= display_limit {
        selected_idx - display_limit + 1
    } else {
        0
    };
    let end_idx = (start_idx + display_limit).min(matches.len());

    let mut list_lines = Vec::new();
    for (i, (cmd, desc)) in matches.iter().enumerate().take(end_idx).skip(start_idx) {
        let is_selected = app.selected_index == Some(i);
        if is_selected {
            list_lines.push(Line::from(vec![
                Span::styled(
                    " › ",
                    Style::default()
                        .fg(theme.brand_accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<20}", cmd),
                    Style::default()
                        .fg(theme.brand_accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc.as_str(), Style::default().fg(theme.text_primary)),
            ]));
        } else {
            list_lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    format!("{:<20}", cmd),
                    Style::default().fg(theme.text_primary),
                ),
                Span::styled(desc.as_str(), Style::default().fg(theme.muted)),
            ]));
        }
    }

    let p = Paragraph::new(Text::from(list_lines));
    f.render_widget(p, inner);
}

// ── Status Bar (Bottom line) ────────────────────────────────────────────────

fn render_status_bar(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let theme = &app.theme;
    let (mcp_loaded, mcp_failed, _mcp_total) = crate::tools::mcp::get_mcp_stats();
    let mcp_done = crate::channels::cli::mcp::is_mcp_done();

    let mut footer_spans = vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            app.model.clone(),
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(theme.muted)),
        Span::styled(app.cwd_display.clone(), Style::default().fg(theme.info)),
    ];

    if let Some(branch) = RatatuiApp::get_git_branch(&app.workspace_root) {
        footer_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        footer_spans.push(Span::styled(
            format!("git:{}", branch),
            Style::default().fg(theme.success),
        ));
    }

    // MCP status
    footer_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
    if !mcp_done {
        let frame_idx = app.spinner_idx % theme::SPINNER_FRAMES.len();
        footer_spans.push(Span::styled(
            format!("mcp:{} ", theme::SPINNER_FRAMES[frame_idx]),
            Style::default().fg(theme.brand_accent),
        ));
    } else if mcp_failed == 0 {
        footer_spans.push(Span::styled(
            format!("mcp:{} active", mcp_loaded),
            Style::default().fg(theme.brand_accent),
        ));
    } else {
        footer_spans.push(Span::styled(
            format!("mcp:{}✓ {}✗", mcp_loaded, mcp_failed),
            Style::default().fg(theme.destructive),
        ));
    }

    // Token context
    footer_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
    footer_spans.push(Span::styled(
        format!("{}/1M", app.approx_tokens),
        Style::default().fg(theme.info),
    ));

    let footer_line = Line::from(footer_spans);
    let p = Paragraph::new(footer_line).block(Block::default().borders(Borders::NONE));
    f.render_widget(p, area);
}
