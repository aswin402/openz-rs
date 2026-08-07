use super::app::{ModelSelectState, RatatuiApp};
use super::theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
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
/// Handles: **bold**, `inline code`, # headings, - list items
fn markdown_line_to_spans(line: &str) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();

    // Heading detection
    if trimmed.starts_with("# ") {
        return vec![Span::styled(
            line.to_string(),
            theme::heading_style(),
        )];
    }
    if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
        return vec![Span::styled(
            line.to_string(),
            theme::heading_style(),
        )];
    }

    // Horizontal rule
    if trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3 {
        return vec![Span::styled(
            "──────".to_string(),
            theme::divider_style(),
        )];
    }

    // List items
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        let rest = &trimmed[2..];
        let mut spans = vec![
            Span::raw(indent),
            Span::styled("• ".to_string(), theme::list_marker_style()),
        ];
        spans.extend(parse_inline_markdown(rest));
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
                Span::styled(
                    format!("{}. ", number),
                    theme::list_marker_style(),
                ),
            ];
            spans.extend(parse_inline_markdown(rest));
            return spans;
        }
    }

    // Regular text with inline formatting
    parse_inline_markdown(line)
}

/// Parse inline markdown (**bold** and `code`) into styled spans.
fn parse_inline_markdown(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = text.to_string();

    // Strategy: find the earliest bold or code match, split around it, recurse
    loop {
        if remaining.is_empty() {
            break;
        }

        let bold_match = re_bold().and_then(|re| {
            re.find(&remaining).map(|m| (m.start(), m.end(), "bold"))
        });
        let code_match = re_code().and_then(|re| {
            re.find(&remaining).map(|m| (m.start(), m.end(), "code"))
        });

        let earliest = match (bold_match, code_match) {
            (Some(b), Some(c)) => {
                if b.0 <= c.0 { Some(b) } else { Some(c) }
            }
            (Some(b), None) => Some(b),
            (None, Some(c)) => Some(c),
            (None, None) => None,
        };

        match earliest {
            Some((start, end, kind)) => {
                // Text before the match
                if start > 0 {
                    spans.push(Span::styled(
                        remaining[..start].to_string(),
                        theme::assistant_text_style(),
                    ));
                }

                let matched = &remaining[start..end];
                match kind {
                    "bold" => {
                        // Strip ** from both ends
                        let inner = &matched[2..matched.len() - 2];
                        spans.push(Span::styled(
                            inner.to_string(),
                            theme::bold_style(),
                        ));
                    }
                    "code" => {
                        // Strip ` from both ends
                        let inner = &matched[1..matched.len() - 1];
                        spans.push(Span::styled(
                            inner.to_string(),
                            theme::code_style(),
                        ));
                    }
                    _ => {}
                }

                remaining = remaining[end..].to_string();
            }
            None => {
                // No more matches — push remaining as plain text
                spans.push(Span::styled(
                    remaining.clone(),
                    theme::assistant_text_style(),
                ));
                break;
            }
        }
    }

    if spans.is_empty() {
        spans.push(Span::styled(
            text.to_string(),
            theme::assistant_text_style(),
        ));
    }

    spans
}

// ── Main render function ────────────────────────────────────────────────────

pub fn render_ratatui_ui(f: &mut Frame, app: &RatatuiApp) {
    let matches = app.matching_slash_commands();
    let has_popup = !app.typed_input.is_empty() && app.typed_input[0] == '/' && !matches.is_empty();

    let popup_lines_count = if has_popup {
        (matches.len().min(5) + 2) as u16
    } else {
        0
    };

    let is_menu_active = match &app.model_select {
        ModelSelectState::Closed => false,
        _ => true,
    };

    let (input_height, menu_height) = if is_menu_active {
        (9, 0) // Allocate 9 lines for the model selection menu
    } else {
        (3, popup_lines_count)
    };

    // Codex-style layout: chat (flex) → input (padded, height 3 or menu height 9) → status (bottom right) → (popup below status)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),                     // Chat scrollback
            Constraint::Length(input_height),        // Input Box or Selection Menu
            Constraint::Length(1),                   // Status bar
            Constraint::Length(menu_height),         // Autocomplete popup
        ])
        .split(f.area());

    // ── 1. Chat Scrollback ──────────────────────────────────────────────────
    let text_lines = render_conversation(app);

    let conversation = Paragraph::new(Text::from(text_lines))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset as u16, 0));
    f.render_widget(conversation, chunks[0]);

    // ── 2. Content Area (Menu or Input box) ──────────────────────────────────
    if is_menu_active {
        render_model_menu(f, app, chunks[1]);
    } else {
        render_input(f, app, chunks[1]);
    }

    // ── 3. Status Bar (model · provider · MCP · context) ───────────────────
    render_status_bar(f, app, chunks[2]);

    // ── 4. Autocomplete Popup (below status bar) ─────────────────────────────
    if !is_menu_active && has_popup {
        render_autocomplete(f, app, &matches, chunks[3]);
    }
}

