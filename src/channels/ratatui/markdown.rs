use super::theme::Theme;
use ratatui::{
    style::{Modifier, Style},
    text::Span,
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
pub(crate) fn markdown_line_to_spans(line: &str, theme: &Theme) -> Vec<Span<'static>> {
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

/// Parse inline markdown (**bold** and `code`) into styled spans.
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
                            Style::default().fg(theme.success).bg(theme.bg_elevated),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_line_to_spans_headings() {
        let theme = super::super::theme::Theme::default_theme();
        let spans_h1 = markdown_line_to_spans("# Heading 1", &theme);
        assert_eq!(spans_h1.len(), 2);
        assert_eq!(spans_h1[0].content, "# ");
        assert_eq!(spans_h1[1].content, "Heading 1");

        let spans_h2 = markdown_line_to_spans("## Heading 2", &theme);
        assert_eq!(spans_h2.len(), 2);
        assert_eq!(spans_h2[0].content, "## ");

        let spans_h3 = markdown_line_to_spans("### Heading 3", &theme);
        assert_eq!(spans_h3.len(), 2);
        assert_eq!(spans_h3[0].content, "### ");
    }

    #[test]
    fn test_markdown_line_to_spans_inline_bold_and_code() {
        let theme = super::super::theme::Theme::default_theme();
        let spans = markdown_line_to_spans("This is **bold** and `code` here", &theme);
        assert!(spans.iter().any(|s| s.content == "bold"));
        assert!(spans.iter().any(|s| s.content == " code "));
    }

    #[test]
    fn test_markdown_line_to_spans_list_items() {
        let theme = super::super::theme::Theme::default_theme();
        let spans_bullet = markdown_line_to_spans("- item one", &theme);
        assert_eq!(spans_bullet[1].content, "• ");
        assert_eq!(spans_bullet[2].content, "item one");

        let spans_numbered = markdown_line_to_spans("1. first step", &theme);
        assert_eq!(spans_numbered[1].content, "1. ");
        assert_eq!(spans_numbered[2].content, "first step");
    }
}

