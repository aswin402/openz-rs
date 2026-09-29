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

    let spans_h4 = markdown_line_to_spans("#### Heading 4", &theme);
    assert_eq!(spans_h4.len(), 2);
    assert_eq!(spans_h4[0].content, "#### ");
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

#[test]
fn test_markdown_blockquotes_and_task_lists() {
    let theme = super::super::theme::Theme::default_theme();
    let spans_quote = markdown_line_to_spans("> Important quote block", &theme);
    assert_eq!(spans_quote[0].content, "▎ ");

    let spans_done = markdown_line_to_spans("- [x] Completed task", &theme);
    assert_eq!(spans_done[0].content, "✔ ");

    let spans_todo = markdown_line_to_spans("- [ ] Pending task", &theme);
    assert_eq!(spans_todo[0].content, "☐ ");
}

#[test]
fn test_markdown_table_detection_and_rendering() {
    let theme = super::super::theme::Theme::default_theme();
    let table = [
        "| Feature | CLI TUI | Ratatui |",
        "|---|---|---|",
        "| Tables | ✔ | ✔ |",
        "| Performance | Fast | Native |",
    ];

    assert!(is_table_row(table[0]));
    assert!(is_divider_row(table[1]));
    assert!(is_table_row(table[2]));

    let rendered = render_markdown_table_to_lines(&table, &theme, 80);
    // 1 header line + 1 divider line + 2 data lines = 4 lines
    assert_eq!(rendered.len(), 4);

    let header_text: String = rendered[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(header_text.contains("Feature") && header_text.contains("CLI TUI") && header_text.contains("Ratatui"));

    let divider_text: String = rendered[1].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(divider_text.contains('┼') && divider_text.contains('─'));

    let row1_text: String = rendered[2].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(row1_text.contains("Tables") && row1_text.contains('│') && row1_text.contains('✔'));
}

#[test]
fn test_markdown_cell_split_and_clean() {
    let cells = split_row("| Col A | Col B \\| Escaped |");
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[0], "Col A");
    assert_eq!(cells[1], "Col B | Escaped");

    let cleaned = clean_cell_text(" | **Cell Title** | ");
    assert_eq!(cleaned, "**Cell Title**");
}
