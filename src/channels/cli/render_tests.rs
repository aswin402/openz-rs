use super::*;

#[test]
fn test_multiline_prompt_wrapping() {
    let text = "012345678901234567890123456789";
    let width: usize = 12;
    let prompt_prefix_width: usize = 2;
    let max_line_content_width = width.saturating_sub(prompt_prefix_width).max(1);

    let display_chars: Vec<char> = text.chars().collect();
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0;

    for &c in &display_chars {
        let cw = char_display_width(c);
        if cur_w + cw > max_line_content_width && !cur.is_empty() {
            lines.push(cur);
            cur = String::new();
            cur_w = 0;
        }
        cur.push(c);
        cur_w += cw;
    }
    lines.push(cur);

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "0123456789");
    assert_eq!(lines[1], "0123456789");
    assert_eq!(lines[2], "0123456789");
}

#[test]
fn test_is_table_row() {
    assert!(is_table_row("| A | B |"));
    assert!(!is_table_row("Not a table row"));
    assert!(!is_table_row("|"));
}

#[test]
fn test_is_divider_row() {
    assert!(is_divider_row("|---|---|"));
    assert!(is_divider_row("|:---|---:|"));
    assert!(!is_divider_row("| A | B |"));
}

#[test]
fn test_split_row() {
    let cells = split_row("| A | B |");
    assert_eq!(cells, vec!["A", "B"]);

    let cells_escaped = split_row("| A\\|B | C |");
    assert_eq!(cells_escaped, vec!["A|B", "C"]);

    let cells_no_outer = split_row("A | B");
    assert_eq!(cells_no_outer, vec!["A", "B"]);
}

#[test]
fn test_wrap_text() {
    let lines = wrap_text("hello world", 7);
    assert_eq!(lines, vec!["hello", "world"]);
}

#[test]
fn test_horizontal_rule_detection() {
    let line1 = "---";
    let line2 = "----";
    let line3 = "  ---  ";
    let line4 = "--";
    let line5 = "-a-";

    let is_hr = |l: &str| {
        let trimmed = l.trim();
        trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3 && !trimmed.is_empty()
    };

    assert!(is_hr(line1));
    assert!(is_hr(line2));
    assert!(is_hr(line3));
    assert!(!is_hr(line4));
    assert!(!is_hr(line5));
}