// ── Conversation Renderer ───────────────────────────────────────────────────

fn render_conversation(app: &RatatuiApp) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // ASCII Logo (same style as CLI TUI)
    let logo_lines = [
        "     ██████╗ ██████╗ ███████╗███╗   ██╗███████╗",
        "    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║╚══███╔╝",
    ];
    let logo_mixed = [
        ("    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║  ", "███╔╝ "),
        ("    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║ ", "███╔╝  "),
        ("    ╚██████╔╝██║     ███████╗██║ ╚████║", "███████╗"),
    ];
    let logo_bottom = "     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝╚══════╝";

    for l in &logo_lines {
        lines.push(Line::from(vec![Span::styled(
            l.to_string(),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )]));
    }
    for (white_part, orange_part) in &logo_mixed {
        lines.push(Line::from(vec![
            Span::styled(
                white_part.to_string(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                orange_part.to_string(),
                Style::default().fg(theme::RED_ORANGE).add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    lines.push(Line::from(vec![Span::styled(
        logo_bottom.to_string(),
        Style::default().fg(theme::RED_ORANGE).add_modifier(Modifier::BOLD),
    )]));

    // Version line only (provider/model/cwd are in the status bar)
    lines.push(Line::from(vec![Span::styled(
        format!(" openz v{}", env!("CARGO_PKG_VERSION")),
        Style::default().fg(theme::RED_ORANGE).add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![Span::styled(
        format!(" {} | {}", app.provider, app.model),
        theme::status_accent_style(),
    )]));
    lines.push(Line::from(""));

    // Empty line for breathing room
    lines.push(Line::from(""));

    // ── Render messages ─────────────────────────────────────────────────────
    let mut prev_was_user = false;

    for msg in &app.messages {
        match msg.role.as_str() {
            "user" => {
                // Divider between conversation turns (not before first user msg)
                if prev_was_user || lines.len() > 12 {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![Span::styled(
                        "─".repeat(50),
                        theme::divider_style(),
                    )]));
                }

                // User message with `> ` prefix
                let content_lines: Vec<&str> = msg.content.lines().collect();
                for (i, line) in content_lines.iter().enumerate() {
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::styled("> ", theme::user_prefix_style()),
                            Span::styled(
                                line.to_string(),
                                theme::user_text_style(),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled("  ", Style::default()),
                            Span::styled(
                                line.to_string(),
                                theme::user_text_style(),
                            ),
                        ]));
                    }
                }
                prev_was_user = true;
            }
            "assistant" => {
                // Thinking/Reasoning block (if present)
                if let Some(thinking_time) = msg.thinking_time {
                    lines.push(Line::from(vec![
                        Span::styled("● ", theme::thinking_bullet_style()),
                        Span::styled(
                            format!("Thought for {:.1}s", thinking_time),
                            theme::thinking_time_style(),
                        ),
                    ]));
                }

                if let Some(ref reasoning) = msg.reasoning {
                    let reasoning_trimmed = reasoning.trim();
                    if !reasoning_trimmed.is_empty() {
                        for r_line in reasoning_trimmed.lines() {
                            lines.push(Line::from(vec![
                                Span::styled("  L ", theme::reasoning_style()),
                                Span::styled(
                                    r_line.to_string(),
                                    theme::reasoning_style(),
                                ),
                            ]));
                        }
                    }
                    lines.push(Line::from(""));
                }

                // Assistant content with inline markdown
                if !msg.content.is_empty() {
                    let content = msg.content.trim();
                    let mut in_code_block = false;

                    for line in content.lines() {
                        let trimmed = line.trim_start();

                        // Code block fence
                        if trimmed.starts_with("```") {
                            in_code_block = !in_code_block;
                            if in_code_block {
                                // Start of code block — show language label
                                let lang = trimmed.strip_prefix("```").unwrap_or("").trim();
                                if !lang.is_empty() {
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            format!("  ┌─ {} ", lang),
                                            theme::code_style(),
                                        ),
                                    ]));
                                } else {
                                    lines.push(Line::from(vec![
                                        Span::styled("  ┌──", theme::code_style()),
                                    ]));
                                }
                            } else {
                                lines.push(Line::from(vec![
                                    Span::styled("  └──", theme::code_style()),
                                ]));
                            }
                            continue;
                        }

                        if in_code_block {
                            // Code block content — indented, emerald color
                            lines.push(Line::from(vec![
                                Span::styled("  │ ", theme::code_style()),
                                Span::styled(
                                    line.to_string(),
                                    theme::code_style(),
                                ),
                            ]));
                        } else {
                            // Regular markdown line
                            let mut spans = vec![Span::raw("  ".to_string())];
                            spans.extend(markdown_line_to_spans(line));
                            lines.push(Line::from(spans));
                        }
                    }
                    lines.push(Line::from(""));
                }

                prev_was_user = false;
            }
            "tool" => {
                // Tool execution cell
                if let Some(ref tool_name) = msg.tool_name {
                    let details = msg.tool_details.as_deref().unwrap_or("");
                    if details.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled("● ", theme::tool_bullet_style()),
                            Span::styled(
                                tool_name.to_string(),
                                theme::tool_name_style(),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled("● ", theme::tool_bullet_style()),
                            Span::styled(
                                format!("{} ", tool_name),
                                theme::tool_name_style(),
                            ),
                            Span::styled(
                                details.to_string(),
                                theme::tool_details_style(),
                            ),
                        ]));
                    }
                }

                // Tool outcome
                if !msg.content.is_empty() {
                    let content = msg.content.trim();
                    // Check for error/success markers
                    let has_error = content.contains('✗') || content.contains('✖') || content.contains("error");
                    let summary = if content.len() > 120 {
                        format!("{}...", &content[..120])
                    } else {
                        content.to_string()
                    };

                    if has_error {
                        lines.push(Line::from(vec![
                            Span::styled("  └ ", theme::reasoning_style()),
                            Span::styled("✗ ", theme::error_style()),
                            Span::styled(
                                summary,
                                theme::reasoning_style(),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled("  └ ", theme::reasoning_style()),
                            Span::styled("✓ ", theme::success_style()),
                            Span::styled(
                                summary,
                                theme::reasoning_style(),
                            ),
                        ]));
                    }
                }

                prev_was_user = false;
            }
            _ => {
                // System or other messages
                if !msg.content.is_empty() {
                    for line in msg.content.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("  ", Style::default()),
                            Span::styled(
                                line.to_string(),
                                theme::reasoning_style(),
                            ),
                        ]));
                    }
                }
                prev_was_user = false;
            }
        }
    }

    // Thinking spinner at the end if agent is working
    if app.is_thinking {
        let frame_idx = app.spinner_idx % theme::SPINNER_FRAMES.len();
        let spinner = theme::SPINNER_FRAMES[frame_idx];
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", spinner),
                theme::thinking_bullet_style(),
            ),
            Span::styled(
                "Working...".to_string(),
                theme::thinking_time_style(),
            ),
        ]));
    }

    lines
}

