use super::app::RatatuiApp;
use super::markdown::markdown_line_to_spans;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Cleans raw tool outcome summary: strips ANSI codes, detects success/failure,
/// and strips duplicate ticks, crosses, and "Failed:" prefixes.
pub fn clean_tool_outcome_summary(raw: &str, default_success: Option<bool>) -> (bool, String) {
    let stripped = crate::agent::style::strip_ansi_escapes(raw);
    let trimmed = stripped.trim();

    let has_fail = trimmed.contains('✗')
        || trimmed.contains('✖')
        || trimmed.contains('✕')
        || trimmed.contains("Failed")
        || trimmed.contains("failed")
        || trimmed.contains("error")
        || trimmed.contains("Error");

    let is_succ = if has_fail {
        false
    } else if trimmed.contains('✓') || trimmed.contains('✔') {
        true
    } else {
        default_success.unwrap_or(true)
    };

    let mut clean = trimmed;
    // Strip leading ticks, crosses, bullets, spaces
    while let Some(c) = clean.chars().next() {
        if c == '✓' || c == '✔' || c == '✗' || c == '✖' || c == '✕' || c == '•' || c == ' ' {
            clean = &clean[c.len_utf8()..];
        } else {
            break;
        }
    }
    clean = clean.trim_start();

    // Strip "Failed:" or "failed:" prefix
    if let Some(rest) = clean.strip_prefix("Failed:") {
        clean = rest.trim_start();
    } else if let Some(rest) = clean.strip_prefix("failed:") {
        clean = rest.trim_start();
    } else if let Some(rest) = clean.strip_prefix("Failed") {
        clean = rest.trim_start();
    } else if let Some(rest) = clean.strip_prefix("failed") {
        clean = rest.trim_start();
    }

    // Strip trailing ticks, crosses, spaces
    while let Some(c) = clean.chars().last() {
        if c == '✓' || c == '✔' || c == '✗' || c == '✖' || c == '✕' || c == '•' || c == ' ' {
            clean = &clean[..clean.len() - c.len_utf8()];
        } else {
            break;
        }
    }
    clean = clean.trim();

    let final_text = if clean.is_empty() {
        if is_succ { "completed" } else { "failed" }
    } else {
        clean
    };

    (is_succ, final_text.to_string())
}

