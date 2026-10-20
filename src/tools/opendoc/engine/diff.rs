use crate::tools::opendoc::ir::Document;

fn compute_lcs_dp(a: &[&str], b: &[&str]) -> Vec<Vec<usize>> {
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            dp[i][j] = match a[i - 1] == b[j - 1] {
                true => dp[i - 1][j - 1] + 1,
                false => dp[i - 1][j].max(dp[i][j - 1]),
            };
        }
    }
    dp
}

/// Compare two documents and return a structured diff (simple line-based)
pub fn diff_documents(a: &Document, b: &Document) -> DiffResult {
    let text_a: Vec<&str> = a.paragraphs.iter().map(|p| p.text.as_str()).collect();
    let text_b: Vec<&str> = b.paragraphs.iter().map(|p| p.text.as_str()).collect();

    let n = text_a.len();
    let m = text_b.len();
    let dp = compute_lcs_dp(&text_a, &text_b);

    let mut changes = Vec::new();
    let mut i = n;
    let mut j = m;

    while i > 0 || j > 0 {
        let same = i > 0 && j > 0 && text_a[i - 1] == text_b[j - 1];
        let take_b = !same && j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]);
        match (same, take_b) {
            (true, _) => {
                i -= 1;
                j -= 1;
            }
            (false, true) => {
                changes.push(DiffChange {
                    tag: "added".to_string(),
                    line: j - 1,
                    old_value: String::new(),
                    new_value: text_b[j - 1].to_string(),
                });
                j -= 1;
            }
            (false, false) => {
                changes.push(DiffChange {
                    tag: "removed".to_string(),
                    line: i - 1,
                    old_value: text_a[i - 1].to_string(),
                    new_value: String::new(),
                });
                i -= 1;
            }
        }
    }

    changes.reverse();
    changes.sort_by_key(|c| (c.line, c.tag == "added"));

    // Post-process to merge adjacent removal and addition at the same index into a modified change
    let mut merged_changes = Vec::new();
    let mut iter = changes.into_iter().peekable();
    while let Some(change) = iter.next() {
        let merge_with_next = change.tag == "removed"
            && iter
                .peek()
                .is_some_and(|next_change| next_change.tag == "added" && next_change.line == change.line);
        match merge_with_next {
            true => {
                let next_change = iter.next().unwrap();
                merged_changes.push(DiffChange {
                    tag: "modified".to_string(),
                    line: change.line,
                    old_value: change.old_value,
                    new_value: next_change.new_value,
                });
            }
            false => merged_changes.push(change),
        }
    }

    DiffResult {
        paragraphs_a: a.paragraphs.len(),
        paragraphs_b: b.paragraphs.len(),
        changes: merged_changes,
    }
}

/// Result of a diff comparison between two documents.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiffResult {
    pub paragraphs_a: usize,
    pub paragraphs_b: usize,
    pub changes: Vec<DiffChange>,
}

/// A single change entry in a diff (added, removed, or modified).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiffChange {
    pub tag: String,
    pub line: usize,
    pub old_value: String,
    pub new_value: String,
}

enum DiffOp {
    Unchanged(String),
    Removed(String),
    Added(String),
    Modified(String, String),
}

/// Word-level diff helper using LCS. Preserves exact spacing.
fn diff_words(old_val: &str, new_val: &str, is_html: bool) -> String {
    let words_a: Vec<&str> = old_val.split_inclusive(char::is_whitespace).collect();
    let words_b: Vec<&str> = new_val.split_inclusive(char::is_whitespace).collect();

    let n = words_a.len();
    let m = words_b.len();
    let dp = compute_lcs_dp(&words_a, &words_b);

    let mut result = Vec::new();
    let mut i = n;
    let mut j = m;

    while i > 0 || j > 0 {
        let same = i > 0 && j > 0 && words_a[i - 1] == words_b[j - 1];
        let take_b = !same && j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]);
        match (same, take_b) {
            (true, _) => {
                result.push(words_a[i - 1].to_string());
                i -= 1;
                j -= 1;
            }
            (false, true) => {
                let val = words_b[j - 1];
                let wrapped = match is_html {
                    true => format!(
                        "<ins style=\"background-color: #e6ffec; text-decoration: none; color: #1a7f37;\">{}</ins>",
                        val
                    ),
                    false => format!("++{}++", val),
                };
                result.push(wrapped);
                j -= 1;
            }
            (false, false) => {
                let val = words_a[i - 1];
                let wrapped = match is_html {
                    true => format!(
                        "<del style=\"background-color: #ffebe9; text-decoration: line-through; color: #cf222e;\">{}</del>",
                        val
                    ),
                    false => format!("~~{}~~", val),
                };
                result.push(wrapped);
                i -= 1;
            }
        }
    }

    result.reverse();
    result.join("")
}

