use super::app::RatatuiApp;
use super::markdown::markdown_line_to_spans;
use super::theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub(crate) fn render_timeline(f: &mut Frame, app: &mut RatatuiApp, area: Rect) {
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
    lines.push(Line::from(vec![Span::styled(
        format!(" openz v{}", env!("CARGO_PKG_VERSION")),
        Style::default()
            .fg(theme.brand_accent)
            .add_modifier(Modifier::BOLD),
    )]));

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
                                Span::styled(r_line.to_string(), Style::default().fg(theme.muted)),
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
                        } else if out_line.starts_with("##") || out_line.starts_with("test result:")
                        {
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
                            format!(
                                "    ... +{} lines (output folded)",
                                total_out_lines - max_lines
                            ),
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