pub(crate) fn render_timeline(f: &mut Frame, app: &mut RatatuiApp, area: Rect) {
    let theme = &app.theme;
    f.render_widget(Clear, area);
    let mut lines = Vec::new();

    // ── OpenZ CLI Authentic ASCII Logo Banner ───────────────────────────────
    let logo_parts = [
        (" ██████╗ ██████╗ ███████╗███╗   ██╗", "███████╗"),
        ("██╔═══██╗██╔══██╗██╔════╝████╗  ██║", "╚══███╔╝"),
        ("██║   ██║██████╔╝█████╗  ██╔██╗ ██║", "  ███╔╝ "),
        ("██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║", " ███╔╝  "),
        ("╚██████╔╝██║     ███████╗██║ ╚████║", "███████╗"),
        (" ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝", "╚══════╝"),
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
                            if r_line.trim().is_empty() {
                                lines.push(Line::from(vec![
                                    Span::styled("  │", Style::default().fg(theme.border)),
                                ]));
                                continue;
                            }
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
                        lines.push(Line::from(vec![
                            Span::styled("  └", Style::default().fg(theme.border)),
                        ]));
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

                // ── 1. Tool Origin & Category Detection ─────────────────────
                let (is_mcp, mcp_server, mcp_tool_name) = if let Some(stripped) = tool_name.strip_prefix("mcp::") {
                    if let Some((srv, t)) = stripped.split_once("::") {
                        (true, Some(srv), t)
                    } else {
                        (true, None, stripped)
                    }
                } else if let Some(stripped) = tool_name.strip_prefix("mcp__") {
                    if let Some((srv, t)) = stripped.split_once("__") {
                        (true, Some(srv), t)
                    } else {
                        (true, None, stripped)
                    }
                } else if let Some((srv, t)) = tool_name.split_once("::") {
                    (true, Some(srv), t)
                } else {
                    (false, None, tool_name)
                };

                let is_subagent = tool_name == "delegate_task"
                    || tool_name == "delegate_profile"
                    || tool_name == "Delegate Task"
                    || tool_name == "Delegate Profile"
                    || tool_name == "orchestrate_workflow";

                let is_failure = msg.tool_success == Some(false)
                    || msg.tool_summary.as_ref().is_some_and(|s| {
                        let stripped = crate::agent::style::strip_ansi_escapes(s);
                        stripped.contains('✗')
                            || stripped.contains('✖')
                            || stripped.contains('✕')
                            || stripped.contains("Failed")
                            || stripped.contains("failed")
                            || stripped.contains("error")
                            || stripped.contains("Error")
                    });

                let bullet_color = if is_failure {
                    theme.destructive
                } else {
                    theme.brand_accent
                };

                // ── 2. Tool Header Spans (Style C1: Aura Modern Rail) ───────
                let mut header_spans = Vec::new();
                header_spans.push(Span::styled("• ", Style::default().fg(bullet_color)));

                if is_failure {
                    header_spans.push(Span::styled(
                        "Failed ",
                        Style::default()
                            .fg(theme.destructive)
                            .add_modifier(Modifier::BOLD),
                    ));
                    let fail_target = if !details.is_empty() {
                        details
                    } else {
                        tool_name
                    };
                    header_spans.push(Span::styled(
                        fail_target.to_string(),
                        Style::default().fg(theme.destructive),
                    ));
                } else if is_mcp {
                    header_spans.push(Span::styled("Called ", Style::default().fg(theme.muted)));
                    let mcp_call = format!("mcp::{}: {}", mcp_server.unwrap_or("mcp"), mcp_tool_name);
                    header_spans.push(Span::styled(
                        mcp_call,
                        Style::default().fg(theme.highlight),
                    ));
                    if !details.is_empty() {
                        header_spans.push(Span::styled(
                            format!(" ({})", details),
                            Style::default().fg(theme.muted),
                        ));
                    }
                } else if is_subagent {
                    header_spans.push(Span::styled("Delegated to ", Style::default().fg(theme.muted)));
                    let target = if !details.is_empty() { details } else { tool_name };
                    header_spans.push(Span::styled(
                        target.to_string(),
                        Style::default().fg(theme.brand_white),
                    ));
                } else {
                    match tool_name.to_lowercase().as_str() {
                        "bash" | "exec_command" | "run_command" | "cargo" | "terminal" => {
                            header_spans.push(Span::styled("Ran ", Style::default().fg(theme.muted)));
                            let target = if !details.is_empty() { details } else { tool_name };
                            header_spans.push(Span::styled(
                                target.to_string(),
                                Style::default().fg(theme.brand_white),
                            ));
                        }
                        "edit" | "write" | "write_file" | "write_to_file" | "patch_file"
                        | "replace_lines" | "replace_file_content" => {
                            header_spans.push(Span::styled("Edited ", Style::default().fg(theme.muted)));
                            let target = if !details.is_empty() { details } else { tool_name };
                            header_spans.push(Span::styled(
                                target.to_string(),
                                Style::default().fg(theme.info),
                            ));
                        }
                        "read" | "read_file" | "view_file" | "list_dir" => {
                            header_spans.push(Span::styled("Read ", Style::default().fg(theme.muted)));
                            let target = if !details.is_empty() { details } else { tool_name };
                            header_spans.push(Span::styled(
                                target.to_string(),
                                Style::default().fg(theme.info),
                            ));
                        }
                        "search" | "grep_search" | "ast_search" | "searchxyz_search_web"
                        | "web_fetch" | "crawl_website" => {
                            header_spans.push(Span::styled("Searched ", Style::default().fg(theme.muted)));
                            let target = if !details.is_empty() { details } else { tool_name };
                            header_spans.push(Span::styled(
                                target.to_string(),
                                Style::default().fg(theme.success),
                            ));
                        }
                        s if s.starts_with("opendoc") || s.contains("doc") => {
                            header_spans.push(Span::styled("Document ", Style::default().fg(theme.muted)));
                            let target = if !details.is_empty() { details } else { tool_name };
                            header_spans.push(Span::styled(
                                target.to_string(),
                                Style::default().fg(theme.highlight),
                            ));
                        }
                        s if s.starts_with("openmedia")
                            || s.contains("video")
                            || s.contains("image") => {
                            header_spans.push(Span::styled("Media ", Style::default().fg(theme.muted)));
                            let target = if !details.is_empty() { details } else { tool_name };
                            header_spans.push(Span::styled(
                                target.to_string(),
                                Style::default().fg(theme.highlight),
                            ));
                        }
                        s if s.contains("think")
                            || s.contains("memory")
                            || s.contains("graph") => {
                            header_spans.push(Span::styled("Reasoned ", Style::default().fg(theme.muted)));
                            let target = if !details.is_empty() { details } else { tool_name };
                            header_spans.push(Span::styled(
                                target.to_string(),
                                Style::default().fg(theme.info),
                            ));
                        }
                        _ => {
                            header_spans.push(Span::styled("Used ", Style::default().fg(theme.muted)));
                            header_spans.push(Span::styled(
                                tool_name.to_string(),
                                Style::default().fg(theme.brand_white),
                            ));
                            if !details.is_empty() {
                                header_spans.push(Span::raw(" "));
                                header_spans.push(Span::styled(
                                    details.to_string(),
                                    Style::default().fg(theme.text_primary),
                                ));
                            }
                        }
                    }
                }
                lines.push(Line::from(header_spans));

                // ── 3. Multi-line Tree Output (Clean │ Rail) ─────────────────
                if !msg.content.is_empty() {
                    let trimmed = msg.content.trim();
                    let is_diff_or_output = trimmed.lines().any(|l| {
                        l.starts_with('+')
                            || l.starts_with('-')
                            || l.starts_with("error[")
                            || l.starts_with("test ")
                            || l.starts_with("npm error")
                            || l.starts_with("Error:")
                    });
                    if is_diff_or_output {
                        let max_lines = 8;
                        for out_line in trimmed.lines().take(max_lines) {
                            if out_line.starts_with("---") || out_line.starts_with("+++") {
                                lines.push(Line::from(vec![
                                    Span::styled("  │  ", Style::default().fg(theme.border)),
                                    Span::styled(
                                        out_line.to_string(),
                                        Style::default()
                                            .fg(theme.muted)
                                            .add_modifier(Modifier::ITALIC),
                                    ),
                                ]));
                            } else if let Some(rest) = out_line.strip_prefix("+ ") {
                                lines.push(Line::from(vec![
                                    Span::styled("  │  ", Style::default().fg(theme.border)),
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
                                ]));
                            } else if let Some(rest) = out_line.strip_prefix("- ") {
                                lines.push(Line::from(vec![
                                    Span::styled("  │  ", Style::default().fg(theme.border)),
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
                                ]));
                            } else if out_line.starts_with("@@") {
                                lines.push(Line::from(vec![
                                    Span::styled("  │  ", Style::default().fg(theme.border)),
                                    Span::styled(
                                        out_line.to_string(),
                                        Style::default().fg(theme.info),
                                    ),
                                ]));
                            } else if out_line.starts_with("error[")
                                || out_line.starts_with("npm error")
                                || out_line.starts_with("Error:")
                            {
                                lines.push(Line::from(vec![
                                    Span::styled("  │  ", Style::default().fg(theme.border)),
                                    Span::styled(
                                        out_line.to_string(),
                                        Style::default().fg(theme.destructive),
                                    ),
                                ]));
                            } else {
                                lines.push(Line::from(vec![
                                    Span::styled("  │  ", Style::default().fg(theme.border)),
                                    Span::styled(
                                        out_line.to_string(),
                                        Style::default().fg(theme.text_primary),
                                    ),
                                ]));
                            }
                        }

                        let total_out_lines = trimmed.lines().count();
                        if total_out_lines > max_lines {
                            lines.push(Line::from(vec![
                                Span::styled("  │  ", Style::default().fg(theme.border)),
                                Span::styled(
                                    format!(
                                        "... +{} lines (output folded)",
                                        total_out_lines - max_lines
                                    ),
                                    Style::default().fg(theme.border),
                                ),
                            ]));
                        }
                    }
                }

                // ── 4. Outcome Summary Line (└─ ✓ / ✗ / running) ─────────────
                let mut outcome_spans = vec![
                    Span::styled("  └─ ", Style::default().fg(theme.border)),
                ];

                if let (Some(ref raw_summary), Some(success)) = (&msg.tool_summary, msg.tool_success) {
                    let (is_succ, clean_summary) = clean_tool_outcome_summary(raw_summary, Some(success));

                    if is_succ {
                        outcome_spans.push(Span::styled(
                            "✓ ",
                            Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
                        ));
                        outcome_spans.push(Span::styled(
                            clean_summary.clone(),
                            Style::default().fg(theme.success),
                        ));
                    } else {
                        outcome_spans.push(Span::styled(
                            "✗ ",
                            Style::default().fg(theme.destructive).add_modifier(Modifier::BOLD),
                        ));
                        outcome_spans.push(Span::styled(
                            clean_summary.clone(),
                            Style::default().fg(theme.destructive),
                        ));
                    }

                    if !clean_summary.contains("ms)")
                        && !clean_summary.contains("s)")
                        && !clean_summary.contains(" in ")
                    {
                        if let Some(duration_ms) = msg.tool_duration_ms {
                            let dur_str = if duration_ms < 1000 {
                                format!(" ({}ms)", duration_ms)
                            } else {
                                format!(" ({:.2}s)", duration_ms as f64 / 1000.0)
                            };
                            let dur_color = if is_succ { theme.success } else { theme.destructive };
                            outcome_spans.push(Span::styled(dur_str, Style::default().fg(dur_color)));
                        }
                    }

                    lines.push(Line::from(outcome_spans));
                } else {
                    let (symbol, text, color) = match msg.tool_success {
                        Some(true) => ("✓ ".to_string(), "completed".to_string(), theme.success),
                        Some(false) => ("✗ ".to_string(), "failed".to_string(), theme.destructive),
                        None => {
                            let frame_idx = ((app.elapsed_millis() / 80) as usize) % super::theme::SPINNER_FRAMES.len();
                            let spinner_frame = super::theme::SPINNER_FRAMES[frame_idx];
                            let verb = if let Some(ref name) = msg.tool_name {
                                let n = name.to_ascii_lowercase();
                                if n.contains("deep_research") || n.contains("deep research") {
                                    "deep researching..."
                                } else if n.contains("search") || n.contains("web") || n.contains("crawl") || n.contains("fetch") {
                                    "researching..."
                                } else if n.contains("write") || n.contains("create") {
                                    "writing..."
                                } else if n.contains("patch") || n.contains("edit") || n.contains("replace") {
                                    "editing..."
                                } else if n.contains("media") || n.contains("video") || n.contains("image") || n.contains("svg") {
                                    "rendering..."
                                } else if n.contains("doc") || n.contains("pdf") || n.contains("xlsx") || n.contains("docx") {
                                    "processing..."
                                } else if n.contains("check") || n.contains("test") || n.contains("clippy") || n.contains("lint") {
                                    "checking..."
                                } else if n.contains("delegate") || n.contains("subagent") || n.contains("orchestrat") {
                                    "delegating..."
                                } else {
                                    "running..."
                                }
                            } else {
                                "running..."
                            };
                            (format!("{} ", spinner_frame), verb.to_string(), theme.brand_accent)
                        }
                    };
                    outcome_spans.push(Span::styled(
                        symbol,
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ));
                    outcome_spans.push(Span::styled(
                        text,
                        Style::default().fg(color),
                    ));
                    if let Some(duration_ms) = msg.tool_duration_ms {
                        let dur_str = if duration_ms < 1000 {
                            format!(" ({}ms)", duration_ms)
                        } else {
                            format!(" ({:.2}s)", duration_ms as f64 / 1000.0)
                        };
                        outcome_spans.push(Span::styled(dur_str, Style::default().fg(color)));
                    }
                    lines.push(Line::from(outcome_spans));
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