// ── Input Renderer ──────────────────────────────────────────────────────────

fn render_input(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let max_width = area.width.saturating_sub(3).max(1) as usize;
    let input_bg = Color::Rgb(40, 44, 52); // Subtle grey background block

    let mut lines = Vec::new();

    // Line 1: Empty padding top
    lines.push(Line::from(Span::styled(
        " ".repeat(area.width as usize),
        Style::default().bg(input_bg),
    )));

    // Line 2: The actual input content
    if app.typed_input.is_empty() {
        // Placeholder
        lines.push(Line::from(vec![
            Span::styled("› ", Style::default().fg(theme::EMERALD).bg(input_bg)),
            Span::styled(
                format!("{:<width$}", "type here...", width = max_width),
                Style::default().fg(theme::AURA_SLATE).bg(input_bg),
            ),
        ]));
    } else {
        let input_str: String = app.typed_input.iter().collect();
        let display = if input_str.len() > max_width {
            let start = input_str.len().saturating_sub(max_width);
            format!("…{}", &input_str[start..])
        } else {
            input_str
        };
        let padded = format!("{:<width$}", display, width = max_width);
        lines.push(Line::from(vec![
            Span::styled("› ", Style::default().fg(theme::EMERALD).bg(input_bg)),
            Span::styled(padded, Style::default().fg(Color::White).bg(input_bg)),
        ]));
    }

    // Line 3: Empty padding bottom
    lines.push(Line::from(Span::styled(
        " ".repeat(area.width as usize),
        Style::default().bg(input_bg),
    )));

    let input_p = Paragraph::new(Text::from(lines));
    f.render_widget(input_p, area);

    // Cursor position (on the middle line of the 3-line block)
    let cursor_col = if app.typed_input.is_empty() {
        2 // After "› "
    } else {
        let visible_len = app.typed_input.len().min(max_width);
        2 + if app.typed_input.len() > max_width {
            visible_len
        } else {
            app.cursor_idx
        }
    };
    f.set_cursor_position((area.x + cursor_col as u16, area.y + 1));
}

