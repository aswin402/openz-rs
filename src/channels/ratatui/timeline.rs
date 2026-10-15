use super::app::RatatuiApp;
use super::markdown::markdown_line_to_spans;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

const FAILURE_INDICATORS: &[&str] = &["✗", "✖", "✕", "Failed", "failed", "error", "Error"];
const FAILED_PREFIXES: &[&str] = &["Failed:", "failed:", "Failed", "failed"];

fn is_outcome_trim_char(c: char) -> bool {
    matches!(c, '✓' | '✔' | '✗' | '✖' | '✕' | '•' | ' ')
}

/// Cleans raw tool outcome summary: strips ANSI codes, detects success/failure,
/// and strips duplicate ticks, crosses, and "Failed:" prefixes.
pub fn clean_tool_outcome_summary(raw: &str, default_success: Option<bool>) -> (bool, String) {
    let stripped = crate::agent::style::strip_ansi_escapes(raw);
    let trimmed = stripped.trim();

    let has_fail = FAILURE_INDICATORS.iter().any(|ind| trimmed.contains(ind));
    let has_tick = trimmed.contains('✓') || trimmed.contains('✔');

    let is_succ = match (has_fail, has_tick) {
        (true, _) => false,
        (false, true) => true,
        (false, false) => default_success.unwrap_or(true),
    };

    let clean_leading = trimmed.trim_start_matches(is_outcome_trim_char).trim_start();

    let after_prefix = FAILED_PREFIXES
        .iter()
        .find_map(|prefix| clean_leading.strip_prefix(prefix).map(str::trim_start))
        .unwrap_or(clean_leading);

    let clean = after_prefix
        .trim_end_matches(is_outcome_trim_char)
        .trim();

    let final_text = match (clean.is_empty(), is_succ) {
        (true, true) => "completed",
        (true, false) => "failed",
        (false, _) => clean,
    };

    (is_succ, final_text.to_string())
}

fn parse_mcp_tool_origin(tool_name: &str) -> (bool, Option<&str>, &str) {
    match tool_name.strip_prefix("mcp::") {
        Some(stripped) => match stripped.split_once("::") {
            Some((srv, t)) => (true, Some(srv), t),
            None => (true, None, stripped),
        },
        None => match tool_name.strip_prefix("mcp__") {
            Some(stripped) => match stripped.split_once("__") {
                Some((srv, t)) => (true, Some(srv), t),
                None => (true, None, stripped),
            },
            None => match tool_name.split_once("::") {
                Some((srv, t)) => (true, Some(srv), t),
                None => (false, None, tool_name),
            },
        },
    }
}

fn classify_running_tool_verb(tool_name: Option<&str>) -> &'static str {
    let name = match tool_name {
        Some(n) => n.to_ascii_lowercase(),
        None => return "running...",
    };
    const VERB_RULES: &[(&[&str], &str)] = &[
        (&["deep_research", "deep research"], "deep researching..."),
        (&["search", "web", "crawl", "fetch"], "researching..."),
        (&["write", "create"], "writing..."),
        (&["patch", "edit", "replace"], "editing..."),
        (&["media", "video", "image", "svg"], "rendering..."),
        (&["doc", "pdf", "xlsx", "docx"], "processing..."),
        (&["check", "test", "clippy", "lint"], "checking..."),
        (&["delegate", "subagent", "orchestrat"], "delegating..."),
    ];
    VERB_RULES
        .iter()
        .find_map(|(patterns, verb)| {
            match patterns.iter().any(|p| name.contains(p)) {
                true => Some(*verb),
                false => None,
            }
        })
        .unwrap_or("running...")
}

fn format_tool_duration(duration_ms: u64) -> String {
    match duration_ms < 1000 {
        true => format!(" ({}ms)", duration_ms),
        false => format!(" ({:.2}s)", duration_ms as f64 / 1000.0),
    }
}

fn non_empty_or<'a>(primary: &'a str, fallback: &'a str) -> &'a str {
    match primary.is_empty() {
        true => fallback,
        false => primary,
    }
}