/// Render a visual difference between two documents as styled HTML or Markdown.
pub fn render_diff_visual(a: &Document, b: &Document, is_html: bool) -> String {
    let text_a: Vec<&str> = a.paragraphs.iter().map(|p| p.text.as_str()).collect();
    let text_b: Vec<&str> = b.paragraphs.iter().map(|p| p.text.as_str()).collect();

    let n = text_a.len();
    let m = text_b.len();
    let dp = compute_lcs_dp(&text_a, &text_b);

    let mut ops = Vec::new();
    let mut i = n;
    let mut j = m;

    while i > 0 || j > 0 {
        let same = i > 0 && j > 0 && text_a[i - 1] == text_b[j - 1];
        let take_b = !same && j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]);
        match (same, take_b) {
            (true, _) => {
                ops.push(DiffOp::Unchanged(text_a[i - 1].to_string()));
                i -= 1;
                j -= 1;
            }
            (false, true) => {
                ops.push(DiffOp::Added(text_b[j - 1].to_string()));
                j -= 1;
            }
            (false, false) => {
                ops.push(DiffOp::Removed(text_a[i - 1].to_string()));
                i -= 1;
            }
        }
    }
    ops.reverse();

    let mut merged_ops = Vec::new();
    let mut iter = ops.into_iter().peekable();
    while let Some(op) = iter.next() {
        match (&op, iter.peek()) {
            (DiffOp::Removed(old_val), Some(DiffOp::Added(new_val))) => {
                let old_val = old_val.clone();
                let new_val = new_val.clone();
                let _ = iter.next();
                merged_ops.push(DiffOp::Modified(old_val, new_val));
            }
            _ => merged_ops.push(op),
        }
    }

    let mut rendered_lines = Vec::new();
    for op in merged_ops {
        match op {
            DiffOp::Unchanged(text) => match is_html {
                true => rendered_lines.push(format!(
                    "<div class=\"diff-line\" style=\"padding: 4px 8px;\">{}</div>",
                    text
                )),
                false => rendered_lines.push(text),
            },
            DiffOp::Added(text) => match is_html {
                true => rendered_lines.push(format!(
                    "<div class=\"diff-line diff-add\" style=\"background-color: #e6ffec; border-left: 4px solid #34d058; padding: 4px 8px; margin: 2px 0;\"><ins style=\"text-decoration: none; color: #1a7f37;\">{}</ins></div>",
                    text
                )),
                false => rendered_lines.push(format!("++{}++", text)),
            },
            DiffOp::Removed(text) => match is_html {
                true => rendered_lines.push(format!(
                    "<div class=\"diff-line diff-remove\" style=\"background-color: #ffebe9; border-left: 4px solid #d73a49; padding: 4px 8px; margin: 2px 0;\"><del style=\"text-decoration: line-through; color: #cf222e;\">{}</del></div>",
                    text
                )),
                false => rendered_lines.push(format!("~~{}~~", text)),
            },
            DiffOp::Modified(old, new) => {
                let diff_content = diff_words(&old, &new, is_html);
                match is_html {
                    true => rendered_lines.push(format!(
                        "<div class=\"diff-line diff-modify\" style=\"background-color: #fbf0dc; border-left: 4px solid #e3b341; padding: 4px 8px; margin: 2px 0;\">{}</div>",
                        diff_content
                    )),
                    false => rendered_lines.push(diff_content),
                }
            }
        }
    }

    match is_html {
        true => {
            let mut html = String::new();
            html.push_str("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>Document Comparison Diff</title>\n");
            html.push_str("<style>\n");
            html.push_str("body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif; line-height: 1.6; max-width: 900px; margin: 40px auto; padding: 0 20px; background-color: #f6f8fa; color: #24292f; }\n");
            html.push_str(".diff-container { border: 1px solid #d0d7de; border-radius: 6px; padding: 24px; background-color: #ffffff; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }\n");
            html.push_str("h1 { border-bottom: 1px solid #d0d7de; padding-bottom: 12px; font-size: 24px; margin-top: 0; }\n");
            html.push_str(".diff-summary { display: flex; gap: 20px; font-size: 14px; color: #57606a; margin-bottom: 20px; border-bottom: 1px solid #d0d7de; padding-bottom: 12px; }\n");
            html.push_str(".diff-line { white-space: pre-wrap; font-family: SFMono-Regular, Consolas, 'Liberation Mono', Menlo, monospace; font-size: 14px; line-height: 20px; }\n");
            html.push_str("</style>\n</head>\n<body>\n");
            html.push_str("<div class=\"diff-container\">\n");
            html.push_str("<h1>Document Diff Comparison</h1>\n");
            html.push_str(&format!(
                "<div class=\"diff-summary\"><span>Original: {} paragraphs</span> <span>Modified: {} paragraphs</span></div>\n",
                n, m
            ));

            for line in rendered_lines {
                html.push_str(&line);
                html.push('\n');
            }

            html.push_str("</div>\n</body>\n</html>");
            html
        }
        false => rendered_lines.join("\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::opendoc::ir::Paragraph;

    fn make_doc(texts: &[&str]) -> Document {
        let mut doc = Document::new("txt");
        for t in texts {
            doc.paragraphs.push(Paragraph::new(*t));
        }
        doc
    }

    #[test]
    fn test_diff_identical() {
        let a = make_doc(&["Hello", "World"]);
        let b = make_doc(&["Hello", "World"]);
        let result = diff_documents(&a, &b);
        assert_eq!(result.paragraphs_a, 2);
        assert_eq!(result.paragraphs_b, 2);
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_diff_modified() {
        let a = make_doc(&["Hello", "World"]);
        let b = make_doc(&["Hello", "Rust"]);
        let result = diff_documents(&a, &b);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].tag, "modified");
        assert_eq!(result.changes[0].line, 1);
        assert_eq!(result.changes[0].old_value, "World");
        assert_eq!(result.changes[0].new_value, "Rust");
    }

    #[test]
    fn test_diff_added() {
        let a = make_doc(&["Hello"]);
        let b = make_doc(&["Hello", "World"]);
        let result = diff_documents(&a, &b);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].tag, "added");
        assert_eq!(result.changes[0].line, 1);
        assert_eq!(result.changes[0].new_value, "World");
    }

    #[test]
    fn test_diff_removed() {
        let a = make_doc(&["Hello", "World"]);
        let b = make_doc(&["Hello"]);
        let result = diff_documents(&a, &b);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].tag, "removed");
        assert_eq!(result.changes[0].line, 1);
        assert_eq!(result.changes[0].old_value, "World");
    }

    #[test]
    fn test_diff_empty_docs() {
        let a = make_doc(&[]);
        let b = make_doc(&[]);
        let result = diff_documents(&a, &b);
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_diff_multiple_changes() {
        let a = make_doc(&["A", "B", "C"]);
        let b = make_doc(&["A", "X", "Y"]);
        let result = diff_documents(&a, &b);
        assert_eq!(result.changes.len(), 2);
        assert_eq!(result.changes[0].old_value, "B");
        assert_eq!(result.changes[0].new_value, "X");
        assert_eq!(result.changes[1].old_value, "C");
        assert_eq!(result.changes[1].new_value, "Y");
    }

    #[test]
    fn test_render_diff_visual_markdown() {
        let a = make_doc(&["Hello World", "This is old line"]);
        let b = make_doc(&["Hello World", "This is new line"]);

        let md_diff = render_diff_visual(&a, &b, false);
        assert!(md_diff.contains("Hello World"));
        // Word level diff inside "This is old line" -> "This is new line"
        assert!(md_diff.contains("This is ~~old ~~++new ++line"));
    }

    #[test]
    fn test_render_diff_visual_html() {
        let a = make_doc(&["Hello World", "Removed paragraph"]);
        let b = make_doc(&["Hello World", "Added paragraph"]);

        let html_diff = render_diff_visual(&a, &b, true);
        assert!(html_diff.contains("<!DOCTYPE html>"));
        assert!(html_diff.contains("Document Comparison Diff"));
        assert!(html_diff.contains("diff-modify"));
    }
}
