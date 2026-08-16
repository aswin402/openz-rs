use super::app::{ModalState, RatatuiApp};
use super::theme::{self, Theme};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::sync::OnceLock;

// ── Regex helpers for inline markdown ────────────────────────────────────────

fn re_bold() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\*\*(.*?)\*\*").ok())
        .as_ref()
}

fn re_code() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"`([^`]+)`").ok())
        .as_ref()
}

/// Convert a single line of markdown text into styled ratatui Spans.
fn markdown_line_to_spans(line: &str, theme: &Theme) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();

    // Heading detection
    if let Some(rest) = trimmed.strip_prefix("### ") {
        return vec![
            Span::styled(
                "### ",
                Style::default()
                    .fg(theme.highlight)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
    }
    if let Some(rest) = trimmed.strip_prefix("## ") {
        return vec![
            Span::styled(
                "## ",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
    }
    if let Some(rest) = trimmed.strip_prefix("# ") {
        return vec![
            Span::styled(
                "# ",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
    }

    // Horizontal rule
    if trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3 {
        return vec![Span::styled(
            "────────────────────────────────────────────────────────────".to_string(),
            Style::default().fg(theme.border),
        )];
    }

    // List items — use classic crisp bullet (•)
    if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| trimmed.strip_prefix("• ")).or_else(|| trimmed.strip_prefix("· ")) {
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        let mut spans = vec![
            Span::raw(indent),
            Span::styled("• ", Style::default().fg(theme.brand_accent)),
        ];
        spans.extend(parse_inline_markdown(rest, theme));
        return spans;
    }

    if let Some(rest) = trimmed.strip_prefix("  - ").or_else(|| trimmed.strip_prefix("  * ")).or_else(|| trimmed.strip_prefix("  • ")).or_else(|| trimmed.strip_prefix("  · ")) {
        let mut spans = vec![
            Span::styled("    • ", Style::default().fg(theme.muted)),
        ];
        spans.extend(parse_inline_markdown(rest, theme));
        return spans;
    }

    // Numbered list items (e.g., "1. item")
    if let Some(pos) = trimmed.find(". ") {
        if pos > 0 && pos <= 3 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            let number = &trimmed[..pos];
            let rest = &trimmed[pos + 2..];
            let mut spans = vec![
                Span::raw(indent),
                Span::styled(format!("{}. ", number), Style::default().fg(theme.brand_accent)),
            ];
            spans.extend(parse_inline_markdown(rest, theme));
            return spans;
        }
    }

    // Regular text with inline formatting
    parse_inline_markdown(line, theme)
}

/// Parse inline markdown (**bold** and `code`) into styled spans.
fn parse_inline_markdown(text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = text.to_string();

    loop {
        if remaining.is_empty() {
            break;
        }

        let bold_match =
            re_bold().and_then(|re| re.find(&remaining).map(|m| (m.start(), m.end(), "bold")));
        let code_match =
            re_code().and_then(|re| re.find(&remaining).map(|m| (m.start(), m.end(), "code")));

        let earliest = match (bold_match, code_match) {
            (Some(b), Some(c)) => {
                if b.0 <= c.0 {
                    Some(b)
                } else {
                    Some(c)
                }
            }
            (Some(b), None) => Some(b),
            (None, Some(c)) => Some(c),
            (None, None) => None,
        };

        match earliest {
            Some((start, end, kind)) => {
                if start > 0 {
                    spans.push(Span::styled(
                        remaining[..start].to_string(),
                        Style::default().fg(theme.text_primary),
                    ));
                }

                let matched = &remaining[start..end];
                match kind {
                    "bold" => {
                        let inner = &matched[2..matched.len() - 2];
                        spans.push(Span::styled(
                            inner.to_string(),
                            Style::default()
                                .fg(theme.brand_white)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    "code" => {
                        let inner = &matched[1..matched.len() - 1];
                        spans.push(Span::styled(
                            format!(" {} ", inner),
                            Style::default()
                                .fg(theme.success)
                                .bg(theme.bg_elevated),
                        ));
                    }
                    _ => {}
                }

                remaining = remaining[end..].to_string();
            }
            None => {
                spans.push(Span::styled(
                    remaining.clone(),
                    Style::default().fg(theme.text_primary),
                ));
                break;
            }
        }
    }

    if spans.is_empty() {
        spans.push(Span::styled(
            text.to_string(),
            Style::default().fg(theme.text_primary),
        ));
    }

    spans
}

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

// ── Conversation Timeline ───────────────────────────────────────────────────

fn render_timeline(f: &mut Frame, app: &mut RatatuiApp, area: Rect) {
    let theme = &app.theme;
    let mut lines = Vec::new();

    // ── OpenZ CLI Authentic ASCII Logo Banner ───────────────────────────────
    let logo_parts = [
        ("     ██████╗ ██████╗ ███████╗███╗   ██╗", "███████╗"),
        ("    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║", "╚══███╔╝"),
        ("    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║", "  ███╔╝ "),
        ("    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║", " ███╔╝  "),
        ("    ╚██████╔╝██║     ███████╗██║ ╚████║", "███████╗"),
        ("     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝", "╚══════╝"),
    ];

    lines.push(Line::from(String::new()));
    for (white_part, orange_part) in &logo_parts {
        lines.push(Line::from(vec![
            Span::styled(
                white_part.to_string(),
                Style::default()
                    .fg(theme.brand_white)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                orange_part.to_string(),
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    lines.push(Line::from(String::new()));
    lines.push(Line::from(vec![
        Span::styled(
            format!(" openz v{}", env!("CARGO_PKG_VERSION")),
            Style::default()
                .fg(theme.brand_accent)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            format!("{} | {}", app.provider, app.model),
            Style::default().fg(theme.warning),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(app.cwd_display.clone(), Style::default().fg(theme.info)),
    ]));

    if let Some(branch) = RatatuiApp::get_git_branch(&app.workspace_root) {
        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                format!("git: {}", branch),
                Style::default().fg(theme.success),
            ),
        ]));
    }

    lines.push(Line::from(vec![Span::styled(
        "────────────────────────────────────────────────────────────",
        Style::default().fg(theme.border),
    )]));
    lines.push(Line::from(String::new()));

    // ── Message History ─────────────────────────────────────────────────────
    let mut prev_was_user = false;

    for msg in &app.messages {
        match msg.role.as_str() {
            "user" => {
                if prev_was_user || lines.len() > 14 {
                    lines.push(Line::from(String::new()));
                    lines.push(Line::from(vec![Span::styled(
                        "─".repeat(area.width.saturating_sub(4) as usize),
                        Style::default().fg(theme.border),
                    )]));
                    lines.push(Line::from(String::new()));
                }

                let content_lines: Vec<&str> = msg.content.lines().collect();
                for (i, line) in content_lines.iter().enumerate() {
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::styled(
                                "› ",
                                Style::default()
                                    .fg(theme.brand_accent)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                line.to_string(),
                                Style::default()
                                    .fg(theme.brand_white)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled("  ", Style::default()),
                            Span::styled(
                                line.to_string(),
                                Style::default()
                                    .fg(theme.brand_white)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }
                }
                prev_was_user = true;
            }
            "assistant" => {
                // ── Thought badge + Monologue (CLI TUI style with bullet dot) ─
                if let Some(thinking_time) = msg.thinking_time {
                    lines.push(Line::from(vec![
                        Span::styled("• ", Style::default().fg(theme.brand_accent)),
                        Span::styled(
                            format!("Thought for {:.1}s", thinking_time),
                            Style::default()
                                .fg(theme.brand_accent)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                } else if msg.reasoning.is_some() {
                    lines.push(Line::from(vec![
                        Span::styled("• ", Style::default().fg(theme.brand_accent)),
                        Span::styled(
                            "Thoughts",
                            Style::default()
                                .fg(theme.brand_accent)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }

                if let Some(ref reasoning) = msg.reasoning {
                    let reasoning_trimmed = reasoning.trim();
                    if !reasoning_trimmed.is_empty() {
                        for r_line in reasoning_trimmed.lines() {
                            lines.push(Line::from(vec![
                                Span::styled("  L ", Style::default().fg(theme.muted)),
                                Span::styled(
                                    r_line.to_string(),
                                    Style::default().fg(theme.muted),
                                ),
                            ]));
                        }
                        lines.push(Line::from(String::new()));
                    }
                }

                // Assistant Markdown content
                if !msg.content.is_empty() {
                    let content = msg.content.trim();
                    let mut in_code_block = false;

                    for line in content.lines() {
                        let trimmed = line.trim_start();

                        // Code fence toggle
                        if trimmed.starts_with("```") {
                            in_code_block = !in_code_block;
                            if in_code_block {
                                let lang = trimmed.strip_prefix("```").unwrap_or("").trim();
                                if !lang.is_empty() {
                                    lines.push(Line::from(vec![Span::styled(
                                        format!("  ┌─ {} ", lang),
                                        Style::default().fg(theme.success),
                                    )]));
                                } else {
                                    lines.push(Line::from(vec![Span::styled(
                                        "  ┌──",
                                        Style::default().fg(theme.success),
                                    )]));
                                }
                            } else {
                                lines.push(Line::from(vec![Span::styled(
                                    "  └──",
                                    Style::default().fg(theme.success),
                                )]));
                            }
                            continue;
                        }

                        if in_code_block {
                            lines.push(Line::from(vec![
                                Span::styled("  │ ", Style::default().fg(theme.success)),
                                Span::styled(
                                    line.to_string(),
                                    Style::default().fg(theme.text_primary),
                                ),
                            ]));
                        } else {
                            let mut spans = vec![Span::raw("  ".to_string())];
                            spans.extend(markdown_line_to_spans(line, theme));
                            lines.push(Line::from(spans));
                        }
                    }
                    lines.push(Line::from(String::new()));
                }

                prev_was_user = false;
            }
            "tool" => {
                let tool_name = msg.tool_name.as_deref().unwrap_or("tool");
                let details = msg.tool_details.as_deref().unwrap_or("");

                let (verb, verb_style) = match msg.tool_success {
                    Some(true) => ("Ran", Style::default().fg(theme.muted)),
                    Some(false) => ("Failed", Style::default().fg(theme.destructive)),
                    None => ("Running", Style::default().fg(theme.brand_accent)),
                };

                // Bullet dot • prefix for tools
                lines.push(Line::from(vec![
                    Span::styled("• ", Style::default().fg(theme.brand_accent)),
                    Span::styled(format!("{} ", verb), verb_style),
                    Span::styled(
                        if details.is_empty() {
                            tool_name.to_string()
                        } else {
                            format!("{}: {}", tool_name, details)
                        },
                        Style::default().fg(theme.warning),
                    ),
                ]));

                // Tool output formatting with diff colorization
                if !msg.content.is_empty() {
                    let trimmed = msg.content.trim();
                    let max_lines = 8;
                    let mut count = 0;
                    for out_line in trimmed.lines().take(max_lines) {
                        count += 1;
                        let prefix = if count == 1 { "  └ " } else { "    " };

                        let line_color = if out_line.starts_with('+') {
                            theme.success
                        } else if out_line.starts_with('-') {
                            theme.destructive
                        } else if out_line.starts_with("##") || out_line.starts_with("test result:") {
                            theme.info
                        } else {
                            theme.muted
                        };

                        lines.push(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(theme.muted)),
                            Span::styled(out_line.to_string(), Style::default().fg(line_color)),
                        ]));
                    }

                    let total_out_lines = trimmed.lines().count();
                    if total_out_lines > max_lines {
                        lines.push(Line::from(vec![Span::styled(
                            format!("    ... +{} lines (output folded)", total_out_lines - max_lines),
                            Style::default().fg(theme.border),
                        )]));
                    }
                    lines.push(Line::from(String::new()));
                }

                prev_was_user = false;
            }
            _ => {
                if !msg.content.is_empty() {
                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("• ", Style::default().fg(theme.brand_accent)),
                            Span::styled(line.to_string(), Style::default().fg(theme.text_primary)),
                        ]));
                    }
                }
                prev_was_user = false;
            }
        }
    }

    // ── Active Thinking Animation Indicator ──────────────────────────────────
    if app.is_thinking {
        let frame_idx = app.spinner_idx % theme::SPINNER_FRAMES.len();
        let spinner = theme::SPINNER_FRAMES[frame_idx];
        let elapsed = app.work_start.map(|s| s.elapsed().as_secs()).unwrap_or(0);

        // Animated dots for thinking pulse
        let dots = match (app.spinner_idx / 3) % 4 {
            0 => ".  ",
            1 => ".. ",
            2 => "...",
            _ => "   ",
        };

        lines.push(Line::from(vec![
            Span::styled("• ", Style::default().fg(theme.brand_accent)),
            Span::styled(
                format!("{} ", spinner),
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Thinking{} ", dots),
                Style::default()
                    .fg(theme.brand_white)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({}s • esc or ctrl+c to interrupt)", elapsed),
                Style::default().fg(theme.muted),
            ),
        ]));
    }

    let total_lines = lines.len() as u32;
    let viewport_height = area.height as u32;
    let max_scroll = total_lines.saturating_sub(viewport_height);
    app.max_scroll = max_scroll;

    let scroll = if app.auto_scroll {
        max_scroll
    } else {
        app.scroll_offset.min(max_scroll)
    };

    // Paragraph can only scroll within u16 range; clamp oversized timelines
    let scroll_u16 = scroll.min(u16::MAX as u32) as u16;

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false })
        .scroll((scroll_u16, 0));

    f.render_widget(paragraph, area);
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

// ── Modal Overlays (minicode style centered dialogs) ────────────────────────

fn render_modal_overlay(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let theme = &app.theme;

    match &app.modal {
        ModalState::None => {}
        ModalState::ProviderSelect {
            providers,
            selected_idx,
        } => {
            let popup_area = centered_rect(55, 50, area);
            f.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Select LLM Provider ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.brand_accent))
                .style(Style::default().bg(theme.bg_elevated));

            let items: Vec<ListItem> = providers
                .iter()
                .enumerate()
                .map(|(i, (_name, display))| {
                    let is_selected = i == *selected_idx;
                    let prefix = if is_selected { " › " } else { "   " };
                    let style = if is_selected {
                        Style::default()
                            .fg(theme.bg_primary)
                            .bg(theme.brand_accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_primary)
                    };
                    ListItem::new(format!("{}{}", prefix, display)).style(style)
                })
                .collect();

            let list = List::new(items).block(block);
            f.render_widget(list, popup_area);
        }
        ModalState::ModelSelect {
            provider_display,
            models,
            filtered_indices,
            selected_idx,
            filter,
            loading,
            ..
        } => {
            let popup_area = centered_rect(75, 70, area);
            f.render_widget(Clear, popup_area);

            let outer_block = Block::default()
                .title(format!(" Select Model ({}) ", provider_display))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.brand_accent))
                .style(Style::default().bg(theme.bg_elevated));

            let inner_area = outer_block.inner(popup_area);
            f.render_widget(outer_block, popup_area);

            if *loading {
                let loading_p = Paragraph::new(format!(
                    "Fetching live models from {}...",
                    provider_display
                ))
                .style(Style::default().fg(theme.warning))
                .alignment(Alignment::Center);
                f.render_widget(loading_p, inner_area);
                return;
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Search box
                    Constraint::Min(5),    // Model list
                    Constraint::Length(1), // Help hints
                ])
                .split(inner_area);

            // Search filter box
            let search_text = format!(" Search: {}█", filter);
            let search_box = Paragraph::new(search_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border))
                        .title(" Filter Models "),
                )
                .style(Style::default().fg(theme.text_primary));
            f.render_widget(search_box, chunks[0]);

            // Model list items
            let items: Vec<ListItem> = filtered_indices
                .iter()
                .enumerate()
                .map(|(visual_idx, &real_idx)| {
                    let m = &models[real_idx];
                    let is_selected = visual_idx == *selected_idx;
                    let prefix = if is_selected { " › " } else { "   " };

                    let style = if is_selected {
                        Style::default()
                            .fg(theme.bg_primary)
                            .bg(theme.brand_accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_primary)
                    };
                    ListItem::new(format!("{}{}", prefix, m)).style(style)
                })
                .collect();

            let list = List::new(items).block(Block::default().borders(Borders::NONE));
            f.render_widget(list, chunks[1]);

            let hints = Paragraph::new(" ↑/↓ Navigate · enter Select · type to Filter · esc Cancel")
                .style(Style::default().fg(theme.muted))
                .alignment(Alignment::Center);
            f.render_widget(hints, chunks[2]);
        }
        ModalState::Help => {
            let popup_area = centered_rect(65, 60, area);
            f.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" OpenZ Help & Commands ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.brand_accent))
                .style(Style::default().bg(theme.bg_elevated));

            let inner = block.inner(popup_area);
            f.render_widget(block, popup_area);

            let help_text = vec![
                Line::from(vec![
                    Span::styled("Slash Commands:", Style::default().fg(theme.brand_accent).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("  /model       ", Style::default().fg(theme.warning)),
                    Span::styled("Switch active LLM provider and model", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  /clear       ", Style::default().fg(theme.warning)),
                    Span::styled("Clear current conversation timeline", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  /history     ", Style::default().fg(theme.warning)),
                    Span::styled("Restore or switch previous sessions", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  /mcps        ", Style::default().fg(theme.warning)),
                    Span::styled("List configured and active MCP tools", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  /memory      ", Style::default().fg(theme.warning)),
                    Span::styled("View cognitive knowledge graph & facts", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  /skills      ", Style::default().fg(theme.warning)),
                    Span::styled("View active autonomous skills", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  /exit        ", Style::default().fg(theme.warning)),
                    Span::styled("Quit OpenZ interactive session", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(String::new()),
                Line::from(vec![
                    Span::styled("Shortcuts:", Style::default().fg(theme.brand_accent).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("  Enter        ", Style::default().fg(theme.info)),
                    Span::styled("Send message / Select highlighted item", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  Tab          ", Style::default().fg(theme.info)),
                    Span::styled("Autocomplete slash command", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  ↑ / ↓        ", Style::default().fg(theme.info)),
                    Span::styled("Navigate commands / prompt history", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  PgUp / PgDn  ", Style::default().fg(theme.info)),
                    Span::styled("Scroll conversation timeline", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  Mouse Wheel  ", Style::default().fg(theme.info)),
                    Span::styled("Scroll conversation up / down smoothly", Style::default().fg(theme.text_primary)),
                ]),
                Line::from(vec![
                    Span::styled("  Esc / Ctrl+C ", Style::default().fg(theme.destructive)),
                    Span::styled("Cancel active agent turn / Dismiss popup", Style::default().fg(theme.text_primary)),
                ]),
            ];

            let p = Paragraph::new(help_text);
            f.render_widget(p, inner);
        }
        ModalState::History {
            sessions,
            selected_idx,
        } => {
            let popup_area = centered_rect(65, 60, area);
            f.render_widget(Clear, popup_area);

            let block = Block::default()
                .title(" Restore Chat Session ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.brand_accent))
                .style(Style::default().bg(theme.bg_elevated));

            let items: Vec<ListItem> = sessions
                .iter()
                .enumerate()
                .map(|(i, (key, title, time))| {
                    let is_selected = i == *selected_idx;
                    let prefix = if is_selected { " › " } else { "   " };
                    let style = if is_selected {
                        Style::default()
                            .fg(theme.bg_primary)
                            .bg(theme.brand_accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_primary)
                    };
                    ListItem::new(format!("{}{:<25} {:<20} ({})", prefix, title, key, time))
                        .style(style)
                })
                .collect();

            let list = List::new(items).block(block);
            f.render_widget(list, popup_area);
        }
    }
}

/// Helper function to create a centered Rect for modals
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
