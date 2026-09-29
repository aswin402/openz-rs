use super::app::RatatuiApp;
use super::modals::render_modal_overlay;
use super::theme;
use super::timeline::render_timeline;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
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

    // If conversation is empty and agent is idle, render Zen Welcome Screen (minicode style)
    if app.messages.is_empty() && !app.is_thinking {
        super::welcome::render_welcome_screen(f, app, f.area());

        // Floating Spotlight Slash Command Palette (if typing '/')
        if app.has_active_slash_query() && !app.modal.is_active() {
            render_slash_palette(f, app, f.area());
        }

        // Modal Dialogs (Overlay)
        if app.modal.is_active() {
            render_modal_overlay(f, app, f.area());
        }
        return;
    }

    // Layout: Conversation Timeline (flex) -> Elevated Input Dock (3) -> Bottom Status Bar (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    // 1. Conversation Timeline
    render_timeline(f, app, chunks[0]);

    // 2. Elevated Input Dock
    render_input_dock(f, app, chunks[1]);

    // 3. Minimal Bottom Status Line
    render_status_bar(f, app, chunks[2]);

    // 4. Floating Spotlight Slash Command Palette (if typing '/')
    if app.has_active_slash_query() && !app.modal.is_active() {
        render_slash_palette(f, app, f.area());
    }

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

// ── Floating Spotlight Slash Command Palette ─────────────────────────────────

