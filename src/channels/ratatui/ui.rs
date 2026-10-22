use super::app::RatatuiApp;
use super::modals::render_modal_overlay;
use super::theme;
use super::timeline::render_timeline;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

// ── Main Layout Renderer ────────────────────────────────────────────────────

pub fn render_ratatui_ui(f: &mut Frame, app: &mut RatatuiApp) {
    // Blank entire canvas to prevent ghost/lingering cells during scrolling
    f.render_widget(Clear, f.area());

    // Background fill
    let bg_color = app.theme.bg_primary;
    let bg_block = Block::default()
        .borders(Borders::NONE)
        .style(Style::default().bg(bg_color));
    f.render_widget(bg_block, f.area());

    // When conversation is empty and agent is idle, render Zen Welcome Screen
    match app.messages.is_empty() && !app.is_thinking {
        true => {
            super::welcome::render_welcome_screen(f, app, f.area());
            render_overlays(f, app);
            return;
        }
        false => {}
    }

    // Layout: Conversation Timeline (flex) -> Live Activity (3 or 0) -> Elevated Input Dock (3..=8) -> Spacer (1) -> Bottom Status Bar (1)
    let activity_height = match app.is_thinking {
        true => 3,
        false => 0,
    };
    let dock_height = compute_input_dock_height(app, f.area().width);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),                  // 0: Conversation Timeline
            Constraint::Length(activity_height), // 1: Live Activity Indicator
            Constraint::Length(dock_height),     // 2: Elevated Input Dock
            Constraint::Length(1),               // 3: Spacer below input dock
            Constraint::Length(1),               // 4: Minimal Bottom Status Line
        ])
        .split(f.area());

    // 1. Conversation Timeline
    render_timeline(f, app, chunks[0]);

    let theme = &app.theme;

    // 2. Live Activity Indicator (pinned directly above input dock with top and bottom spacing)
    match app.is_thinking {
        true => {
            let elapsed_secs = app
                .work_start
                .map(|s| s.elapsed().as_secs_f64())
                .unwrap_or(0.0);
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
            let activity_lines = vec![
                Line::from(String::new()),
                Line::from(padded_spans),
                Line::from(String::new()),
            ];
            f.render_widget(
                Paragraph::new(activity_lines).style(Style::default().bg(theme.bg_primary)),
                chunks[1],
            );
        }
        false => {}
    }

    // 3. Elevated Input Dock
    render_input_dock_with_placeholder(
        f,
        app,
        chunks[2],
        "Ask OpenZ anything or type '/' for slash commands...",
    );

    // 4. Minimal Bottom Status Line
    render_status_bar(f, app, chunks[4]);

    // 5. Floating Spotlight Slash Command Palette & Modal Dialogs
    render_overlays(f, app);
}

fn render_overlays(f: &mut Frame, app: &RatatuiApp) {
    match (app.has_active_slash_query(), app.modal.is_active()) {
        (true, false) => render_slash_palette(f, app, f.area()),
        _ => {}
    }
    match app.modal.is_active() {
        true => render_modal_overlay(f, app, f.area()),
        false => {}
    }
}

// ── Shared Input Dock (used by both Zen Welcome Screen & Active Timeline) ───

pub(crate) const MAX_INPUT_DOCK_INNER_LINES: usize = 6;

pub(crate) fn wrap_input_chars_with_cursor(
    typed_input: &[char],
    cursor_idx: usize,
    max_line_width: usize,
) -> (Vec<String>, usize, usize) {
    let safe_width = max_line_width.max(1);
    let clamped_cursor = cursor_idx.min(typed_input.len());

    let mut lines: Vec<String> = Vec::new();
    let mut cur_line = String::new();
    let mut cur_width: usize = 0;
    let mut cursor_line: usize = 0;
    let mut cursor_col: usize = 0;

    for (idx, &raw_c) in typed_input.iter().enumerate() {
        (idx == clamped_cursor).then(|| {
            cursor_line = lines.len();
            cursor_col = cur_width;
        });

        match raw_c == '\n' {
            true => {
                lines.push(std::mem::take(&mut cur_line));
                cur_width = 0;
            }
            false => {
                let c = match raw_c.is_control() {
                    true => ' ',
                    false => raw_c,
                };
                let cw = crate::agent::style::markdown::char_display_width(c)
                    .max(1)
                    .min(safe_width);
                let should_wrap = cur_width + cw > safe_width && !cur_line.is_empty();
                should_wrap.then(|| {
                    lines.push(std::mem::take(&mut cur_line));
                    cur_width = 0;
                    (idx == clamped_cursor).then(|| {
                        cursor_line = lines.len();
                        cursor_col = 0;
                    });
                });
                cur_line.push(c);
                cur_width += cw;
            }
        }
    }

    (clamped_cursor == typed_input.len()).then(|| {
        cursor_line = lines.len();
        cursor_col = cur_width.min(safe_width);
    });

    lines.push(cur_line);
    (lines, cursor_line, cursor_col)
}

