use super::theme::Theme;
pub(crate) use crate::agent::style::markdown::{
    compute_table_column_widths, is_divider_row, is_table_row, split_row, text_display_width,
    tokenize_inline_markdown, wrap_display_text, InlineSegment,
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

// ── Markdown Table Parsing and Rendering ─────────────────────────────────────

pub(crate) fn clean_cell_text(text: &str) -> String {
    crate::agent::style::markdown::clean_cell_text(text, false)
}

#[allow(dead_code)]
pub(crate) fn wrap_cell_text(text: &str, max_width: usize) -> Vec<String> {
    wrap_display_text(text, max_width)
}

/// Renders a block of markdown table lines into styled Ratatui Lines with box dividers.
pub(crate) fn render_markdown_table_to_lines(
    table_lines: &[&str],
    theme: &Theme,
    available_width: usize,
) -> Vec<Line<'static>> {
    match table_lines.len() < 2 {
        true => {
            return table_lines
                .iter()
                .map(|l| Line::from(markdown_line_to_spans(l, theme)))
                .collect();
        }
        false => {}
    }

    let headers: Vec<String> = split_row(table_lines[0])
        .into_iter()
        .map(|h| clean_cell_text(&h))
        .collect();
    let num_cols = headers.len();
    match num_cols == 0 {
        true => {
            return table_lines
                .iter()
                .map(|l| Line::from(markdown_line_to_spans(l, theme)))
                .collect();
        }
        false => {}
    }

    let mut data_rows = Vec::new();
    for &line in &table_lines[2..] {
        let mut cells: Vec<String> = split_row(line)
            .into_iter()
            .map(|c| clean_cell_text(&c))
            .collect();
        while cells.len() < num_cols {
            cells.push(String::new());
        }
        cells.truncate(num_cols);
        data_rows.push(cells);
    }

    let mut max_content_widths = vec![0; num_cols];
    for col in 0..num_cols {
        let mut max_w = text_display_width(&headers[col]);
        for row in &data_rows {
            max_w = max_w.max(text_display_width(&row[col]));
        }
        max_content_widths[col] = max_w.max(3);
    }

    let col_widths = compute_table_column_widths(&max_content_widths, available_width, 4);
    let mut result_lines = Vec::new();

    // 1. Header rows
    let mut header_cell_lines = Vec::new();
    let mut max_header_lines = 1;
    for col in 0..num_cols {
        let lines = wrap_display_text(&headers[col], col_widths[col]);
        max_header_lines = max_header_lines.max(lines.len());
        header_cell_lines.push(lines);
    }

    let last_col = num_cols.saturating_sub(1);
    for line_idx in 0..max_header_lines {
        let mut spans = vec![Span::raw("  ")];
        for col in 0..num_cols {
            let text = header_cell_lines[col]
                .get(line_idx)
                .cloned()
                .unwrap_or_default();
            let visible_w = text_display_width(&text);
            let padding_len = col_widths[col].saturating_sub(visible_w);

            let mut cell_spans = parse_inline_markdown(&text, theme);
            for s in &mut cell_spans {
                s.style = s.style.fg(theme.brand_white).add_modifier(Modifier::BOLD);
            }
            spans.extend(cell_spans);

            match padding_len > 0 {
                true => spans.push(Span::raw(" ".repeat(padding_len))),
                false => {}
            }

            match col < last_col {
                true => spans.push(Span::styled(" │ ", Style::default().fg(theme.border))),
                false => {}
            }
        }
        result_lines.push(Line::from(spans));
    }

    // 2. Divider row
    let mut divider_spans = vec![Span::raw("  ")];
    for (col, &dash_count) in col_widths.iter().enumerate().take(num_cols) {
        divider_spans.push(Span::styled(
            "─".repeat(dash_count),
            Style::default().fg(theme.border),
        ));
        match col < last_col {
            true => divider_spans.push(Span::styled("─┼─", Style::default().fg(theme.border))),
            false => {}
        }
    }
    result_lines.push(Line::from(divider_spans));

    // 3. Data rows
    for row in data_rows {
        let mut cell_lines = Vec::new();
        let mut max_lines = 1;
        for col in 0..num_cols {
            let lines = wrap_display_text(&row[col], col_widths[col]);
            max_lines = max_lines.max(lines.len());
            cell_lines.push(lines);
        }

        for line_idx in 0..max_lines {
            let mut spans = vec![Span::raw("  ")];
            for col in 0..num_cols {
                let text = cell_lines[col].get(line_idx).cloned().unwrap_or_default();
                let visible_w = text_display_width(&text);
                let padding_len = col_widths[col].saturating_sub(visible_w);

                let cell_spans = parse_inline_markdown(&text, theme);
                spans.extend(cell_spans);

                match padding_len > 0 {
                    true => spans.push(Span::raw(" ".repeat(padding_len))),
                    false => {}
                }

                match col < last_col {
                    true => spans.push(Span::styled(" │ ", Style::default().fg(theme.border))),
                    false => {}
                }
            }
            result_lines.push(Line::from(spans));
        }
    }

    result_lines
}

