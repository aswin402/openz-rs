use serde_json::Value;

pub const DEFAULT_JSON_COMPACT_CHARS: usize = 16_000;
pub const DEFAULT_LOG_COMPACT_CHARS: usize = 2_000;

pub fn compress_json(raw_json: &str) -> anyhow::Result<String> {
    compress_json_with_limit(raw_json, DEFAULT_JSON_COMPACT_CHARS)
}

pub fn compress_json_with_limit(raw_json: &str, max_chars: usize) -> anyhow::Result<String> {
    let effective_max = max_chars.max(DEFAULT_LOG_COMPACT_CHARS);
    let half_max = (effective_max / 2).max(1);
    let value: Value = serde_json::from_str(raw_json)?;
    match value {
        Value::Array(arr) => match arr.len() {
            0 => Ok("[]".to_string()),
            1..=5 => Ok(serde_json::to_string_pretty(&arr)?),
            total_count => {
                let keys: std::collections::BTreeSet<String> = arr
                    .iter()
                    .filter_map(Value::as_object)
                    .flat_map(|map| map.keys().cloned())
                    .collect();
                let keys_str = keys.into_iter().collect::<Vec<String>>().join(", ");
                let first_items: Vec<_> = arr.iter().take(2).collect();
                let first_str = serde_json::to_string_pretty(&first_items).unwrap_or_default();
                let last_str = arr
                    .last()
                    .and_then(|item| serde_json::to_string_pretty(item).ok())
                    .unwrap_or_default();

                Ok(format!(
                    "[JSON Array: {} objects. Common keys: [{}].\nFirst 2 items:\n{}\n\n... [{} intermediate items omitted; full payload saved to disk] ...\n\nLast item:\n{}]",
                    total_count,
                    keys_str,
                    first_str,
                    total_count.saturating_sub(3),
                    last_str
                ))
            }
        },
        other => {
            let pretty = serde_json::to_string_pretty(&other)?;
            let char_count = pretty.chars().count();
            match char_count > effective_max {
                false => Ok(pretty),
                true => {
                    let first_part: String = pretty.chars().take(half_max).collect();
                    let last_part: String = pretty
                        .chars()
                        .skip(char_count.saturating_sub(half_max))
                        .collect();
                    Ok(format!(
                        "{}\n\n... [TRUNCATED INTERMEDIATE JSON OBJECT; full payload saved to disk] ...\n\n{}",
                        first_part, last_part
                    ))
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CodeLexState {
    Normal,
    SeenSlash,
    InLineComment,
    InBlockComment,
    InBlockCommentSeenStar,
    InDoubleQuote,
    InDoubleQuoteEscape,
}

pub(crate) fn is_hashline_anchored_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed
        .split_once('|')
        .and_then(|(prefix, _)| prefix.split_once(':'))
        .is_some_and(|(line_num, hash_hex)| {
            !line_num.is_empty()
                && line_num.chars().all(|c| c.is_ascii_digit())
                && hash_hex.len() == 4
                && hash_hex.chars().all(|c| c.is_ascii_hexdigit())
        })
}

pub(crate) fn has_hashline_anchors(raw_code: &str) -> bool {
    let mut sample = raw_code
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(5);
    sample
        .next()
        .is_some_and(|first| is_hashline_anchored_line(first) && sample.all(is_hashline_anchored_line))
}

pub(crate) fn strip_code_comments_preserving_strings(raw_code: &str) -> String {
    let mut out = String::with_capacity(raw_code.len());
    let final_state = raw_code.chars().fold(CodeLexState::Normal, |state, ch| {
        match (state, ch) {
            (CodeLexState::Normal, '/') => CodeLexState::SeenSlash,
            (CodeLexState::Normal, '"') => {
                out.push('"');
                CodeLexState::InDoubleQuote
            }
            (CodeLexState::Normal, c) => {
                out.push(c);
                CodeLexState::Normal
            }
            (CodeLexState::SeenSlash, '/') => match out.ends_with(':') {
                true => {
                    out.push_str("//");
                    CodeLexState::Normal
                }
                false => CodeLexState::InLineComment,
            },
            (CodeLexState::SeenSlash, '*') => CodeLexState::InBlockComment,
            (CodeLexState::SeenSlash, '"') => {
                out.push('/');
                out.push('"');
                CodeLexState::InDoubleQuote
            }
            (CodeLexState::SeenSlash, c) => {
                out.push('/');
                out.push(c);
                CodeLexState::Normal
            }
            (CodeLexState::InLineComment, '\n') => {
                out.push('\n');
                CodeLexState::Normal
            }
            (CodeLexState::InLineComment, _) => CodeLexState::InLineComment,
            (CodeLexState::InBlockComment, '*') => CodeLexState::InBlockCommentSeenStar,
            (CodeLexState::InBlockComment, _) => CodeLexState::InBlockComment,
            (CodeLexState::InBlockCommentSeenStar, '/') => CodeLexState::Normal,
            (CodeLexState::InBlockCommentSeenStar, '*') => CodeLexState::InBlockCommentSeenStar,
            (CodeLexState::InBlockCommentSeenStar, _) => CodeLexState::InBlockComment,
            (CodeLexState::InDoubleQuote, '\\') => {
                out.push('\\');
                CodeLexState::InDoubleQuoteEscape
            }
            (CodeLexState::InDoubleQuote, '"') => {
                out.push('"');
                CodeLexState::Normal
            }
            (CodeLexState::InDoubleQuote, c) => {
                out.push(c);
                CodeLexState::InDoubleQuote
            }
            (CodeLexState::InDoubleQuoteEscape, c) => {
                out.push(c);
                CodeLexState::InDoubleQuote
            }
        }
    });

    match final_state {
        CodeLexState::SeenSlash => out.push('/'),
        _ => {}
    }

    out.lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub fn compress_code(raw_code: &str) -> String {
    match has_hashline_anchors(raw_code) {
        true => raw_code.trim().to_string(),
        false => strip_code_comments_preserving_strings(raw_code),
    }
}

pub(crate) use crate::agent::style::strip_ansi_escapes;

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    let needle_bytes = needle.as_bytes();
    let n_len = needle_bytes.len();
    match n_len == 0 {
        true => true,
        false => haystack
            .as_bytes()
            .windows(n_len)
            .any(|window| window.eq_ignore_ascii_case(needle_bytes)),
    }
}

fn is_rust_backtrace_header(trimmed: &str) -> bool {
    trimmed.contains("stack backtrace:") || trimmed.contains("backtrace::")
}

fn is_numbered_frame_prefix(trimmed: &str) -> bool {
    trimmed
        .split_once(':')
        .is_some_and(|(digits, rest)| {
            !digits.is_empty()
                && digits.chars().all(|c| c.is_ascii_digit())
                && rest.chars().next().is_some_and(char::is_whitespace)
        })
}

fn is_backtrace_frame_line(trimmed: &str) -> bool {
    trimmed.starts_with("at ")
        || trimmed.starts_with("at\t")
        || trimmed.starts_with("frame #")
        || is_numbered_frame_prefix(trimmed)
}

fn is_cargo_warning_line(trimmed: &str) -> bool {
    contains_ascii_case_insensitive(trimmed, "warning:")
}

fn has_rust_error_code(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    bytes.windows(7).enumerate().any(|(idx, window)| {
        window.eq_ignore_ascii_case(b"error[e")
            && trimmed[idx + 7..]
                .split_once("]:")
                .is_some_and(|(digits, _)| {
                    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
                })
    })
}

fn is_cargo_error_line(trimmed: &str) -> bool {
    contains_ascii_case_insensitive(trimmed, "error:") || has_rust_error_code(trimmed)
}

#[derive(Default)]
struct LogFilterState {
    filtered_lines: Vec<String>,
    warning_count: usize,
    error_count: usize,
    is_backtrace: bool,
}

pub fn compress_logs(raw_logs: &str) -> String {
    compress_logs_with_limit(raw_logs, DEFAULT_LOG_COMPACT_CHARS)
}

pub fn compress_logs_with_limit(raw_logs: &str, max_chars: usize) -> String {
    let effective_max = max_chars.max(200);
    let half_max = (effective_max / 2).max(1);
    let max_warnings = (effective_max / 200).clamp(5, 25);
    let max_errors = (effective_max / 400).clamp(3, 15);
    let clean_logs = strip_ansi_escapes(raw_logs);
    let mut state = clean_logs
        .lines()
        .fold(LogFilterState::default(), |mut acc, line| {
            let trimmed = line.trim();
            match is_rust_backtrace_header(trimmed) {
                true => {
                    acc.is_backtrace = true;
                    acc.filtered_lines.push(
                        "[Backtrace detected - stripping stack frames for token reduction]"
                            .to_string(),
                    );
                    acc
                }
                false => {
                    let skip_frame = match (acc.is_backtrace, trimmed.is_empty()) {
                        (true, true) => {
                            acc.is_backtrace = false;
                            false
                        }
                        (true, false) => is_backtrace_frame_line(trimmed),
                        (false, _) => false,
                    };
                    match skip_frame {
                        true => acc,
                        false => {
                            let is_warn = is_cargo_warning_line(trimmed);
                            acc.warning_count += usize::from(is_warn);
                            let skip_warn = is_warn && acc.warning_count > max_warnings;

                            let is_err = !skip_warn && is_cargo_error_line(trimmed);
                            acc.error_count += usize::from(is_err);
                            let skip_err = is_err && acc.error_count > max_errors;

                            match skip_warn || skip_err {
                                true => acc,
                                false => {
                                    acc.filtered_lines.push(line.to_string());
                                    acc
                                }
                            }
                        }
                    }
                }
            }
        });

    (state.warning_count > max_warnings).then(|| {
        state.filtered_lines.push(format!(
            "... [Skipped {} additional cargo warnings to save tokens] ...",
            state.warning_count - max_warnings
        ));
    });
    (state.error_count > max_errors).then(|| {
        state.filtered_lines.push(format!(
            "... [Skipped {} additional cargo errors to save tokens] ...",
            state.error_count - max_errors
        ));
    });

    let filtered_logs = state.filtered_lines.join("\n");
    let char_count = filtered_logs.chars().count();
    match char_count > effective_max {
        false => filtered_logs,
        true => {
            let first_part: String = filtered_logs.chars().take(half_max).collect();
            let last_part: String = filtered_logs
                .chars()
                .skip(char_count.saturating_sub(half_max))
                .collect();
            format!(
                "{}\n\n... [TRUNCATED LOGS] ...\n\n{}",
                first_part, last_part
            )
        }
    }
}

pub fn compress_tool_output(tool_name: &str, raw_output: &str) -> String {
    compress_tool_output_with_limit(tool_name, raw_output, DEFAULT_LOG_COMPACT_CHARS)
}

pub fn compress_tool_output_with_limit(
    tool_name: &str,
    raw_output: &str,
    max_chars: usize,
) -> String {
    let raw_trimmed = raw_output.trim();
    match raw_trimmed.is_empty() {
        true => "Empty output.".to_string(),
        false => {
            let json_limit = max_chars.max(DEFAULT_JSON_COMPACT_CHARS);
            let log_limit = (max_chars / 2).max(DEFAULT_LOG_COMPACT_CHARS);
            let json_candidate = (raw_trimmed.starts_with('[') || raw_trimmed.starts_with('{'))
                .then(|| compress_json_with_limit(raw_trimmed, json_limit).ok())
                .flatten();
            match json_candidate {
                Some(compressed) => compressed,
                None => {
                    let is_code_tool = matches!(
                        tool_name,
                        "read_file"
                            | "read_file_content"
                            | "patch_file"
                            | "write_file"
                            | "grep_search"
                            | "code_outline"
                            | "view_file"
                    );
                    match is_code_tool && raw_trimmed.chars().count() > log_limit {
                        true => compress_code(raw_trimmed),
                        false => compress_logs_with_limit(raw_trimmed, log_limit),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_json_small_array_preserved() {
        let json_input = r#"[{"id": 1, "name": "first"}, {"id": 2, "name": "second"}]"#;
        let compressed = compress_json(json_input).unwrap();
        assert!(compressed.contains("first"));
        assert!(compressed.contains("second"));
        assert!(!compressed.contains("intermediate items omitted"));
    }

    #[test]
    fn test_compress_json_large_array_structure_preserved() {
        let items: Vec<String> = (1..=10)
            .map(|i| format!(r#"{{"index": {}, "data": "item_{}"}}"#, i, i))
            .collect();
        let json_input = format!("[{}]", items.join(", "));
        let compressed = compress_json(&json_input).unwrap();

        assert!(compressed.contains("Common keys: [data, index]"));
        assert!(compressed.contains("item_1"));
        assert!(compressed.contains("item_2"));
        assert!(compressed.contains("item_10"));
        assert!(compressed.contains("7 intermediate items omitted"));
    }

    #[test]
    fn test_compress_tool_output_empty() {
        assert_eq!(compress_tool_output("exec_command", ""), "Empty output.");
        assert_eq!(compress_tool_output("exec_command", "   \n"), "Empty output.");
    }

    #[test]
    fn test_compress_code_strips_comments_while_preserving_urls_and_string_literals() {
        let code = r#"
// Top level comment
fn endpoint() -> &'static str {
    /* block comment
       spanning lines */
    let url = "https://example.com/api//v1";
    let pattern = "/* not a comment */";
    url // trailing comment
}
"#;
        let compressed = compress_code(code);
        assert!(!compressed.contains("Top level comment"));
        assert!(!compressed.contains("spanning lines"));
        assert!(!compressed.contains("trailing comment"));
        assert!(compressed.contains(r#""https://example.com/api//v1""#));
        assert!(compressed.contains(r#""/* not a comment */""#));
    }

    #[test]
    fn test_compress_code_preserves_hashline_anchored_output() {
        let hashline_output = [
            "1:a3f1| // header comment that contributes to line hash",
            "2:09bc| fn main() {",
            "3:f41d|     println!(\"hello\"); // inline comment",
            "4:77e2| }",
        ]
        .join("\n");
        let compressed = compress_code(&hashline_output);
        assert_eq!(compressed, hashline_output);
    }

    #[test]
    fn test_compress_logs_strips_ansi_and_folds_backtraces_and_caps_diagnostics() {
        let mut lines = vec![
            "\x1B[32mCompiling\x1B[0m openz v0.0.253".to_string(),
            "stack backtrace:".to_string(),
            "   0: rust_begin_unwind".to_string(),
            "             at /rustc/library/std/src/panicking.rs:645:5".to_string(),
            "   1: core::panicking::panic_fmt".to_string(),
            "".to_string(),
        ];
        lines.extend((1..=12).map(|i| format!("warning: unused variable `w{i}`")));
        lines.extend((1..=7).map(|i| format!("error[E0425]: cannot find value `e{i}`")));
        let raw = lines.join("\n");

        let compressed = compress_logs(&raw);
        assert!(compressed.contains("Compiling openz v0.0.253"));
        assert!(!compressed.contains("\x1B["));
        assert!(compressed.contains("[Backtrace detected - stripping stack frames for token reduction]"));
        assert!(!compressed.contains("rust_begin_unwind"));
        assert!(compressed.contains("Skipped 2 additional cargo warnings"));
        assert!(compressed.contains("Skipped 2 additional cargo errors"));
    }

    #[test]
    fn test_compress_logs_multibyte_utf8_no_duplication() {
        // 800 multi-byte characters (2400 bytes > 2000 bytes, but 800 chars <= 2000 chars)
        let line = format!("UNIQUE_MARKER {}", "▸".repeat(800));
        let compressed = compress_logs(&line);
        assert_eq!(compressed.matches("UNIQUE_MARKER").count(), 1);
        assert!(!compressed.contains("[TRUNCATED LOGS]"));
    }
}

