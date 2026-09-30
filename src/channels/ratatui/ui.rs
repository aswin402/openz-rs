use super::app::RatatuiApp;
use super::modals::render_modal_overlay;
use super::theme;
use super::timeline::render_timeline;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

// ── Main Layout Renderer ────────────────────────────────────────────────────

pub fn render_ratatui_ui(f: &mut Frame, app: &mut RatatuiApp) {
    // Background fill
    let bg_color = app.theme.bg_primary;
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(bg_color));
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

    // Layout: Conversation Timeline (flex) -> Live Activity (2 or 0) -> Elevated Input Dock (3) -> Spacer (1) -> Bottom Status Bar (1)
    let activity_height = if app.is_thinking { 2 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),                     // 0: Conversation Timeline
            Constraint::Length(activity_height),    // 1: Live Activity Indicator (Thinking / Generating / Working)
            Constraint::Length(3),                  // 2: Elevated Input Dock
            Constraint::Length(1),                  // 3: Spacer below input dock
            Constraint::Length(1),                  // 4: Minimal Bottom Status Line
        ])
        .split(f.area());

    // 1. Conversation Timeline
    render_timeline(f, app, chunks[0]);

    let theme = &app.theme;

    // 2. Live Activity Indicator (pinned directly above input dock)
    if app.is_thinking {
        let elapsed_secs = app.work_start.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
        let activity = app
            .current_activity
            .as_ref()
            .unwrap_or(&super::animation::AgentActivity::Thinking);
        let live_line = super::animation::render_live_activity_line(
            activity,
            app.spinner_style,
            app.elapsed_millis(),
            elapsed_secs,
            theme,
        );
        let mut padded_spans = vec![Span::raw(" ")];
        padded_spans.extend(live_line.spans);
        let activity_lines = vec![Line::from(padded_spans), Line::from(String::new())];
        f.render_widget(
            Paragraph::new(activity_lines).style(Style::default().bg(theme.bg_primary)),
            chunks[1],
        );
    }

    // 3. Elevated Input Dock
    render_input_dock(f, app, chunks[2]);

    // 4. Minimal Bottom Status Line
    render_status_bar(f, app, chunks[4]);

    // 5. Floating Spotlight Slash Command Palette (if typing '/')
    if app.has_active_slash_query() && !app.modal.is_active() {
        render_slash_palette(f, app, f.area());
    }

    // 6. Modal Dialogs (Overlay)
    if app.modal.is_active() {
        render_modal_overlay(f, app, f.area());
    }
}

// ── Elevated Input Dock (minicode style) ────────────────────────────────────

fn render_input_dock(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
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

    let provider_model = if app.model.starts_with(&app.provider) {
        app.model.clone()
    } else {
        format!("{}:{}", app.provider, app.model)
    };

    let mut left_spans = vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            provider_model,
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" | ", Style::default().fg(theme.muted)),
        Span::styled(app.cwd_display.clone(), Style::default().fg(theme.info)),
    ];

    if let Some(branch) = RatatuiApp::get_git_branch(&app.workspace_root) {
        left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        left_spans.push(Span::styled(
            format!("git:{}", branch),
            Style::default().fg(theme.success),
        ));
    }

    // MCP status pill
    left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
    if !mcp_done {
        let frame_idx = app.spinner_idx % theme::SPINNER_FRAMES.len();
        left_spans.push(Span::styled(
            format!("◇ MCP {} ", theme::SPINNER_FRAMES[frame_idx]),
            Style::default().fg(theme.warning),
        ));
    } else if mcp_failed == 0 {
        left_spans.push(Span::styled(
            format!("◇ MCP {}✓", mcp_loaded),
            Style::default().fg(theme.brand_accent),
        ));
    } else {
        left_spans.push(Span::styled(
            format!("◇ MCP {}✓ {}✗", mcp_loaded, mcp_failed),
            Style::default().fg(theme.destructive),
        ));
    }

    // Queued prompts indicator
    if !app.queued_prompts.is_empty() {
        left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        left_spans.push(Span::styled(
            format!("[{} queued]", app.queued_prompts.len()),
            Style::default()
                .fg(theme.brand_accent)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Background servers indicator
    let bg_servers = crate::shutdown::list_registered_children();
    if !bg_servers.is_empty() {
        left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        left_spans.push(Span::styled(
            format!("⚙ {} srv", bg_servers.len()),
            Style::default().fg(theme.info),
        ));
    }

    // Token context & Dynamic Context Window (Right Side)
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

    let right_spans = vec![
        Span::styled(
            approx_tokens_str,
            Style::default()
                .fg(token_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" / ", Style::default().fg(theme.muted)),
        Span::styled(limit_str, Style::default().fg(theme.muted)),
        Span::styled(" ", Style::default()),
    ];

    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),
            Constraint::Length(25),
        ])
        .split(area);

    let left_p = Paragraph::new(Line::from(left_spans)).block(Block::default().borders(Borders::NONE));
    f.render_widget(left_p, status_chunks[0]);

    let right_p = Paragraph::new(Line::from(right_spans))
        .block(Block::default().borders(Borders::NONE))
        .alignment(ratatui::layout::Alignment::Right);
    f.render_widget(right_p, status_chunks[1]);
}