fn render_slash_palette(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    if !app.has_active_slash_query() {
        return;
    }

    let theme = &app.theme;
    let matches = app.matching_palette_commands();

    // Modal dimensions (responsive spotlight centered in upper-middle of screen)
    let width = 68.min(area.width.saturating_sub(4)).max(48);
    let height = 11.min(area.height.saturating_sub(4)).max(6);

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 3;
    let popup_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup_area);

    // Build Title Bar with Category Radio Tabs on right
    let current_cat_idx = app.slash_category_idx % super::app::CommandCategory::all().len();
    let mut title_spans = vec![
        Span::styled(
            " ⌘ Commands ",
            Style::default()
                .fg(theme.brand_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("[Tab] ", Style::default().fg(theme.muted)),
    ];

    for (idx, cat) in super::app::CommandCategory::all().iter().enumerate() {
        let is_active = idx == current_cat_idx;
        if is_active {
            title_spans.push(Span::styled(
                format!("◉ {} ", cat.label()),
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            title_spans.push(Span::styled(
                format!("○ {} ", cat.label()),
                Style::default().fg(theme.muted),
            ));
        }
    }
    title_spans.push(Span::raw(" "));

    let outer_block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(theme.brand_accent))
        .style(Style::default().bg(theme.bg_elevated));

    let inner_area = outer_block.inner(popup_area);
    f.render_widget(outer_block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Search row: › /search█
            Constraint::Length(1), // Divider
            Constraint::Min(3),    // Commands list
        ])
        .split(inner_area);

    // 1. Search Query Row
    let typed_text: String = app.typed_input.iter().collect();
    let search_line = Line::from(vec![
        Span::styled(
            "  › ",
            Style::default()
                .fg(theme.brand_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if typed_text.is_empty() { "/" } else { &typed_text },
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("█", Style::default().fg(theme.brand_accent)),
    ]);
    f.render_widget(Paragraph::new(search_line), chunks[0]);

    // Top Divider
    let divider = Paragraph::new(Line::from(vec![Span::styled(
        "─".repeat(inner_area.width as usize),
        Style::default().fg(theme.border),
    )]));
    f.render_widget(divider, chunks[1]);

    // 2. Command Items List
    let list_height = chunks[2].height as usize;
    let selected_idx = app.slash_selected_idx.min(matches.len().saturating_sub(1));
    let scroll_offset = super::modals::compute_scroll_offset(selected_idx, list_height);

    let mut item_lines = Vec::new();
    let inner_width = inner_area.width as usize;

    if matches.is_empty() {
        item_lines.push(Line::from(vec![Span::styled(
            "   No matching commands found",
            Style::default().fg(theme.muted),
        )]));
    } else {
        for (i, &cmd_idx) in matches
            .iter()
            .skip(scroll_offset)
            .take(list_height)
            .enumerate()
        {
            let actual_idx = scroll_offset + i;
            let is_selected = actual_idx == selected_idx;
            let cmd = &super::app::PALETTE_COMMANDS[cmd_idx];
            let shortcut_str = cmd.shortcut.unwrap_or("");

            let prefix = if is_selected { " ❯ " } else { "   " };
            let left_content = format!("{}{:<13} {}", prefix, cmd.slash_name, cmd.title);
            let left_width = left_content.chars().count();
            let shortcut_width = shortcut_str.chars().count();

            let avail_space = inner_width.saturating_sub(left_width + shortcut_width + 2);
            let padding = " ".repeat(avail_space);

            if is_selected {
                let line_str = format!("{}{}{}", left_content, padding, shortcut_str);
                let current_width = line_str.chars().count();
                let trailing_spaces = " ".repeat(inner_width.saturating_sub(current_width));
                let full_padded = format!("{}{}", line_str, trailing_spaces);
                item_lines.push(Line::from(vec![Span::styled(
                    full_padded,
                    Style::default()
                        .bg(theme.brand_accent)
                        .fg(theme.bg_primary)
                        .add_modifier(Modifier::BOLD),
                )]));
            } else {
                let mut spans = vec![
                    Span::styled(left_content, Style::default().fg(theme.text_primary)),
                    Span::raw(padding),
                ];
                if !shortcut_str.is_empty() {
                    spans.push(Span::styled(shortcut_str, Style::default().fg(theme.warning)));
                }
                spans.push(Span::raw(" "));
                item_lines.push(Line::from(spans));
            }
        }
    }

    let list_p = Paragraph::new(item_lines);
    f.render_widget(list_p, chunks[2]);
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

    // MCP status pill
    footer_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
    if !mcp_done {
        let frame_idx = app.spinner_idx % theme::SPINNER_FRAMES.len();
        footer_spans.push(Span::styled(
            format!("◇ MCP {} ", theme::SPINNER_FRAMES[frame_idx]),
            Style::default().fg(theme.warning),
        ));
    } else if mcp_failed == 0 {
        footer_spans.push(Span::styled(
            format!("◇ MCP {}✓", mcp_loaded),
            Style::default().fg(theme.brand_accent),
        ));
    } else {
        footer_spans.push(Span::styled(
            format!("◇ MCP {}✓ {}✗", mcp_loaded, mcp_failed),
            Style::default().fg(theme.destructive),
        ));
    }

    // Token context & Dynamic Context Window
    let limit_tokens = crate::providers::DynamicContextRegistry::resolve_context_window(
        &app.model,
        &crate::config::schema::Config::default(),
    );
    let limit_str = if limit_tokens >= 1_000_000 {
        format!("{}M", limit_tokens / 1_000_000)
    } else {
        format!("{}K", limit_tokens / 1000)
    };
    let approx_tokens_str = if app.approx_tokens >= 1000 {
        format!("{:.1}K", app.approx_tokens as f64 / 1000.0)
    } else {
        format!("{}", app.approx_tokens)
    };

    let ratio = if limit_tokens > 0 {
        (app.approx_tokens as f64) / (limit_tokens as f64)
    } else {
        0.0
    };
    let token_color = if ratio > 0.85 {
        theme.destructive
    } else if ratio > 0.60 {
        theme.warning
    } else {
        theme.success
    };

    footer_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
    footer_spans.push(Span::styled(
        format!("{}/{}", approx_tokens_str, limit_str),
        Style::default().fg(token_color),
    ));

    // Queued prompts indicator
    if !app.queued_prompts.is_empty() {
        footer_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        footer_spans.push(Span::styled(
            format!("[{} queued]", app.queued_prompts.len()),
            Style::default()
                .fg(theme.brand_accent)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Background servers indicator
    let bg_servers = crate::shutdown::list_registered_children();
    if !bg_servers.is_empty() {
        footer_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        footer_spans.push(Span::styled(
            format!("⚙ {} srv", bg_servers.len()),
            Style::default().fg(theme.info),
        ));
    }

    // Right-aligned quick shortcut hints if terminal width allows
    let right_hints = "[F1] Help · [Ctrl+L] Model · [Ctrl+H] History · [Ctrl+C] Exit";
    let left_len: usize = footer_spans.iter().map(|s| s.width()).sum();
    let available = area.width as usize;
    if available > left_len + right_hints.len() + 4 {
        let padding = available - left_len - right_hints.len() - 1;
        footer_spans.push(Span::raw(" ".repeat(padding)));
        footer_spans.push(Span::styled(right_hints, Style::default().fg(theme.muted)));
    }

    let footer_line = Line::from(footer_spans);
    let p = Paragraph::new(footer_line).block(Block::default().borders(Borders::NONE));
    f.render_widget(p, area);
}
