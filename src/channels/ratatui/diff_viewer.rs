use super::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Formatter for producing syntax-highlighted, colored unified diff lines in Ratatui.
pub struct DiffViewer;

impl DiffViewer {
    /// Generates colored `Line` elements for a unified diff between old and new text.
    pub fn render_diff(
        old_text: &str,
        new_text: &str,
        theme: &Theme,
        max_lines: usize,
    ) -> Vec<Line<'static>> {
        let patch = diffy::create_patch(old_text, new_text);
        let patch_str = patch.to_string();
        Self::render_patch_str(&patch_str, theme, max_lines)
    }

    /// Renders an existing unified patch / diff string into colored Ratatui Lines
    pub fn render_patch_str(
        patch_str: &str,
        theme: &Theme,
        max_lines: usize,
    ) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        for raw_line in patch_str.lines() {
            if lines.len() >= max_lines {
                lines.push(Line::from(vec![Span::styled(
                    "  ... (diff truncated, more lines below)",
                    Style::default().fg(theme.muted),
                )]));
                break;
            }

            if raw_line.starts_with("---") || raw_line.starts_with("+++") {
                lines.push(Line::from(vec![Span::styled(
                    raw_line.to_string(),
                    Style::default()
                        .fg(theme.muted)
                        .add_modifier(Modifier::BOLD),
                )]));
            } else if raw_line.starts_with("@@") {
                lines.push(Line::from(vec![
                    Span::styled(
                        "┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈ ",
                        Style::default().fg(theme.border),
                    ),
                    Span::styled(
                        raw_line.to_string(),
                        Style::default()
                            .fg(theme.info)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else if let Some(rest) = raw_line.strip_prefix('+') {
                lines.push(Line::from(vec![
                    Span::styled(
                        "+ ",
                        Style::default()
                            .fg(theme.success)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        rest.to_string(),
                        Style::default()
                            .fg(theme.success)
                            .bg(theme.bg_elevated),
                    ),
                ]));
            } else if let Some(rest) = raw_line.strip_prefix('-') {
                lines.push(Line::from(vec![
                    Span::styled(
                        "- ",
                        Style::default()
                            .fg(theme.destructive)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        rest.to_string(),
                        Style::default()
                            .fg(theme.destructive)
                            .bg(theme.bg_elevated),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default().fg(theme.muted)),
                    Span::styled(
                        raw_line.strip_prefix(' ').unwrap_or(raw_line).to_string(),
                        Style::default().fg(theme.muted),
                    ),
                ]));
            }
        }

        if lines.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "  (no changes detected)",
                Style::default().fg(theme.muted),
            )]));
        }

        lines
    }
}