// ── Status Bar Renderer ─────────────────────────────────────────────────────

fn render_status_bar(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let (mcp_loaded, mcp_failed, _mcp_total) = crate::tools::mcp::get_mcp_stats();
    let mcp_done = crate::channels::cli::mcp::is_mcp_done();

    let mut spans = Vec::new();

    // 1. Model name
    let model_display = if app.model.len() > 30 {
        format!("{}…", &app.model[..29])
    } else {
        app.model.clone()
    };
    spans.push(Span::styled(model_display, theme::status_accent_style()));
    spans.push(Span::styled(" · ", theme::status_bar_style()));

    // 2. Provider
    spans.push(Span::styled(app.provider.clone(), theme::status_accent_style()));
    spans.push(Span::styled(" · ", theme::status_bar_style()));

    // 3. MCP status (Styled RED_ORANGE without ◇ diamond symbol)
    let mcp_style = Style::default().fg(theme::RED_ORANGE);
    if !mcp_done {
        let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame_idx = app.spinner_idx % spinner_frames.len();
        spans.push(Span::styled(
            format!("MCP {} ", spinner_frames[frame_idx]),
            mcp_style,
        ));
    } else if mcp_failed == 0 {
        spans.push(Span::styled(
            format!("MCP {}✓", mcp_loaded),
            mcp_style,
        ));
    } else {
        spans.push(Span::styled(
            format!("MCP {}✓ ", mcp_loaded),
            mcp_style,
        ));
        spans.push(Span::styled(
            format!("{}✗", mcp_failed),
            theme::error_style(),
        ));
    }
    spans.push(Span::styled(" · ", theme::status_bar_style()));

    // 4. Token Context
    let token_context = format!("{}/1M", app.approx_tokens);
    spans.push(Span::styled(token_context, theme::status_accent_style()));

    // Add trailing space for padding on the right
    spans.push(Span::raw(" "));

    let status_line = Line::from(spans);
    let status_p = Paragraph::new(status_line).alignment(Alignment::Right);
    f.render_widget(status_p, area);
}

// ── Autocomplete Popup ──────────────────────────────────────────────────────

fn render_autocomplete(
    f: &mut Frame,
    app: &RatatuiApp,
    matches: &[(String, String)],
    area: Rect,
) {
    let mut popup_lines = Vec::new();
    let display_limit = 5;
    let start_idx = if let Some(idx) = app.selected_index {
        if idx >= display_limit { idx - display_limit + 1 } else { 0 }
    } else {
        0
    };
    let end_idx = (start_idx + display_limit).min(matches.len());

    for (i, (cmd, desc)) in matches.iter().enumerate().take(end_idx).skip(start_idx) {
        let is_selected = app.selected_index == Some(i);
        if is_selected {
            popup_lines.push(Line::from(vec![
                Span::styled("> ", theme::user_prefix_style()),
                Span::styled(
                    format!("{:<30}", cmd),
                    Style::default().fg(theme::RED_ORANGE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc.as_str(), theme::status_accent_style()),
            ]));
        } else {
            popup_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{:<30}", cmd),
                    theme::assistant_text_style(),
                ),
                Span::styled(desc.as_str(), theme::status_accent_style()),
            ]));
        }
    }

    popup_lines.push(Line::from(Span::styled(
        "  ↑/↓ Navigate · enter Select · tab Complete",
        theme::status_accent_style(),
    )));

    let help_p = Paragraph::new(Text::from(popup_lines));
    f.render_widget(help_p, area);
}

