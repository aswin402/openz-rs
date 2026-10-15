use super::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

enum DiffLineKind<'a> {
    FileHeader(&'a str),
    HunkHeader(&'a str),
    Added(&'a str),
    Removed(&'a str),
    Context(&'a str),
}

fn classify_diff_line(raw_line: &str) -> DiffLineKind<'_> {
    match raw_line.starts_with("---") || raw_line.starts_with("+++") {
        true => return DiffLineKind::FileHeader(raw_line),
        false => {}
    }
    match raw_line.starts_with("@@") {
        true => return DiffLineKind::HunkHeader(raw_line),
        false => {}
    }
    match raw_line.strip_prefix('+') {
        Some(rest) => return DiffLineKind::Added(rest),
        None => {}
    }
    match raw_line.strip_prefix('-') {
        Some(rest) => return DiffLineKind::Removed(rest),
        None => {}
    }
    DiffLineKind::Context(raw_line.strip_prefix(' ').unwrap_or(raw_line))
}

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
            match lines.len() >= max_lines {
                true => {
                    lines.push(Line::from(vec![Span::styled(
                        "  ... (diff truncated, more lines below)",
                        Style::default().fg(theme.muted),
                    )]));
                    break;
                }
                false => {}
            }

            let styled_line = match classify_diff_line(raw_line) {
                DiffLineKind::FileHeader(header) => Line::from(vec![Span::styled(
                    header.to_string(),
                    Style::default()
                        .fg(theme.muted)
                        .add_modifier(Modifier::BOLD),
                )]),
                DiffLineKind::HunkHeader(hunk) => Line::from(vec![
                    Span::styled(
                        "┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈ ",
                        Style::default().fg(theme.border),
                    ),
                    Span::styled(
                        hunk.to_string(),
                        Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                    ),
                ]),
                DiffLineKind::Added(rest) => Line::from(vec![
                    Span::styled(
                        "+ ",
                        Style::default()
                            .fg(theme.success)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        rest.to_string(),
                        Style::default().fg(theme.success).bg(theme.bg_elevated),
                    ),
                ]),
                DiffLineKind::Removed(rest) => Line::from(vec![
                    Span::styled(
                        "- ",
                        Style::default()
                            .fg(theme.destructive)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        rest.to_string(),
                        Style::default().fg(theme.destructive).bg(theme.bg_elevated),
                    ),
                ]),
                DiffLineKind::Context(ctx) => Line::from(vec![
                    Span::styled("  ", Style::default().fg(theme.muted)),
                    Span::styled(ctx.to_string(), Style::default().fg(theme.muted)),
                ]),
            };
            lines.push(styled_line);
        }

        match lines.is_empty() {
            true => lines.push(Line::from(vec![Span::styled(
                "  (no changes detected)",
                Style::default().fg(theme.muted),
            )])),
            false => {}
        }

        lines
    }
}
