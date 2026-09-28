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

#[test]
fn test_format_markdown_line_bold_and_code() {
    let formatted = format_markdown_line("**hello** `world`");
    assert!(formatted.contains("hello"));
    assert!(formatted.contains("world"));
    assert!(!formatted.contains("**"));
    assert!(!formatted.contains('`'));
}

#[test]
fn test_format_markdown_line_heading() {
    let formatted = format_markdown_line("# OpenZ Engine");
    assert!(formatted.contains("OpenZ Engine"));
    assert!(formatted.contains("\x1b[38;2;135;206;250m") || formatted.contains("\x1b["));
}

#[test]
fn test_format_markdown_line_horizontal_rule() {
    let formatted = format_markdown_line("---");
    assert!(formatted.contains("──────"));
}

#[test]
fn test_render_table_lines() {
    let table = ["| Name | Role |", "|---|---|", "| OpenZ | Agent |"];
    let lines = render_table_lines(&table);
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("Name") && lines[0].contains("Role"));
    assert!(lines[1].contains('┼') && lines[1].contains('─'));
    assert!(lines[2].contains("OpenZ") && lines[2].contains("Agent") && lines[2].contains('│'));
}

#[test]
fn test_streaming_markdown_renderer_normal_lines() {
    let mut streamer = StreamingMarkdownRenderer::new_with_capture();
    streamer.push_chunk("Line 1\n");
    streamer.push_chunk("Line 2 with **bold**\n");
    streamer.finish();

    let captured = streamer.captured();
    assert_eq!(captured.len(), 2);
    assert!(captured[0].contains("Line 1"));
    assert!(captured[1].contains("Line 2 with ") && captured[1].contains("bold"));
    assert!(!captured[1].contains("**"));
}

#[test]
fn test_streaming_markdown_renderer_table() {
    let mut streamer = StreamingMarkdownRenderer::new_with_capture();
    streamer.push_chunk("Here is comparison:\n\n");
    streamer.push_chunk("**OpenZ** | **Hermes Agent**\n");
    streamer.push_chunk("---|---\n");
    streamer.push_chunk("Rust | Python\n");
    streamer.push_chunk("260 native tools | 70+ tools\n\n");
    streamer.push_chunk("Summary line\n");
    streamer.finish();

    let captured = streamer.captured();
    // Intro line, empty line, header, divider, 2 data rows, empty line, summary line
    assert!(captured.iter().any(|l| l.contains("Here is comparison:")));
    assert!(captured.iter().any(|l| l.contains('┼')));
    assert!(captured.iter().any(|l| l.contains('│') && l.contains("Rust")));
    assert!(captured.iter().any(|l| l.contains('│') && l.contains("260 native tools")));
    assert!(captured.iter().any(|l| l.contains("Summary line")));
}

#[test]
fn test_streaming_markdown_renderer_table_finish_without_trailing_newline() {
    let mut streamer = StreamingMarkdownRenderer::new_with_capture();
    streamer.push_chunk("| Col A | Col B |\n|---|---|\n| val1 | val2");
    streamer.finish();

    let captured = streamer.captured();
    assert!(captured.iter().any(|l| l.contains('┼')));
    assert!(captured.iter().any(|l| l.contains("val1") && l.contains("val2")));
}

#[test]
fn test_streaming_markdown_renderer_pipe_not_table() {
    let mut streamer = StreamingMarkdownRenderer::new_with_capture();
    streamer.push_chunk("Option A | Option B\n");
    streamer.push_chunk("Next sentence without divider\n");
    streamer.finish();

    let captured = streamer.captured();
    assert_eq!(captured.len(), 2);
    assert!(captured[0].contains("Option A | Option B"));
    assert!(captured[1].contains("Next sentence without divider"));
    // Ensure no table divider was generated
    assert!(!captured.iter().any(|l| l.contains('┼')));
}

#[test]
fn test_streaming_markdown_renderer_code_block() {
    let mut streamer = StreamingMarkdownRenderer::new_with_capture();
    streamer.push_chunk("```rust\n");
    streamer.push_chunk("let x = a | b;\n");
    streamer.push_chunk("```\n");
    streamer.finish();

    let captured = streamer.captured();
    assert_eq!(captured.len(), 3);
    assert!(captured[0].contains("```rust"));
    assert!(captured[1].contains("let x = a | b;"));
    assert!(captured[2].contains("```"));
    assert!(!captured.iter().any(|l| l.contains('┼')));
}
