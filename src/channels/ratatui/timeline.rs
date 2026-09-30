use super::app::RatatuiApp;
use super::markdown::markdown_line_to_spans;
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
                        let max_t_width = (area.width.saturating_sub(6) as usize).max(20);
                        for r_line in reasoning_trimmed.lines() {
                            let chunks = split_line_into_chunks(r_line, max_t_width);
                            for chunk in chunks {
                                lines.push(Line::from(vec![
                                    Span::styled("  │ ", Style::default().fg(theme.border)),
                                    Span::styled(
                                        chunk,
                                        Style::default()
                                            .fg(theme.muted)
                                            .add_modifier(Modifier::ITALIC),
                                    ),
                                ]));
                            }
                        }
                        lines.push(Line::from(String::new()));
                    }
                }

                // Assistant Markdown content
                if !msg.content.is_empty() {
                    let content = msg.content.trim();
                    let mut in_code_block = false;
                    let table_width = area.width.saturating_sub(4) as usize;

                    let content_lines: Vec<&str> = content.lines().collect();
                    let mut i = 0;
                    while i < content_lines.len() {
                        let line = content_lines[i];
                        let trimmed = line.trim_start();

                        // Code fence toggle
                        if trimmed.starts_with("```") {
                            in_code_block = !in_code_block;
                            if in_code_block {
                                let lang = trimmed.strip_prefix("```").unwrap_or("").trim();
                                if !lang.is_empty() {
                                    lines.push(Line::from(vec![
                                        Span::styled("  ┌─ ", Style::default().fg(theme.success)),
                                        Span::styled(
                                            lang.to_string(),
                                            Style::default()
                                                .fg(theme.brand_accent)
                                                .add_modifier(Modifier::BOLD),
                                        ),
                                        Span::styled(" ─────────────────────────────────", Style::default().fg(theme.border)),
                                    ]));
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
                            i += 1;
                            continue;
                        }

                        if in_code_block {
                            lines.push(Line::from(vec![
                                Span::styled("  │ ", Style::default().fg(theme.border)),
                                Span::styled(
                                    line.to_string(),
                                    Style::default().fg(theme.text_primary),
                                ),
                            ]));
                            i += 1;
                            continue;
                        }

                        // Table detection: check if current line is a table header and next line is a divider
                        if crate::channels::ratatui::markdown::is_table_row(line)
                            && i + 1 < content_lines.len()
                            && crate::channels::ratatui::markdown::is_divider_row(content_lines[i + 1])
                        {
                            let mut table_chunk = Vec::new();
                            table_chunk.push(line);
                            table_chunk.push(content_lines[i + 1]);
                            i += 2;
                            while i < content_lines.len()
                                && crate::channels::ratatui::markdown::is_table_row(content_lines[i])
                            {
                                table_chunk.push(content_lines[i]);
                                i += 1;
                            }
                            let table_rendered = crate::channels::ratatui::markdown::render_markdown_table_to_lines(
                                &table_chunk,
                                theme,
                                table_width,
                            );
                            lines.extend(table_rendered);
                            continue;
                        }

                        let mut spans = vec![Span::raw("  ".to_string())];
                        spans.extend(markdown_line_to_spans(line, theme));
                        lines.push(Line::from(spans));
                        i += 1;
                    }
                    lines.push(Line::from(String::new()));
                }

                prev_was_user = false;
            }
            "tool" => {
                let tool_name = msg.tool_name.as_deref().unwrap_or("Tool");
                let details = msg.tool_details.as_deref().unwrap_or("");

                // Tool start bullet line: • ToolName details
                let mut header_spans = vec![
                    Span::styled("• ", Style::default().fg(theme.brand_accent)),
                    Span::styled(
                        tool_name.to_string(),
                        Style::default()
                            .fg(theme.brand_white)
                            .add_modifier(Modifier::BOLD),
                    ),
                ];
                if !details.is_empty() {
                    header_spans.push(Span::raw(" "));
                    header_spans.push(Span::styled(
                        details.to_string(),
                        Style::default().fg(theme.muted),
                    ));
                }
                lines.push(Line::from(header_spans));

                // Outcome summary line:   L ✓ summary (0.3s)
                if let Some(ref summary) = msg.tool_summary {
                    let has_success = summary.contains('✓') || summary.contains('✔');
                    let has_fail = summary.contains('✗')
                        || summary.contains('✖')
                        || summary.contains("Failed")
                        || summary.contains("error");

                    let summary_color = if has_success || msg.tool_success == Some(true) {
                        theme.success
                    } else if has_fail || msg.tool_success == Some(false) {
                        theme.destructive
                    } else {
                        theme.info
                    };

                    let mut outcome_spans = vec![
                        Span::styled("  L ", Style::default().fg(theme.muted)),
                    ];

                    if has_success || has_fail {
                        outcome_spans.push(Span::styled(
                            summary.clone(),
                            Style::default().fg(summary_color),
                        ));
                    } else if msg.tool_success == Some(true) {
                        outcome_spans.push(Span::styled(
                            format!("✓ {}", summary),
                            Style::default().fg(summary_color),
                        ));
                    } else if msg.tool_success == Some(false) {
                        outcome_spans.push(Span::styled(
                            format!("✗ {}", summary),
                            Style::default().fg(summary_color),
                        ));
                    } else {
                        outcome_spans.push(Span::styled(
                            summary.clone(),
                            Style::default().fg(summary_color),
                        ));
                    }

                    if let Some(duration_ms) = msg.tool_duration_ms {
                        outcome_spans.push(Span::styled(
                            format!(" ({:.1}s)", duration_ms as f64 / 1000.0),
                            Style::default().fg(theme.muted),
                        ));
                    }

                    lines.push(Line::from(outcome_spans));
                } else {
                    let (icon, verb_style) = match msg.tool_success {
                        Some(true) => ("✓ completed", Style::default().fg(theme.success)),
                        Some(false) => ("✗ failed", Style::default().fg(theme.destructive)),
                        None => ("running...", Style::default().fg(theme.brand_accent)),
                    };
                    lines.push(Line::from(vec![
                        Span::styled("  L ", Style::default().fg(theme.muted)),
                        Span::styled(icon, verb_style),
                    ]));
                }

                // Tool output preview (diffs / test results)
                if !msg.content.is_empty() {
                    let trimmed = msg.content.trim();
                    let is_diff_or_output = trimmed.lines().any(|l| {
                        l.starts_with('+')
                            || l.starts_with('-')
                            || l.starts_with("error[")
                            || l.starts_with("test ")
                    });
                    if is_diff_or_output {
                        let max_lines = 8;
                        for out_line in trimmed.lines().take(max_lines) {
                            let line_color = if out_line.starts_with('+') && !out_line.starts_with("+++") {
                                theme.success
                            } else if out_line.starts_with('-') && !out_line.starts_with("---") {
                                theme.destructive
                            } else if out_line.starts_with("@@") || out_line.starts_with("test result:") {
                                theme.info
                            } else {
                                theme.muted
                            };

                            lines.push(Line::from(vec![
                                Span::styled("    ", Style::default()),
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
                    }
                }

                lines.push(Line::from(String::new()));
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


    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false });

    let total_rendered_lines = paragraph.line_count(area.width) as u32;
    let viewport_height = area.height as u32;
    let max_scroll = total_rendered_lines.saturating_sub(viewport_height);
    app.max_scroll = max_scroll;

    let scroll = if app.auto_scroll {
        max_scroll
    } else {
        app.scroll_offset.min(max_scroll)
    };

    // Paragraph can only scroll within u16 range; clamp oversized timelines
    let scroll_u16 = scroll.min(u16::MAX as u32) as u16;

    let paragraph = paragraph.scroll((scroll_u16, 0));
    f.render_widget(paragraph, area);
}

fn split_line_into_chunks(line: &str, max_width: usize) -> Vec<String> {
    if line.len() <= max_width {
        return vec![line.to_string()];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            chunks.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    if chunks.is_empty() {
        chunks.push(line.to_string());
    }
    chunks
}