pub(crate) fn render_timeline(f: &mut Frame, app: &mut RatatuiApp, area: Rect) {
    let theme = &app.theme;
    f.render_widget(Clear, area);
    let mut lines = Vec::new();

    // ── OpenZ CLI Authentic ASCII Logo Banner ───────────────────────────────
    lines.push(Line::from(String::new()));
    for (white_part, orange_part) in crate::agent::style::markdown::OPENZ_BANNER_PARTS {
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

    match RatatuiApp::get_git_branch(&app.workspace_root) {
        Some(branch) => {
            lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    format!("git: {}", branch),
                    Style::default().fg(theme.success),
                ),
            ]));
        }
        None => {}
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
                match prev_was_user || lines.len() > 14 {
                    true => {
                        lines.push(Line::from(String::new()));
                        lines.push(Line::from(vec![Span::styled(
                            "─".repeat(area.width.saturating_sub(4) as usize),
                            Style::default().fg(theme.border),
                        )]));
                        lines.push(Line::from(String::new()));
                    }
                    false => {}
                }

                for (i, line) in msg.content.lines().enumerate() {
                    let prefix_span = match i {
                        0 => Span::styled(
                            "› ",
                            Style::default()
                                .fg(theme.brand_accent)
                                .add_modifier(Modifier::BOLD),
                        ),
                        _ => Span::styled("  ", Style::default()),
                    };
                    lines.push(Line::from(vec![
                        prefix_span,
                        Span::styled(
                            line.to_string(),
                            Style::default()
                                .fg(theme.brand_white)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
                prev_was_user = true;
            }
            "assistant" => {
                // ── Thought badge + Monologue (CLI TUI style with bullet dot) ─
                match (msg.thinking_time, msg.reasoning.is_some()) {
                    (Some(thinking_time), _) => {
                        lines.push(Line::from(vec![
                            Span::styled("• ", Style::default().fg(theme.brand_accent)),
                            Span::styled(
                                format!("Thought for {:.1}s", thinking_time),
                                Style::default()
                                    .fg(theme.brand_accent)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }
                    (None, true) => {
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
                    (None, false) => {}
                }

                match msg.reasoning.as_deref().map(str::trim) {
                    Some(reasoning_trimmed) => match reasoning_trimmed.is_empty() {
                        false => {
                            let max_t_width = (area.width.saturating_sub(6) as usize).max(20);
                            for r_line in reasoning_trimmed.lines() {
                                match r_line.trim().is_empty() {
                                    true => {
                                        lines.push(Line::from(vec![Span::styled(
                                            "  │",
                                            Style::default().fg(theme.border),
                                        )]));
                                    }
                                    false => {
                                        for chunk in split_line_into_chunks(r_line, max_t_width) {
                                            lines.push(Line::from(vec![
                                                Span::styled(
                                                    "  │ ",
                                                    Style::default().fg(theme.border),
                                                ),
                                                Span::styled(
                                                    chunk,
                                                    Style::default()
                                                        .fg(theme.muted)
                                                        .add_modifier(Modifier::ITALIC),
                                                ),
                                            ]));
                                        }
                                    }
                                }
                            }
                            lines.push(Line::from(vec![Span::styled(
                                "  └",
                                Style::default().fg(theme.border),
                            )]));
                            lines.push(Line::from(String::new()));
                        }
                        true => {}
                    },
                    None => {}
                }

                // Assistant Markdown content
                match msg.content.is_empty() {
                    false => {
                        let content = msg.content.trim();
                        let mut in_code_block = false;
                        let table_width = area.width.saturating_sub(4) as usize;

                        let content_lines: Vec<&str> = content.lines().collect();
                        let mut i = 0;
                        while i < content_lines.len() {
                            let line = content_lines[i];
                            let trimmed = line.trim_start();

                            match trimmed.starts_with("```") {
                                true => {
                                    in_code_block = !in_code_block;
                                    match in_code_block {
                                        true => {
                                            let lang =
                                                trimmed.strip_prefix("```").unwrap_or("").trim();
                                            match lang.is_empty() {
                                                false => {
                                                    lines.push(Line::from(vec![
                                                        Span::styled(
                                                            "  ┌─ ",
                                                            Style::default().fg(theme.success),
                                                        ),
                                                        Span::styled(
                                                            lang.to_string(),
                                                            Style::default()
                                                                .fg(theme.brand_accent)
                                                                .add_modifier(Modifier::BOLD),
                                                        ),
                                                        Span::styled(
                                                            " ─────────────────────────────────",
                                                            Style::default().fg(theme.border),
                                                        ),
                                                    ]));
                                                }
                                                true => {
                                                    lines.push(Line::from(vec![Span::styled(
                                                        "  ┌──",
                                                        Style::default().fg(theme.success),
                                                    )]));
                                                }
                                            }
                                        }
                                        false => {
                                            lines.push(Line::from(vec![Span::styled(
                                                "  └──",
                                                Style::default().fg(theme.success),
                                            )]));
                                        }
                                    }
                                    i += 1;
                                    continue;
                                }
                                false => {}
                            }

                            match in_code_block {
                                true => {
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
                                false => {}
                            }

                            let is_table_start =
                                crate::channels::ratatui::markdown::is_table_row(line)
                                    && i + 1 < content_lines.len()
                                    && crate::channels::ratatui::markdown::is_divider_row(
                                        content_lines[i + 1],
                                    );
                            match is_table_start {
                                true => {
                                    let mut table_chunk = vec![line, content_lines[i + 1]];
                                    i += 2;
                                    while i < content_lines.len()
                                        && crate::channels::ratatui::markdown::is_table_row(
                                            content_lines[i],
                                        )
                                    {
                                        table_chunk.push(content_lines[i]);
                                        i += 1;
                                    }
                                    let table_rendered =
                                        crate::channels::ratatui::markdown::render_markdown_table_to_lines(
                                            &table_chunk,
                                            theme,
                                            table_width,
                                        );
                                    lines.extend(table_rendered);
                                }
                                false => {
                                    let mut spans = vec![Span::raw("  ".to_string())];
                                    spans.extend(markdown_line_to_spans(line, theme));
                                    lines.push(Line::from(spans));
                                    i += 1;
                                }
                            }
                        }
                        lines.push(Line::from(String::new()));
                    }
                    true => {}
                }

                prev_was_user = false;
            }
            "tool" => {
                let tool_name = msg.tool_name.as_deref().unwrap_or("Tool");
                let details = msg.tool_details.as_deref().unwrap_or("");

                // ── 1. Tool Origin & Category Detection ─────────────────────
                let (is_mcp, mcp_server, mcp_tool_name) = parse_mcp_tool_origin(tool_name);

                let is_subagent = matches!(
                    tool_name,
                    "delegate_task"
                        | "delegate_profile"
                        | "Delegate Task"
                        | "Delegate Profile"
                        | "orchestrate_workflow"
                );

                let is_failure = msg.tool_success == Some(false)
                    || msg.tool_summary.as_ref().is_some_and(|s| {
                        let stripped = crate::agent::style::strip_ansi_escapes(s);
                        FAILURE_INDICATORS.iter().any(|ind| stripped.contains(ind))
                    });

                let bullet_color = match is_failure {
                    true => theme.destructive,
                    false => theme.brand_accent,
                };

                // ── 2. Tool Header Spans (Style C1: Aura Modern Rail) ───────
                let mut header_spans = vec![Span::styled("• ", Style::default().fg(bullet_color))];
                let target_or_name = non_empty_or(details, tool_name);

                match (is_failure, is_mcp, is_subagent) {
                    (true, _, _) => {
                        header_spans.push(Span::styled(
                            "Failed ",
                            Style::default()
                                .fg(theme.destructive)
                                .add_modifier(Modifier::BOLD),
                        ));
                        header_spans.push(Span::styled(
                            target_or_name.to_string(),
                            Style::default().fg(theme.destructive),
                        ));
                    }
                    (false, true, _) => {
                        header_spans
                            .push(Span::styled("Called ", Style::default().fg(theme.muted)));
                        let mcp_call = format!(
                            "mcp::{}: {}",
                            mcp_server.unwrap_or("mcp"),
                            mcp_tool_name
                        );
                        header_spans
                            .push(Span::styled(mcp_call, Style::default().fg(theme.highlight)));
                        match details.is_empty() {
                            false => header_spans.push(Span::styled(
                                format!(" ({})", details),
                                Style::default().fg(theme.muted),
                            )),
                            true => {}
                        }
                    }
                    (false, false, true) => {
                        header_spans.push(Span::styled(
                            "Delegated to ",
                            Style::default().fg(theme.muted),
                        ));
                        header_spans.push(Span::styled(
                            target_or_name.to_string(),
                            Style::default().fg(theme.brand_white),
                        ));
                    }
                    (false, false, false) => {
                        let lower_name = tool_name.to_lowercase();
                        let is_doc =
                            lower_name.starts_with("opendoc") || lower_name.contains("doc");
                        let is_media = lower_name.starts_with("openmedia")
                            || lower_name.contains("video")
                            || lower_name.contains("image");
                        let is_reason = lower_name.contains("think")
                            || lower_name.contains("memory")
                            || lower_name.contains("graph");
                        match (lower_name.as_str(), is_doc, is_media, is_reason) {
                            (
                                "bash" | "exec_command" | "run_command" | "cargo" | "terminal",
                                _,
                                _,
                                _,
                            ) => {
                                header_spans
                                    .push(Span::styled("Ran ", Style::default().fg(theme.muted)));
                                header_spans.push(Span::styled(
                                    target_or_name.to_string(),
                                    Style::default().fg(theme.brand_white),
                                ));
                            }
                            (
                                "edit"
                                | "write"
                                | "write_file"
                                | "write_to_file"
                                | "patch_file"
                                | "replace_lines"
                                | "replace_file_content",
                                _,
                                _,
                                _,
                            ) => {
                                header_spans.push(Span::styled(
                                    "Edited ",
                                    Style::default().fg(theme.muted),
                                ));
                                header_spans.push(Span::styled(
                                    target_or_name.to_string(),
                                    Style::default().fg(theme.info),
                                ));
                            }
                            ("read" | "read_file" | "view_file" | "list_dir", _, _, _) => {
                                header_spans
                                    .push(Span::styled("Read ", Style::default().fg(theme.muted)));
                                header_spans.push(Span::styled(
                                    target_or_name.to_string(),
                                    Style::default().fg(theme.info),
                                ));
                            }
                            (
                                "search"
                                | "grep_search"
                                | "ast_search"
                                | "searchxyz_search_web"
                                | "web_fetch"
                                | "crawl_website",
                                _,
                                _,
                                _,
                            ) => {
                                header_spans.push(Span::styled(
                                    "Searched ",
                                    Style::default().fg(theme.muted),
                                ));
                                header_spans.push(Span::styled(
                                    target_or_name.to_string(),
                                    Style::default().fg(theme.success),
                                ));
                            }
                            (_, true, _, _) => {
                                header_spans.push(Span::styled(
                                    "Document ",
                                    Style::default().fg(theme.muted),
                                ));
                                header_spans.push(Span::styled(
                                    target_or_name.to_string(),
                                    Style::default().fg(theme.highlight),
                                ));
                            }
                            (_, false, true, _) => {
                                header_spans
                                    .push(Span::styled("Media ", Style::default().fg(theme.muted)));
                                header_spans.push(Span::styled(
                                    target_or_name.to_string(),
                                    Style::default().fg(theme.highlight),
                                ));
                            }
                            (_, false, false, true) => {
                                header_spans.push(Span::styled(
                                    "Reasoned ",
                                    Style::default().fg(theme.muted),
                                ));
                                header_spans.push(Span::styled(
                                    target_or_name.to_string(),
                                    Style::default().fg(theme.info),
                                ));
                            }
                            _ => {
                                header_spans
                                    .push(Span::styled("Used ", Style::default().fg(theme.muted)));
                                header_spans.push(Span::styled(
                                    tool_name.to_string(),
                                    Style::default().fg(theme.brand_white),
                                ));
                                match details.is_empty() {
                                    false => {
                                        header_spans.push(Span::raw(" "));
                                        header_spans.push(Span::styled(
                                            details.to_string(),
                                            Style::default().fg(theme.text_primary),
                                        ));
                                    }
                                    true => {}
                                }
                            }
                        }
                    }
                }
                lines.push(Line::from(header_spans));

                // ── 3. Multi-line Tree Output (Clean │ Rail) ─────────────────
                match msg.content.is_empty() {
                    false => {
                        let trimmed = msg.content.trim();
                        let is_diff_or_output = trimmed.lines().any(|l| {
                            l.starts_with('+')
                                || l.starts_with('-')
                                || l.starts_with("error[")
                                || l.starts_with("test ")
                                || l.starts_with("npm error")
                                || l.starts_with("Error:")
                        });
                        match is_diff_or_output {
                            true => {
                                let max_lines = 8;
                                for out_line in trimmed.lines().take(max_lines) {
                                    let rail =
                                        Span::styled("  │  ", Style::default().fg(theme.border));
                                    let is_file_hdr =
                                        out_line.starts_with("---") || out_line.starts_with("+++");
                                    let is_err_line = out_line.starts_with("error[")
                                        || out_line.starts_with("npm error")
                                        || out_line.starts_with("Error:");
                                    let rendered_line = match (
                                        is_file_hdr,
                                        out_line.strip_prefix("+ "),
                                        out_line.strip_prefix("- "),
                                        out_line.starts_with("@@"),
                                        is_err_line,
                                    ) {
                                        (true, _, _, _, _) => Line::from(vec![
                                            rail,
                                            Span::styled(
                                                out_line.to_string(),
                                                Style::default()
                                                    .fg(theme.muted)
                                                    .add_modifier(Modifier::ITALIC),
                                            ),
                                        ]),
                                        (false, Some(rest), _, _, _) => Line::from(vec![
                                            rail,
                                            Span::styled(
                                                "+ ",
                                                Style::default()
                                                    .fg(theme.success)
                                                    .add_modifier(Modifier::BOLD),
                                            ),
                                            Span::styled(
                                                rest.to_string(),
                                                Style::default().fg(theme.success),
                                            ),
                                        ]),
                                        (false, None, Some(rest), _, _) => Line::from(vec![
                                            rail,
                                            Span::styled(
                                                "- ",
                                                Style::default()
                                                    .fg(theme.destructive)
                                                    .add_modifier(Modifier::BOLD),
                                            ),
                                            Span::styled(
                                                rest.to_string(),
                                                Style::default().fg(theme.destructive),
                                            ),
                                        ]),
                                        (false, None, None, true, _) => Line::from(vec![
                                            rail,
                                            Span::styled(
                                                out_line.to_string(),
                                                Style::default().fg(theme.info),
                                            ),
                                        ]),
                                        (false, None, None, false, true) => Line::from(vec![
                                            rail,
                                            Span::styled(
                                                out_line.to_string(),
                                                Style::default().fg(theme.destructive),
                                            ),
                                        ]),
                                        _ => Line::from(vec![
                                            rail,
                                            Span::styled(
                                                out_line.to_string(),
                                                Style::default().fg(theme.text_primary),
                                            ),
                                        ]),
                                    };
                                    lines.push(rendered_line);
                                }

                                let total_out_lines = trimmed.lines().count();
                                match total_out_lines > max_lines {
                                    true => {
                                        lines.push(Line::from(vec![
                                            Span::styled(
                                                "  │  ",
                                                Style::default().fg(theme.border),
                                            ),
                                            Span::styled(
                                                format!(
                                                    "... +{} lines (output folded)",
                                                    total_out_lines - max_lines
                                                ),
                                                Style::default().fg(theme.border),
                                            ),
                                        ]));
                                    }
                                    false => {}
                                }
                            }
                            false => {}
                        }
                    }
                    true => {}
                }

                // ── 4. Outcome Summary Line (└─ ✓ / ✗ / running) ─────────────
                let mut outcome_spans =
                    vec![Span::styled("  └─ ", Style::default().fg(theme.border))];

                match (&msg.tool_summary, msg.tool_success) {
                    (Some(raw_summary), Some(success)) => {
                        let (is_succ, clean_summary) =
                            clean_tool_outcome_summary(raw_summary, Some(success));
                        let (glyph, status_color) = match is_succ {
                            true => ("✓ ", theme.success),
                            false => ("✗ ", theme.destructive),
                        };
                        outcome_spans.push(Span::styled(
                            glyph,
                            Style::default()
                                .fg(status_color)
                                .add_modifier(Modifier::BOLD),
                        ));
                        outcome_spans.push(Span::styled(
                            clean_summary.clone(),
                            Style::default().fg(status_color),
                        ));

                        let has_inline_duration = clean_summary.contains("ms)")
                            || clean_summary.contains("s)")
                            || clean_summary.contains(" in ");
                        match (has_inline_duration, msg.tool_duration_ms) {
                            (false, Some(duration_ms)) => {
                                outcome_spans.push(Span::styled(
                                    format_tool_duration(duration_ms),
                                    Style::default().fg(status_color),
                                ));
                            }
                            _ => {}
                        }
                        lines.push(Line::from(outcome_spans));
                    }
                    _ => {
                        let (symbol, text, color) = match msg.tool_success {
                            Some(true) => {
                                ("✓ ".to_string(), "completed".to_string(), theme.success)
                            }
                            Some(false) => {
                                ("✗ ".to_string(), "failed".to_string(), theme.destructive)
                            }
                            None => {
                                let frame_idx = ((app.elapsed_millis() / 80) as usize)
                                    % super::theme::SPINNER_FRAMES.len();
                                let spinner_frame = super::theme::SPINNER_FRAMES[frame_idx];
                                let verb = classify_running_tool_verb(msg.tool_name.as_deref());
                                (
                                    format!("{} ", spinner_frame),
                                    verb.to_string(),
                                    theme.brand_accent,
                                )
                            }
                        };
                        outcome_spans.push(Span::styled(
                            symbol,
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ));
                        outcome_spans.push(Span::styled(text, Style::default().fg(color)));
                        match msg.tool_duration_ms {
                            Some(duration_ms) => {
                                outcome_spans.push(Span::styled(
                                    format_tool_duration(duration_ms),
                                    Style::default().fg(color),
                                ));
                            }
                            None => {}
                        }
                        lines.push(Line::from(outcome_spans));
                    }
                }

                lines.push(Line::from(String::new()));
                prev_was_user = false;
            }
            _ => {
                for line in msg.content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("• ", Style::default().fg(theme.brand_accent)),
                        Span::styled(line.to_string(), Style::default().fg(theme.text_primary)),
                    ]));
                }
                prev_was_user = false;
            }
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .style(Style::default().bg(theme.bg_primary)),
        )
        .style(Style::default().bg(theme.bg_primary))
        .wrap(Wrap { trim: false });

    let total_rendered_lines = paragraph.line_count(area.width) as u32;
    let viewport_height = area.height as u32;
    let max_scroll = total_rendered_lines.saturating_sub(viewport_height);
    app.max_scroll = max_scroll;

    let scroll = match app.auto_scroll {
        true => max_scroll,
        false => app.scroll_offset.min(max_scroll),
    };

    let scroll_u16 = scroll.min(u16::MAX as u32) as u16;
    let paragraph = paragraph.scroll((scroll_u16, 0));
    f.render_widget(paragraph, area);
}

fn split_line_into_chunks(line: &str, max_width: usize) -> Vec<String> {
    match line.len() <= max_width {
        true => return vec![line.to_string()],
        false => {}
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        match (current.is_empty(), current.len() + 1 + word.len() <= max_width) {
            (true, _) => current.push_str(word),
            (false, true) => {
                current.push(' ');
                current.push_str(word);
            }
            (false, false) => {
                chunks.push(current);
                current = word.to_string();
            }
        }
    }
    match current.is_empty() {
        false => chunks.push(current),
        true => {}
    }
    match chunks.is_empty() {
        true => chunks.push(line.to_string()),
        false => {}
    }
    chunks
}