// ── Line-Level Markdown Parser ──────────────────────────────────────────────

enum RatatuiLineKind<'a> {
    Heading4(&'a str),
    Heading3(&'a str),
    Heading2(&'a str),
    Heading1(&'a str),
    HorizontalRule,
    Blockquote(&'a str),
    TaskDone(&'a str),
    TaskTodo(&'a str),
    Bullet { indent: String, rest: &'a str },
    SubBullet(&'a str),
    Numbered { indent: String, num: &'a str, rest: &'a str },
    Paragraph(&'a str),
}

fn classify_ratatui_line(line: &str) -> RatatuiLineKind<'_> {
    let trimmed = line.trim_start();
    match trimmed.strip_prefix("#### ") {
        Some(rest) => return RatatuiLineKind::Heading4(rest),
        None => {}
    }
    match trimmed.strip_prefix("### ") {
        Some(rest) => return RatatuiLineKind::Heading3(rest),
        None => {}
    }
    match trimmed.strip_prefix("## ") {
        Some(rest) => return RatatuiLineKind::Heading2(rest),
        None => {}
    }
    match trimmed.strip_prefix("# ") {
        Some(rest) => return RatatuiLineKind::Heading1(rest),
        None => {}
    }
    match trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-') {
        true => return RatatuiLineKind::HorizontalRule,
        false => {}
    }
    match trimmed.strip_prefix("> ") {
        Some(rest) => return RatatuiLineKind::Blockquote(rest),
        None => {}
    }
    match trimmed
        .strip_prefix("- [x] ")
        .or_else(|| trimmed.strip_prefix("* [x] "))
    {
        Some(rest) => return RatatuiLineKind::TaskDone(rest),
        None => {}
    }
    match trimmed
        .strip_prefix("- [ ] ")
        .or_else(|| trimmed.strip_prefix("* [ ] "))
    {
        Some(rest) => return RatatuiLineKind::TaskTodo(rest),
        None => {}
    }
    match line
        .strip_prefix("  - ")
        .or_else(|| line.strip_prefix("  * "))
        .or_else(|| line.strip_prefix("  • "))
        .or_else(|| line.strip_prefix("  · "))
    {
        Some(rest) => return RatatuiLineKind::SubBullet(rest),
        None => {}
    }
    match trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("• "))
        .or_else(|| trimmed.strip_prefix("· "))
    {
        Some(rest) => {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            return RatatuiLineKind::Bullet { indent, rest };
        }
        None => {}
    }
    match trimmed.find(". ") {
        Some(pos) => match pos > 0 && pos <= 3 && trimmed[..pos].chars().all(|c| c.is_ascii_digit())
        {
            true => {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                return RatatuiLineKind::Numbered {
                    indent,
                    num: &trimmed[..pos],
                    rest: &trimmed[pos + 2..],
                };
            }
            false => {}
        },
        None => {}
    }
    RatatuiLineKind::Paragraph(line)
}

/// Convert a single line of markdown text into styled ratatui Spans.
pub(crate) fn markdown_line_to_spans(line: &str, theme: &Theme) -> Vec<Span<'static>> {
    match classify_ratatui_line(line) {
        RatatuiLineKind::Heading4(rest) => vec![
            Span::styled(
                "#### ",
                Style::default()
                    .fg(theme.info)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        RatatuiLineKind::Heading3(rest) => vec![
            Span::styled(
                "### ",
                Style::default()
                    .fg(theme.highlight)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.brand_white)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        RatatuiLineKind::Heading2(rest) => vec![
            Span::styled(
                "## ",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.brand_white)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        RatatuiLineKind::Heading1(rest) => vec![
            Span::styled(
                "# ",
                Style::default()
                    .fg(theme.brand_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(theme.brand_white)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        RatatuiLineKind::HorizontalRule => vec![Span::styled(
            "────────────────────────────────────────────────────────────".to_string(),
            Style::default().fg(theme.border),
        )],
        RatatuiLineKind::Blockquote(rest) => {
            let mut spans = vec![Span::styled("▎ ", Style::default().fg(theme.brand_accent))];
            let mut inner = parse_inline_markdown(rest, theme);
            for s in &mut inner {
                s.style = s.style.add_modifier(Modifier::ITALIC);
            }
            spans.extend(inner);
            spans
        }
        RatatuiLineKind::TaskDone(rest) => {
            let mut spans = vec![Span::styled(
                "✓ ",
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            )];
            spans.extend(parse_inline_markdown(rest, theme));
            spans
        }
        RatatuiLineKind::TaskTodo(rest) => {
            let mut spans = vec![Span::styled("☐ ", Style::default().fg(theme.muted))];
            spans.extend(parse_inline_markdown(rest, theme));
            spans
        }
        RatatuiLineKind::SubBullet(rest) => {
            let mut spans = vec![Span::styled("    • ", Style::default().fg(theme.muted))];
            spans.extend(parse_inline_markdown(rest, theme));
            spans
        }
        RatatuiLineKind::Bullet { indent, rest } => {
            let mut spans = vec![
                Span::raw(indent),
                Span::styled("• ", Style::default().fg(theme.brand_accent)),
            ];
            spans.extend(parse_inline_markdown(rest, theme));
            spans
        }
        RatatuiLineKind::Numbered { indent, num, rest } => {
            let mut spans = vec![
                Span::raw(indent),
                Span::styled(
                    format!("{}. ", num),
                    Style::default().fg(theme.brand_accent),
                ),
            ];
            spans.extend(parse_inline_markdown(rest, theme));
            spans
        }
        RatatuiLineKind::Paragraph(raw) => parse_inline_markdown(raw, theme),
    }
}

// ── Inline Markdown Parsing (Zero Regex) ────────────────────────────────────

/// Parse inline markdown (**bold**, `code`, *italic*, and status icons) into styled spans.
pub(crate) fn parse_inline_markdown(text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for segment in tokenize_inline_markdown(text) {
        match segment {
            InlineSegment::Plain(plain) => push_text_with_icons(&mut spans, &plain, theme),
            InlineSegment::Bold(inner) => spans.push(Span::styled(
                inner.to_string(),
                Style::default()
                    .fg(theme.brand_white)
                    .add_modifier(Modifier::BOLD),
            )),
            InlineSegment::Code(inner) => spans.push(Span::styled(
                format!(" {} ", inner),
                Style::default().fg(theme.success).bg(theme.bg_elevated),
            )),
            InlineSegment::Italic(inner) => spans.push(Span::styled(
                inner.to_string(),
                Style::default()
                    .fg(theme.info)
                    .add_modifier(Modifier::ITALIC),
            )),
        }
    }

    match spans.is_empty() {
        true => spans.push(Span::styled(
            text.to_string(),
            Style::default().fg(theme.text_primary),
        )),
        false => {}
    }

    spans
}

fn flush_plain_span(spans: &mut Vec<Span<'static>>, cur: &mut String, theme: &Theme) {
    match cur.is_empty() {
        true => {}
        false => spans.push(Span::styled(
            std::mem::take(cur),
            Style::default().fg(theme.text_primary),
        )),
    }
}

/// Helper that splits text on common status glyphs so they receive vivid Aura colors.
fn push_text_with_icons(spans: &mut Vec<Span<'static>>, text: &str, theme: &Theme) {
    let mut cur = String::new();
    for c in text.chars() {
        match c {
            '✔' | '✅' | '✓' => {
                flush_plain_span(spans, &mut cur, theme);
                spans.push(Span::styled(
                    "✓",
                    Style::default()
                        .fg(theme.success)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            '✖' | '❌' | '✗' => {
                flush_plain_span(spans, &mut cur, theme);
                spans.push(Span::styled(
                    "✗",
                    Style::default()
                        .fg(theme.destructive)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            '⚠' => {
                flush_plain_span(spans, &mut cur, theme);
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default()
                        .fg(theme.warning)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            '⚡' => {
                flush_plain_span(spans, &mut cur, theme);
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default()
                        .fg(theme.brand_accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            _ => cur.push(c),
        }
    }
    flush_plain_span(spans, &mut cur, theme);
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
