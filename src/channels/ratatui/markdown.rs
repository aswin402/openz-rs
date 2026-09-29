use super::theme::Theme;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::sync::OnceLock;

// ── Width and Character Utilities ───────────────────────────────────────────

/// Measures character display width in terminal columns, accounting for emojis and wide glyphs.
pub(crate) fn char_display_width(c: char) -> usize {
    let cp = c as u32;
    if cp == 0xFE0F {
        0
    } else if (0x1F000..=0x1FBF9).contains(&cp)
        || c == '⬢'
        || c == '🗑'
        || c == '📊'
        || c == '✅'
        || c == '❌'
        || c == '⚠'
        || c == '⚡'
        || c == 'ℹ'
    {
        2
    } else {
        1
    }
}

/// Measures text display width in terminal columns, stripping inline markdown tokens.
pub(crate) fn text_display_width(text: &str) -> usize {
    let cleaned = text.replace("**", "").replace(['`', '*'], "");
    cleaned.chars().map(char_display_width).sum()
}

// ── Regex Helpers for Inline Markdown ───────────────────────────────────────

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

fn re_italic() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\*([^*\n]+)\*").ok())
        .as_ref()
}

// ── Markdown Table Parsing and Rendering ─────────────────────────────────────

pub(crate) fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && !is_divider_row(line)
}

pub(crate) fn is_divider_row(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.contains('|') {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
}

pub(crate) fn split_row(line: &str) -> Vec<String> {
    let mut trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if trimmed.starts_with('|') {
        trimmed = trimmed[1..].trim();
    }
    if trimmed.ends_with('|') {
        trimmed = trimmed[..trimmed.len() - 1].trim();
    }
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&'|') = chars.peek() {
                current.push('|');
                chars.next();
            } else {
                current.push('\\');
            }
        } else if c == '|' {
            cells.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(c);
        }
    }
    cells.push(current.trim().to_string());
    cells
}

pub(crate) fn clean_cell_text(text: &str) -> String {
    let mut cleaned = text.trim();
    while let Some(rest) = cleaned.strip_prefix('|') {
        cleaned = rest.trim();
    }
    while let Some(rest) = cleaned.strip_suffix('|') {
        cleaned = rest.trim();
    }
    cleaned.to_string()
}

