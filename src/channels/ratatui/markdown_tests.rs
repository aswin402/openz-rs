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