pub(crate) fn compute_input_dock_height(app: &RatatuiApp, dock_width: u16) -> u16 {
    match app.typed_input.is_empty() {
        true => 3,
        false => {
            let max_line_width = dock_width.saturating_sub(5).max(1) as usize;
            let (lines, _, _) =
                wrap_input_chars_with_cursor(&app.typed_input, app.cursor_idx, max_line_width);
            let inner_rows = lines.len().clamp(1, MAX_INPUT_DOCK_INNER_LINES);
            (inner_rows as u16) + 2
        }
    }
}

pub(crate) fn render_input_dock_with_placeholder(
    f: &mut Frame,
    app: &RatatuiApp,
    area: Rect,
    placeholder: &str,
) {
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.bg_input));

    let inner = block.inner(area);
    f.render_widget(block, area);

    match inner.width == 0 || inner.height == 0 {
        true => return,
        false => {}
    }

    let max_width = inner.width.saturating_sub(3).max(1) as usize;
    let visible_rows = (inner.height as usize).max(1);

    let (rendered_lines, cursor_row, cursor_col) = match app.typed_input.is_empty() {
        true => {
            let placeholder_display: String = placeholder.chars().take(max_width).collect();
            (
                vec![Line::from(vec![
                    Span::styled(
                        "› ",
                        Style::default()
                            .fg(theme.brand_accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(placeholder_display, Style::default().fg(theme.muted)),
                ])],
                0usize,
                2usize,
            )
        }
        false => {
            let (wrapped_lines, c_line, c_col) =
                wrap_input_chars_with_cursor(&app.typed_input, app.cursor_idx, max_width);
            let scroll_top = super::modals::compute_scroll_offset(c_line, visible_rows);
            let lines: Vec<Line<'static>> = wrapped_lines
                .into_iter()
                .enumerate()
                .skip(scroll_top)
                .take(visible_rows)
                .map(|(line_idx, text)| {
                    let prefix_span = match line_idx == 0 {
                        true => Span::styled(
                            "› ",
                            Style::default()
                                .fg(theme.brand_accent)
                                .add_modifier(Modifier::BOLD),
                        ),
                        false => Span::styled("· ", Style::default().fg(theme.muted)),
                    };
                    Line::from(vec![
                        prefix_span,
                        Span::styled(text, Style::default().fg(theme.brand_white)),
                    ])
                })
                .collect();
            let visible_cursor_row = c_line
                .saturating_sub(scroll_top)
                .min(visible_rows.saturating_sub(1));
            (lines, visible_cursor_row, 2 + c_col)
        }
    };

    f.render_widget(Paragraph::new(rendered_lines), inner);
    let cursor_x = inner
        .x
        .saturating_add(cursor_col as u16)
        .min(inner.x + inner.width.saturating_sub(1));
    let cursor_y = inner
        .y
        .saturating_add(cursor_row as u16)
        .min(inner.y + inner.height.saturating_sub(1));
    f.set_cursor_position((cursor_x, cursor_y));
}

// ── Floating Spotlight Slash Command Palette ─────────────────────────────────

fn render_slash_palette(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    match app.has_active_slash_query() {
        false => return,
        true => {}
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
        let span = match idx == current_cat_idx {
            true => Span::styled(
                format!("◉ {} ", cat.label()),
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            false => Span::styled(
                format!("○ {} ", cat.label()),
                Style::default().fg(theme.muted),
            ),
        };
        title_spans.push(span);
    }
    title_spans.push(Span::raw(" "));

    let outer_block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
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
    let max_query_chars = (chunks[0].width as usize).saturating_sub(6).max(1);
    let typed_text: String = app.typed_input.iter().take(max_query_chars).collect();
    let query_display = match typed_text.is_empty() {
        true => "/".to_string(),
        false => typed_text,
    };
    let search_line = Line::from(vec![
        Span::styled(
            "  › ",
            Style::default()
                .fg(theme.brand_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            query_display,
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

    match matches.is_empty() {
        true => {
            item_lines.push(Line::from(vec![Span::styled(
                "   No matching commands found",
                Style::default().fg(theme.muted),
            )]));
        }
        false => {
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

                let prefix = match is_selected {
                    true => " ❯ ",
                    false => "   ",
                };
                let left_content = format!("{}{:<13} {}", prefix, cmd.slash_name, cmd.title);
                let left_width = left_content.chars().count();
                let shortcut_width = shortcut_str.chars().count();

                let avail_space = inner_width.saturating_sub(left_width + shortcut_width + 2);
                let padding = " ".repeat(avail_space);

                match is_selected {
                    true => {
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
                    }
                    false => {
                        let mut spans = vec![
                            Span::styled(left_content, Style::default().fg(theme.text_primary)),
                            Span::raw(padding),
                        ];
                        match shortcut_str.is_empty() {
                            false => spans.push(Span::styled(
                                shortcut_str,
                                Style::default().fg(theme.warning),
                            )),
                            true => {}
                        }
                        spans.push(Span::raw(" "));
                        item_lines.push(Line::from(spans));
                    }
                }
            }
        }
    }

    f.render_widget(Paragraph::new(item_lines), chunks[2]);
}

// ── Status Bar (Bottom line) ────────────────────────────────────────────────

fn render_status_bar(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let theme = &app.theme;
    let (mcp_loaded, mcp_failed, _mcp_total) = crate::tools::mcp::get_mcp_stats();
    let mcp_done = crate::tools::mcp::is_mcp_done();

    let provider_model = match app.model.starts_with(&app.provider) {
        true => app.model.clone(),
        false => format!("{}:{}", app.provider, app.model),
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

    push_git_branch_spans(&mut left_spans, &app.workspace_root, &theme);

    // MCP status indicator (clean & minimal)
    left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
    let mcp_span = match (mcp_done, mcp_failed, mcp_loaded) {
        (false, _, _) => {
            let frame_idx = app.spinner_idx % theme::SPINNER_FRAMES.len();
            Span::styled(
                format!("mcp {}", theme::SPINNER_FRAMES[frame_idx]),
                Style::default().fg(theme.warning),
            )
        }
        (true, 0, 0) => Span::styled("mcp 0", Style::default().fg(theme.muted)),
        (true, 0, loaded) => Span::styled(
            format!("mcp {}", loaded),
            Style::default().fg(theme.brand_accent),
        ),
        (true, failed, loaded) => Span::styled(
            format!("mcp {} ({}✗)", loaded, failed),
            Style::default().fg(theme.destructive),
        ),
    };
    left_spans.push(mcp_span);

    // Queued prompts indicator
    match app.queued_prompts.is_empty() {
        false => {
            left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
            left_spans.push(Span::styled(
                format!("[{} queued]", app.queued_prompts.len()),
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        true => {}
    }

    // Background servers indicator
    let bg_servers = crate::shutdown::list_registered_children();
    match bg_servers.is_empty() {
        false => {
            left_spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
            left_spans.push(Span::styled(
                format!("⚙ {} srv", bg_servers.len()),
                Style::default().fg(theme.info),
            ));
        }
        true => {}
    }

    // Token context & Dynamic Context Window (Right Side)
    let limit_tokens = crate::providers::DynamicContextRegistry::resolve_context_window(
        &app.model,
        &crate::config::schema::Config::default(),
    );
    let limit_str = match limit_tokens >= 1_000_000 {
        true => format!("{}M", limit_tokens / 1_000_000),
        false => format!("{}K", limit_tokens / 1000),
    };
    let approx_tokens_str = match app.approx_tokens >= 1000 {
        true => format!("{:.1}K", app.approx_tokens as f64 / 1000.0),
        false => format!("{}", app.approx_tokens),
    };

    let ratio = match limit_tokens > 0 {
        true => (app.approx_tokens as f64) / (limit_tokens as f64),
        false => 0.0,
    };
    let token_color = match (ratio > 0.85, ratio > 0.60) {
        (true, _) => theme.destructive,
        (false, true) => theme.warning,
        (false, false) => theme.success,
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
        .constraints([Constraint::Min(20), Constraint::Length(25)])
        .split(area);

    let left_p =
        Paragraph::new(Line::from(left_spans)).block(Block::default().borders(Borders::NONE));
    f.render_widget(left_p, status_chunks[0]);

    let right_p = Paragraph::new(Line::from(right_spans))
        .block(Block::default().borders(Borders::NONE))
        .alignment(Alignment::Right);
    f.render_widget(right_p, status_chunks[1]);
}

pub(crate) fn push_git_branch_spans<'a>(
    spans: &mut Vec<Span<'a>>,
    workspace_root: &std::path::Path,
    theme: &theme::Theme,
) {
    match RatatuiApp::get_git_branch(workspace_root) {
        Some(branch) => {
            spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
            spans.push(Span::styled(
                format!("git:{}", branch),
                Style::default().fg(theme.success),
            ));
        }
        None => {}
    }
}