pub(crate) fn wrap_cell_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut current_line = String::new();
        let mut current_width = 0;

        for word in paragraph.split_whitespace() {
            let word_width = text_display_width(word);

            if current_line.is_empty() {
                if word_width <= max_width {
                    current_line.push_str(word);
                    current_width = word_width;
                } else {
                    let mut w_chars = word.chars().peekable();
                    while w_chars.peek().is_some() {
                        let mut chunk = String::new();
                        let mut chunk_w = 0;
                        while let Some(&c) = w_chars.peek() {
                            let cw = char_display_width(c);
                            if chunk_w + cw > max_width && chunk_w > 0 {
                                break;
                            }
                            chunk.push(c);
                            chunk_w += cw;
                            w_chars.next();
                        }
                        lines.push(chunk);
                    }
                }
            } else {
                let space_width = 1;
                if current_width + space_width + word_width <= max_width {
                    current_line.push(' ');
                    current_line.push_str(word);
                    current_width += space_width + word_width;
                } else {
                    lines.push(current_line);
                    current_line = String::new();
                    current_width = 0;

                    if word_width <= max_width {
                        current_line.push_str(word);
                        current_width = word_width;
                    } else {
                        let mut w_chars = word.chars().peekable();
                        while w_chars.peek().is_some() {
                            let mut chunk = String::new();
                            let mut chunk_w = 0;
                            while let Some(&c) = w_chars.peek() {
                                let cw = char_display_width(c);
                                if chunk_w + cw > max_width && chunk_w > 0 {
                                    break;
                                }
                                chunk.push(c);
                                chunk_w += cw;
                                w_chars.next();
                            }
                            if w_chars.peek().is_some() {
                                lines.push(chunk);
                            } else {
                                current_line = chunk;
                                current_width = chunk_w;
                            }
                        }
                    }
                }
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        if lines.is_empty() && paragraph.is_empty() {
            lines.push(String::new());
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Renders a block of markdown table lines into styled Ratatui Lines with box dividers.
pub(crate) fn render_markdown_table_to_lines(
    table_lines: &[&str],
    theme: &Theme,
    available_width: usize,
) -> Vec<Line<'static>> {
    if table_lines.len() < 2 {
        return table_lines
            .iter()
            .map(|l| Line::from(markdown_line_to_spans(l, theme)))
            .collect();
    }

    let headers: Vec<String> = split_row(table_lines[0])
        .into_iter()
        .map(|h| clean_cell_text(&h))
        .collect();
    let num_cols = headers.len();
    if num_cols == 0 {
        return table_lines
            .iter()
            .map(|l| Line::from(markdown_line_to_spans(l, theme)))
            .collect();
    }

    let mut data_rows = Vec::new();
    for &line in &table_lines[2..] {
        let mut cells = split_row(line);
        while cells.len() < num_cols {
            cells.push(String::new());
        }
        cells.truncate(num_cols);
        data_rows.push(cells);
    }

    let separator_overhead = 3 * (num_cols - 1) + 4;
    let content_usable_width = available_width
        .saturating_sub(separator_overhead)
        .max(num_cols * 4);

    let mut max_content_widths = vec![0; num_cols];
    for col in 0..num_cols {
        let mut max_w = text_display_width(&headers[col]);
        for row in &data_rows {
            max_w = max_w.max(text_display_width(&row[col]));
        }
        max_content_widths[col] = max_w.max(3);
    }

    let total_content_width: usize = max_content_widths.iter().sum();
    let mut col_widths = max_content_widths.clone();

    if total_content_width > content_usable_width {
        let mut remaining_width = content_usable_width;
        let mut large_cols = Vec::new();

        for col in 0..num_cols {
            if max_content_widths[col] <= 15 {
                col_widths[col] = max_content_widths[col];
                remaining_width = remaining_width.saturating_sub(col_widths[col]);
            } else {
                large_cols.push(col);
            }
        }

        if !large_cols.is_empty() {
            let equal_share = remaining_width / large_cols.len();
            let mut extra = remaining_width % large_cols.len();
            for &col in &large_cols {
                let share = equal_share
                    + if extra > 0 {
                        extra -= 1;
                        1
                    } else {
                        0
                    };
                col_widths[col] = share.max(8);
            }
        }
    }

    let mut result_lines = Vec::new();

    // 1. Header rows
    let mut header_cell_lines = Vec::new();
    let mut max_header_lines = 1;
    for col in 0..num_cols {
        let lines = wrap_cell_text(&headers[col], col_widths[col]);
        max_header_lines = max_header_lines.max(lines.len());
        header_cell_lines.push(lines);
    }

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

            if padding_len > 0 {
                spans.push(Span::raw(" ".repeat(padding_len)));
            }

            if col < num_cols - 1 {
                spans.push(Span::styled(" │ ", Style::default().fg(theme.border)));
            }
        }
        result_lines.push(Line::from(spans));
    }

    // 2. Divider row
    let mut divider_spans = vec![Span::raw("  ")];
    for (col, &dash_count) in col_widths.iter().enumerate().take(num_cols) {
        divider_spans.push(Span::styled("─".repeat(dash_count), Style::default().fg(theme.border)));
        if col < num_cols - 1 {
            divider_spans.push(Span::styled("─┼─", Style::default().fg(theme.border)));
        }
    }
    result_lines.push(Line::from(divider_spans));

    // 3. Data rows
    for row in data_rows {
        let mut cell_lines = Vec::new();
        let mut max_lines = 1;
        for col in 0..num_cols {
            let lines = wrap_cell_text(&row[col], col_widths[col]);
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

                if padding_len > 0 {
                    spans.push(Span::raw(" ".repeat(padding_len)));
                }

                if col < num_cols - 1 {
                    spans.push(Span::styled(" │ ", Style::default().fg(theme.border)));
                }
            }
            result_lines.push(Line::from(spans));
        }
    }

    result_lines
}

// ── Line-Level Markdown Parser ──────────────────────────────────────────────