// ── Interactive Model Selection Menu ────────────────────────────────────────

fn render_model_menu(f: &mut Frame, app: &RatatuiApp, area: Rect) {
    let mut lines = Vec::new();
    let menu_bg = Color::Rgb(33, 37, 43); // Darker charcoal background for menu

    // Header info line
    lines.push(Line::from(vec![
        Span::styled("Current active model: ", Style::default().fg(theme::AURA_SLATE).bg(menu_bg)),
        Span::styled(app.model.clone(), Style::default().fg(Color::White).bg(menu_bg)),
        Span::styled(" | Provider: ", Style::default().fg(theme::AURA_SLATE).bg(menu_bg)),
        Span::styled(app.provider.clone(), Style::default().fg(Color::White).bg(menu_bg)),
    ]));

    // Title / Prompt line
    let prompt = match &app.model_select {
        ModelSelectState::ChoosingProvider { .. } => "> Choose an LLM provider:".to_string(),
        ModelSelectState::FetchingModels { provider_display, .. } => {
            let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let frame = spinner[app.spinner_idx as usize % spinner.len()];
            format!("{} Fetching models from {}...", frame, provider_display)
        }
        ModelSelectState::ChoosingModel { provider_display, .. } => {
            format!("> Choose a model from {}:", provider_display)
        }
        _ => String::new(),
    };
    lines.push(Line::from(Span::styled(
        prompt,
        Style::default().fg(theme::RED_ORANGE).add_modifier(Modifier::BOLD).bg(menu_bg),
    )));

    // Render list items
    let (items, selected_idx): (Vec<String>, usize) = match &app.model_select {
        ModelSelectState::ChoosingProvider { providers, selected_idx } => {
            let list = providers.iter().map(|(_, display)| display.clone()).collect::<Vec<_>>();
            (list, *selected_idx)
        }
        ModelSelectState::FetchingModels { .. } => {
            // Show loading placeholder
            (vec!["  Loading available models...".to_string()], 0)
        }
        ModelSelectState::ChoosingModel { models, selected_idx, .. } => {
            (models.clone(), *selected_idx)
        }
        _ => (Vec::new(), 0),
    };

    // Calculate vertical scroll/slice for large lists (we have 4 lines for items in a 9-line menu block)
    let display_limit = 4;
    let start_idx = if selected_idx >= display_limit {
        selected_idx - display_limit + 1
    } else {
        0
    };
    let end_idx = (start_idx + display_limit).min(items.len());

    for i in start_idx..end_idx {
        let is_selected = selected_idx == i;
        if is_selected {
            lines.push(Line::from(vec![
                Span::styled("> ", Style::default().fg(theme::RED_ORANGE).add_modifier(Modifier::BOLD).bg(menu_bg)),
                Span::styled(
                    items[i].clone(),
                    Style::default().fg(theme::RED_ORANGE).add_modifier(Modifier::BOLD).bg(menu_bg),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default().bg(menu_bg)),
                Span::styled(items[i].clone(), Style::default().fg(Color::White).bg(menu_bg)),
            ]));
        }
    }

    // Add scroll down helper if there are more items
    if end_idx < items.len() {
        lines.push(Line::from(Span::styled(
            format!("  ↓ {} more", items.len() - end_idx),
            Style::default().fg(theme::AURA_SLATE).bg(menu_bg),
        )));
    } else {
        lines.push(Line::from(Span::styled("", Style::default().bg(menu_bg))));
    }

    // Help instructions line
    lines.push(Line::from(Span::styled(
        "  ↑/↓ Navigate · enter Select · esc to cancel",
        Style::default().fg(theme::AURA_SLATE).bg(menu_bg),
    )));

    // Fill remaining lines to area height with background to avoid background bleed
    let current_len = lines.len();
    for _ in current_len..(area.height as usize) {
        lines.push(Line::from(Span::styled(
            " ".repeat(area.width as usize),
            Style::default().bg(menu_bg),
        )));
    }

    let p = Paragraph::new(Text::from(lines));
    f.render_widget(p, area);
}