/// Convert a single line of markdown text into styled ratatui Spans.
pub(crate) fn markdown_line_to_spans(line: &str, theme: &Theme) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();

    // Heading detection
    if let Some(rest) = trimmed.strip_prefix("#### ") {
        return vec![
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
        ];
    }
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
                    .fg(theme.brand_white)
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
                    .fg(theme.brand_white)
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
                    .fg(theme.brand_white)
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

    // Blockquote detection: > or >>
    if let Some(rest) = trimmed.strip_prefix("> ") {
        let mut spans = vec![
            Span::styled("▎ ", Style::default().fg(theme.brand_accent)),
        ];
        let mut inner = parse_inline_markdown(rest, theme);
        for s in &mut inner {
            s.style = s.style.add_modifier(Modifier::ITALIC);
        }
        spans.extend(inner);
        return spans;
    }

    // Task lists: - [x] or - [ ]
    if let Some(rest) = trimmed.strip_prefix("- [x] ").or_else(|| trimmed.strip_prefix("* [x] ")) {
        let mut spans = vec![
            Span::styled("✔ ", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
        ];
        spans.extend(parse_inline_markdown(rest, theme));
        return spans;
    }
    if let Some(rest) = trimmed.strip_prefix("- [ ] ").or_else(|| trimmed.strip_prefix("* [ ] ")) {
        let mut spans = vec![
            Span::styled("☐ ", Style::default().fg(theme.muted)),
        ];
        spans.extend(parse_inline_markdown(rest, theme));
        return spans;
    }

    // List items — use classic crisp bullet (•)
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("• "))
        .or_else(|| trimmed.strip_prefix("· "))
    {
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        let mut spans = vec![
            Span::raw(indent),
            Span::styled("• ", Style::default().fg(theme.brand_accent)),
        ];
        spans.extend(parse_inline_markdown(rest, theme));
        return spans;
    }

    if let Some(rest) = trimmed
        .strip_prefix("  - ")
        .or_else(|| trimmed.strip_prefix("  * "))
        .or_else(|| trimmed.strip_prefix("  • "))
        .or_else(|| trimmed.strip_prefix("  · "))
    {
        let mut spans = vec![Span::styled("    • ", Style::default().fg(theme.muted))];
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
                Span::styled(
                    format!("{}. ", number),
                    Style::default().fg(theme.brand_accent),
                ),
            ];
            spans.extend(parse_inline_markdown(rest, theme));
            return spans;
        }
    }

    // Regular text with inline formatting
    parse_inline_markdown(line, theme)
}

// ── Inline Markdown Parsing ─────────────────────────────────────────────────

/// Parse inline markdown (**bold**, `code`, *italic*, and status icons) into styled spans.
pub(crate) fn parse_inline_markdown(text: &str, theme: &Theme) -> Vec<Span<'static>> {
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
        let italic_match = if bold_match.is_none() {
            re_italic().and_then(|re| re.find(&remaining).map(|m| (m.start(), m.end(), "italic")))
        } else {
            None
        };

        let mut earliest: Option<(usize, usize, &'static str)> = None;
        for candidate in [bold_match, code_match, italic_match].into_iter().flatten() {
            if earliest.is_none_or(|cur| candidate.0 < cur.0) {
                earliest = Some(candidate);
            }
        }

        match earliest {
            Some((start, end, kind)) => {
                if start > 0 {
                    push_text_with_icons(&mut spans, &remaining[..start], theme);
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
                            Style::default().fg(theme.success).bg(theme.bg_elevated),
                        ));
                    }
                    "italic" => {
                        let inner = &matched[1..matched.len() - 1];
                        spans.push(Span::styled(
                            inner.to_string(),
                            Style::default()
                                .fg(theme.info)
                                .add_modifier(Modifier::ITALIC),
                        ));
                    }
                    _ => {}
                }

                remaining = remaining[end..].to_string();
            }
            None => {
                push_text_with_icons(&mut spans, &remaining, theme);
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

/// Helper that splits text on common status glyphs so they receive vivid Aura colors.
fn push_text_with_icons(spans: &mut Vec<Span<'static>>, text: &str, theme: &Theme) {
    let mut cur = String::new();
    for c in text.chars() {
        match c {
            '✔' | '✅' | '✓' => {
                if !cur.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut cur),
                        Style::default().fg(theme.text_primary),
                    ));
                }
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
                ));
            }
            '✖' | '❌' | '✗' => {
                if !cur.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut cur),
                        Style::default().fg(theme.text_primary),
                    ));
                }
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default().fg(theme.destructive).add_modifier(Modifier::BOLD),
                ));
            }
            '⚠' => {
                if !cur.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut cur),
                        Style::default().fg(theme.text_primary),
                    ));
                }
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
                ));
            }
            '⚡' => {
                if !cur.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut cur),
                        Style::default().fg(theme.text_primary),
                    ));
                }
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default().fg(theme.brand_accent).add_modifier(Modifier::BOLD),
                ));
            }
            _ => {
                cur.push(c);
            }
        }
    }
    if !cur.is_empty() {
        spans.push(Span::styled(cur, Style::default().fg(theme.text_primary)));
    }
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
