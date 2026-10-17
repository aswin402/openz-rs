use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use super::query::{detect_active_session, session_matches, LogLevelFilter, SessionFilter};
use super::storage::default_db_path;

// ── AURA palette (raw ANSI — no crossterm needed) ──────────────────────────
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

const PURPLE: &str = "\x1b[38;2;199;146;234m"; // AURA_PURPLE  — brand / header
const BLUE: &str = "\x1b[38;2;130;170;255m"; // AURA_BLUE    — target module
const GREEN: &str = "\x1b[38;2;195;232;141m"; // AURA_GREEN   — INFO
const GOLD: &str = "\x1b[38;2;255;203;107m"; // AURA_GOLD    — WARN
const ROSE: &str = "\x1b[38;2;240;113;120m"; // AURA_ROSE    — ERROR
const SLATE: &str = "\x1b[38;2;107;122;153m"; // AURA_SLATE   — timestamp / dim
const WHITE: &str = "\x1b[38;2;220;220;220m"; // LIGHT_WHITE  — message body
const ORANGE: &str = "\x1b[38;2;255;133;75m"; // warm accent  — DEBUG
const CYAN: &str = "\x1b[38;2;137;221;255m"; // AURA_CYAN    — session tag

// ── Line parser ─────────────────────────────────────────────────────────────

struct ParsedLine<'a> {
    timestamp: &'a str,
    level: &'a str,
    target: &'a str,
    message: &'a str,
    /// All session= values found anywhere in the line (spans + trailing field).
    sessions: Vec<String>,
}

/// Extract ALL `session=<value>` occurrences from the entire line.
/// Handles nested tracing spans like `turn{session=cli:abc}:turn{session=subagent:x:123}`
/// as well as trailing `session=` fields.
fn extract_all_sessions_from_line(line: &str) -> Vec<String> {
    let mut sessions = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = line[search_from..].find("session=") {
        let abs_pos = search_from + pos;
        let start = abs_pos + "session=".len();
        let val_slice = &line[start..];
        let end = val_slice
            .find([' ', ',', '}', ']', '\n', ')'])
            .unwrap_or(val_slice.len());
        let val = val_slice[..end].trim().to_string();
        match !val.is_empty() && !sessions.contains(&val) {
            true => sessions.push(val),
            false => {}
        }
        search_from = start + end;
    }
    sessions
}

/// Parse a tracing-subscriber line:
/// `2026-06-16T17:20:43.215712Z  INFO openz::tools::mcp: message  session=cli:direct`
///
/// The `session=` field is optional — old log entries before the span was added
/// will not have it and are always shown regardless of filter.
fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let line = line.trim();
    (!line.is_empty()).then_some(())?;

    // timestamp is the first token (ISO 8601, ends with 'Z')
    let (ts, rest) = line.split_once(' ')?;
    ts.ends_with('Z').then_some(())?;

    // level is the next word
    let rest = rest.trim_start();
    let (level, rest) = rest.split_once(' ')?;
    let level = level.trim();

    // target ends at the first ': '
    let rest = rest.trim_start();
    let (target, message_raw) = match rest.find(": ") {
        Some(idx) => (&rest[..idx], &rest[idx + 2..]),
        None => ("", rest),
    };

    // Extract `session=<value>` from the trailing span fields.
    // tracing-subscriber appends span fields after the message, separated by
    // at least two spaces (or a tab):  `message  session=cli:direct`
    let (message, _session_opt) = extract_session_field(message_raw);
    // Collect ALL session= values from anywhere in the line (spans + trailing).
    let sessions = extract_all_sessions_from_line(line);

    Some(ParsedLine {
        timestamp: ts,
        level,
        target,
        message,
        sessions,
    })
}

/// Split `"message body  session=cli:direct"` into `("message body", Some("cli:direct"))`.
/// Returns `(original, None)` when the field is absent.
fn extract_session_field(raw: &str) -> (&str, Option<String>) {
    match raw.rfind(" session=") {
        Some(pos) => {
            let value = raw[pos + " session=".len()..].trim().to_string();
            let msg = raw[..pos].trim_end();
            (msg, Some(value))
        }
        None => match raw.strip_prefix("session=") {
            Some(stripped) => ("", Some(stripped.trim().to_string())),
            None => (raw, None),
        },
    }
}

// ── Pretty-print a single line ──────────────────────────────────────────────

fn is_word_boundary_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_number_token(token: &str) -> bool {
    let mut parts = token.split('.');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(int_part), None, None) => {
            !int_part.is_empty() && int_part.chars().all(|c| c.is_ascii_digit())
        }
        (Some(int_part), Some(frac_part), None) => {
            !int_part.is_empty()
                && !frac_part.is_empty()
                && int_part.chars().all(|c| c.is_ascii_digit())
                && frac_part.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

fn highlight_json_segment(json_part: &str) -> String {
    let mut out = String::with_capacity(json_part.len() + 32);
    let mut rest = json_part;
    let mut prev_word_char = false;

    while !rest.is_empty() {
        let first = rest.chars().next().unwrap_or('\0');
        match first {
            '"' => {
                let after_open = &rest[1..];
                match after_open.find('"') {
                    Some(end_idx) => {
                        let inner = &after_open[..end_idx];
                        let after_close = &after_open[end_idx + 1..];
                        match (!inner.is_empty(), after_close.strip_prefix(':')) {
                            (true, Some(after_colon)) => {
                                let trimmed = after_colon.trim_start();
                                out.push_str(&format!("\"{CYAN}{inner}{RESET}\": "));
                                rest = trimmed;
                            }
                            (true, None) => {
                                out.push_str(&format!("\"{GREEN}{inner}{RESET}\""));
                                rest = after_close;
                            }
                            (false, _) => {
                                out.push_str("\"\"");
                                rest = after_close;
                            }
                        }
                        prev_word_char = false;
                    }
                    None => {
                        out.push('"');
                        rest = after_open;
                        prev_word_char = false;
                    }
                }
            }
            c => {
                let starts_token =
                    !prev_word_char && (c.is_ascii_digit() || matches!(c, 't' | 'f' | 'n'));
                match starts_token {
                    true => {
                        let token_len = rest
                            .char_indices()
                            .find_map(|(idx, ch)| {
                                (!(ch.is_ascii_alphanumeric() || ch == '_'))
                                    .then_some(idx)
                            })
                            .unwrap_or(rest.len());
                        let num_len = rest
                            .char_indices()
                            .find_map(|(idx, ch)| {
                                (!(ch.is_ascii_digit() || ch == '.')).then_some(idx)
                            })
                            .unwrap_or(rest.len());
                        let candidate_len = match c.is_ascii_digit() {
                            true => num_len.max(token_len),
                            false => token_len,
                        };
                        let token = &rest[..candidate_len];
                        let after_token = &rest[candidate_len..];
                        let next_is_word = after_token
                            .chars()
                            .next()
                            .is_some_and(is_word_boundary_char);
                        let is_literal = !next_is_word
                            && (matches!(token, "true" | "false" | "null")
                                || is_number_token(token));
                        match is_literal {
                            true => {
                                out.push_str(&format!("{ORANGE}{token}{RESET}"));
                                rest = after_token;
                                prev_word_char = false;
                            }
                            false => {
                                out.push(c);
                                rest = &rest[c.len_utf8()..];
                                prev_word_char = is_word_boundary_char(c);
                            }
                        }
                    }
                    false => {
                        out.push(c);
                        rest = &rest[c.len_utf8()..];
                        prev_word_char = is_word_boundary_char(c);
                    }
                }
            }
        }
    }

    out
}

fn highlight_message(msg: &str) -> String {
    match msg.find('{').or_else(|| msg.find('[')) {
        Some(start_idx) => {
            let prefix = &msg[..start_idx];
            let json_part = &msg[start_idx..];
            format!("{}{}", prefix, highlight_json_segment(json_part))
        }
        None => msg.to_string(),
    }
}

fn is_subagent_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "delegate_task" | "parallel_research" | "evaluator_optimizer_loop"
    ) || crate::subagents::load_profiles()
        .map(|profiles| profiles.iter().any(|p| p.name == tool_name))
        .unwrap_or(false)
}

fn slice_after_prefix<'a>(text: &'a str, prefixes: &[&str]) -> &'a str {
    prefixes
        .iter()
        .find_map(|prefix| text.find(prefix).map(|idx| &text[idx + prefix.len()..]))
        .unwrap_or(text)
}

fn format_user_log(message: &str) -> Option<(String, String, String, &'static str)> {
    (message.contains("User prompt:") || message.contains("User input:")).then(|| {
        let prompt = slice_after_prefix(message, &["User prompt:", "User input:"]);
        (
            "👤".to_string(),
            "USER".to_string(),
            prompt.trim().to_string(),
            CYAN,
        )
    })
}

fn format_llm_call_log(message: &str) -> Option<(String, String, String, &'static str)> {
    (message.contains("Sending completion request to LLM")
        || message.contains("Sending request to LLM"))
    .then(|| {
        let model = message
            .find("model:")
            .map(|idx| {
                message[idx + "model:".len()..]
                    .trim_matches(|c| c == ')' || c == '"' || c == '(')
                    .trim()
            })
            .unwrap_or("");
        let msg = match model.is_empty() {
            true => "Sending request to LLM".to_string(),
            false => format!("Sending request to LLM (model: {})", model),
        };
        ("📡".to_string(), "LLM CALL".to_string(), msg, SLATE)
    })
}

fn format_thinking_log(message: &str) -> Option<(String, String, String, &'static str)> {
    message.contains("LLM reasoning content:").then(|| {
        let thought = slice_after_prefix(message, &["LLM reasoning content:"]);
        (
            "🧠".to_string(),
            "THINKING".to_string(),
            thought.trim().to_string(),
            ORANGE,
        )
    })
}

fn format_response_text_log(message: &str) -> Option<(String, String, String, &'static str)> {
    message.contains("LLM text content:").then(|| {
        let resp = slice_after_prefix(message, &["LLM text content:"]);
        (
            "🤖".to_string(),
            "RESPONSE".to_string(),
            resp.trim().to_string(),
            WHITE,
        )
    })
}

fn format_response_meta_log(message: &str) -> Option<(String, String, String, &'static str)> {
    message.contains("Received LLM response").then(|| {
        let finish = message
            .find("finish_reason:")
            .map(|idx| {
                message[idx + "finish_reason:".len()..]
                    .trim_matches(|c| c == ')' || c == '"' || c == '(')
                    .trim()
            })
            .unwrap_or("");
        let msg = match finish.is_empty() {
            true => "Received LLM response".to_string(),
            false => format!("Received LLM response (finish_reason: {})", finish),
        };
        ("🤖".to_string(), "RESPONSE".to_string(), msg, WHITE)
    })
}

fn format_tool_start_log(message: &str) -> Option<(String, String, String, &'static str)> {
    message.contains("Executing tool call").then(|| {
        let tool = extract_quoted_field(message, "tool=");
        let args = extract_quoted_field(message, "arguments=");
        let is_subagent = tool.as_deref().is_some_and(is_subagent_tool);
        let formatted = match (tool, args) {
            (Some(t), Some(a)) => format!("{} with args: {}", t, a),
            (Some(t), None) => t,
            _ => "tool execution".to_string(),
        };
        match is_subagent {
            true => (
                "🤖".to_string(),
                "SUBAGENT START".to_string(),
                formatted,
                PURPLE,
            ),
            false => ("🛠️".to_string(), "TOOL START".to_string(), formatted, GOLD),
        }
    })
}

fn format_tool_done_log(message: &str) -> Option<(String, String, String, &'static str)> {
    message.contains("Tool call completed").then(|| {
        let tool = extract_quoted_field(message, "tool=");
        let is_subagent = tool.as_deref().is_some_and(is_subagent_tool);
        let formatted = match tool {
            Some(t) => format!("{} completed", t),
            None => "tool completed".to_string(),
        };
        match is_subagent {
            true => (
                "🤖".to_string(),
                "SUBAGENT DONE".to_string(),
                formatted,
                GREEN,
            ),
            false => ("✅".to_string(), "TOOL DONE".to_string(), formatted, GREEN),
        }
    })
}

fn format_tool_fail_log(message: &str) -> Option<(String, String, String, &'static str)> {
    (message.contains("Tool call failed") || message.contains("Tool call timed out")).then(|| {
        let tool = extract_quoted_field(message, "tool=");
        let err = extract_quoted_field(message, "error=");
        let timed_out = message.contains("timed out");
        let base = match timed_out {
            true => "timed out",
            false => "failed",
        };
        let is_subagent = tool.as_deref().is_some_and(is_subagent_tool);
        let formatted = match (tool, err) {
            (Some(t), Some(e)) => format!("{} {} - error: {}", t, base, e),
            (Some(t), None) => format!("{} {}", t, base),
            _ => format!("tool {}", base),
        };
        let icon = match timed_out {
            true => "⏱️",
            false => "✕",
        };
        let label = match is_subagent {
            true => "SUBAGENT FAIL",
            false => "TOOL FAIL",
        };
        (icon.to_string(), label.to_string(), formatted, ROSE)
    })
}

fn format_blocked_log(message: &str) -> Option<(String, String, String, &'static str)> {
    (message.contains("Tool execution blocked")
        || message.contains("forbidden by security")
        || message.contains("denied by user"))
    .then(|| {
        let tool = extract_quoted_field(message, "tool=");
        let reason = [
            (message.contains("blocked"), "loop/repetition detected"),
            (
                message.contains("forbidden"),
                "forbidden by security policies",
            ),
        ]
        .into_iter()
        .find_map(|(matched, text)| matched.then_some(text))
        .unwrap_or("denied by user");
        let formatted = match tool {
            Some(t) => format!("{} blocked - reason: {}", t, reason),
            None => format!("tool blocked - reason: {}", reason),
        };
        ("🛡️".to_string(), "BLOCKED".to_string(), formatted, GOLD)
    })
}

fn format_curator_log(message: &str) -> Option<(String, String, String, &'static str)> {
    message.contains("Self-improvement curator").then(|| {
        let rest = slice_after_prefix(
            message,
            &["Self-improvement curator:", "Self-improvement curator"],
        );
        (
            "🧹".to_string(),
            "CURATOR".to_string(),
            rest.trim()
                .trim_start_matches(['-', ':'])
                .trim()
                .to_string(),
            PURPLE,
        )
    })
}

fn format_lifecycle_log(message: &str) -> Option<(String, String, String, &'static str)> {
    let rules: [(bool, &str, &str, String, &'static str); 3] = [
        (
            message.contains("Session saved"),
            "💾",
            "SAVED",
            "Session saved. Turn complete.".to_string(),
            GREEN,
        ),
        (
            message.contains("Compacting history"),
            "🗜️",
            "COMPACT",
            message.to_string(),
            SLATE,
        ),
        (
            message.contains("Compacted summary length")
                || message.contains("Consolidated long-term memory"),
            "💾",
            "COMPACTED",
            message.to_string(),
            SLATE,
        ),
    ];
    rules
        .into_iter()
        .find_map(|(matched, icon, label, text, color)| {
            matched.then(|| (icon.to_string(), label.to_string(), text, color))
        })
}

type LogFormatterFn = fn(&str) -> Option<(String, String, String, &'static str)>;

const PRETTY_LOG_FORMATTERS: &[LogFormatterFn] = &[
    format_user_log,
    format_llm_call_log,
    format_thinking_log,
    format_response_text_log,
    format_response_meta_log,
    format_tool_start_log,
    format_tool_done_log,
    format_tool_fail_log,
    format_blocked_log,
    format_curator_log,
    format_lifecycle_log,
];

fn pretty_format_log(message: &str) -> Option<(String, String, String, &'static str)> {
    PRETTY_LOG_FORMATTERS
        .iter()
        .find_map(|formatter| formatter(message))
}

fn extract_quoted_field(text: &str, field_prefix: &str) -> Option<String> {
    let pos = text.find(field_prefix)?;
    let start = pos + field_prefix.len();
    let tail = &text[start..];
    match tail.strip_prefix('"') {
        Some(rest) => {
            let mut val = String::new();
            let mut chars = rest.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => match chars.peek().copied() {
                        Some('"' | '\\') => match chars.next() {
                            Some(escaped) => val.push(escaped),
                            None => {}
                        },
                        _ => val.push(c),
                    },
                    '"' => break,
                    other => val.push(other),
                }
            }
            Some(val)
        }
        None => {
            let end = tail.find(' ').unwrap_or(tail.len());
            Some(tail[..end].to_string())
        }
    }
}

const TARGET_COLOR_PREFIXES: &[(&str, &str)] = &[
    ("openz::agent::", PURPLE),
    ("openz::providers::", CYAN),
    ("openz::tools::", GOLD),
    ("openz::channels::", GREEN),
];

fn resolve_target_color(target: &str) -> &'static str {
    TARGET_COLOR_PREFIXES
        .iter()
        .find_map(|(prefix, color)| target.starts_with(prefix).then_some(*color))
        .unwrap_or(BLUE)
}

fn matches_search_query(message: &str, target: &str, search: Option<&str>) -> bool {
    match search {
        Some(q) => {
            let q_lower = q.to_lowercase();
            message.to_lowercase().contains(&q_lower) || target.to_lowercase().contains(&q_lower)
        }
        None => true,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_log_row(
    timestamp: &str,
    level: &str,
    target: &str,
    message: &str,
    sessions: &[String],
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
    search: Option<&str>,
) {
    let should_render = session_matches(sessions, filter)
        && level_filter.matches(level)
        && matches_search_query(message, target, search);
    match should_render {
        false => return,
        true => {}
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let ts = timestamp.get(11..19).unwrap_or(timestamp);

    let (level_col, level_label) = match level {
        "ERROR" => (ROSE, "ERROR"),
        "WARN" => (GOLD, "WARN "),
        "INFO" => (GREEN, "INFO "),
        "DEBUG" => (ORANGE, "DEBUG"),
        "TRACE" => (SLATE, "TRACE"),
        other => (SLATE, other),
    };

    let msg_col = match level {
        "ERROR" => ROSE,
        "WARN" => GOLD,
        _ => WHITE,
    };

    let target_str = match target.len() > 35 {
        true => format!("…{}", &target[target.len().saturating_sub(34)..]),
        false => target.to_string(),
    };

    let target_col = resolve_target_color(target);

    let session_badge = match sessions.last() {
        Some(s) => {
            let short = s.split(':').next().unwrap_or(s.as_str());
            format!("  {CYAN}{DIM}[{short}]{RESET}")
        }
        None => String::new(),
    };

    match pretty_format_log(message) {
        Some((icon, label, formatted_msg, color)) => {
            let highlighted = highlight_message(&formatted_msg);
            let _ = writeln!(
                out,
                "{SLATE}{DIM}{ts}{RESET}  {BOLD}{level_col}{level_label}{RESET}  {target_col}{target_str:<35}{RESET}  {icon} {BOLD}{color}[{label}]{RESET} {color}{}{RESET}{session_badge}",
                highlighted,
                ts = ts,
                level_col = level_col,
                level_label = level_label,
                target_str = &target_str,
                target_col = target_col,
                icon = icon,
                label = label,
                color = color,
                session_badge = session_badge,
            );
        }
        None => {
            let highlighted_msg = highlight_message(message);
            let _ = writeln!(
                out,
                "{SLATE}{DIM}{ts}{RESET}  {BOLD}{level_col}{level_label}{RESET}  {target_col}{target_str:<35}{RESET}  {msg_col}{}{RESET}{session_badge}",
                highlighted_msg,
                ts = ts,
                level_col = level_col,
                level_label = level_label,
                target_str = &target_str,
                target_col = target_col,
                msg_col = msg_col,
                session_badge = session_badge,
            );
        }
    }
}

fn print_line_filtered(
    raw: &str,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
    search: Option<&str>,
) {
    match parse_line(raw) {
        Some(p) => render_log_row(
            p.timestamp,
            p.level,
            p.target,
            p.message,
            &p.sessions,
            filter,
            level_filter,
            search,
        ),
        None => {
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            let _ = writeln!(out, "  {DIM}{SLATE}{raw}{RESET}");
        }
    }
}

// ── Header banner ───────────────────────────────────────────────────────────

fn print_header(
    path: &Path,
    tail: usize,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
) {
    let fname = path.display();
    let filter_label = filter.label();
    let level_label = format!("{:?}", level_filter);
    println!(
        "\n{PURPLE}{BOLD}  ◇ openz{RESET}  {SLATE}live logs{RESET}  {DIM}─{RESET}  {SLATE}{fname}{RESET}  {DIM}(tail {tail}  ·  {filter_label}  ·  level {level_label}){RESET}"
    );
    println!("{SLATE}{DIM}  {}{RESET}\n", "─".repeat(72));
    println!(
        "  {SLATE}{DIM}{:<8}  {:<5}  {:<35}  MESSAGE{RESET}",
        "TIME", "LEVEL", "TARGET"
    );
    println!("  {SLATE}{DIM}{}{RESET}\n", "─".repeat(72));
}

// ── Tail initial lines (reverse seek, O(tail) memory) ───────────────────

fn push_non_empty_reversed(leftover: &mut Vec<char>, lines: &mut Vec<String>) {
    match leftover.is_empty() {
        true => {}
        false => {
            let line: String = leftover.iter().rev().collect();
            leftover.clear();
            match line.trim().is_empty() {
                true => {}
                false => lines.push(line),
            }
        }
    }
}

fn print_tail(
    path: &PathBuf,
    tail: usize,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
    search: Option<&str>,
) -> Result<u64> {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
            println!(
                "  {GOLD}Waiting for log file to appear at {SLATE}{}{RESET}",
                path.display()
            );
            return Ok(0);
        }
    };

    let file_len = f.seek(SeekFrom::End(0))?;

    match tail {
        0 => return Ok(file_len),
        _ => {}
    }

    let mut lines = Vec::with_capacity(tail.min(1000));
    let mut pos = file_len;
    let mut leftover = Vec::new();
    const BLOCK_SIZE: usize = 4096;

    while pos > 0 && lines.len() < tail {
        let read_size = BLOCK_SIZE.min(pos as usize);
        pos -= read_size as u64;
        f.seek(SeekFrom::Start(pos))?;

        let mut buf = vec![0u8; read_size];
        f.read_exact(&mut buf)?;

        let block_str = String::from_utf8_lossy(&buf);
        for (_i, c) in block_str.char_indices().rev() {
            match c {
                '\n' => {
                    push_non_empty_reversed(&mut leftover, &mut lines);
                    match lines.len() >= tail {
                        true => break,
                        false => {}
                    }
                }
                other => leftover.push(other),
            }
        }
    }

    push_non_empty_reversed(&mut leftover, &mut lines);

    lines.reverse();

    for line in &lines {
        print_line_filtered(line, filter, level_filter, search);
    }

    Ok(file_len)
}

// ── Read new bytes helper (shared between notify and poll paths) ─────────

#[cfg(unix)]
fn get_file_id(metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.ino()
}

#[cfg(not(unix))]
fn get_file_id(_metadata: &std::fs::Metadata) -> u64 {
    0
}

fn read_new_bytes(
    path: &PathBuf,
    pos: &mut u64,
    buffer: &mut Vec<u8>,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
    current_file_id: &mut Option<u64>,
    search: Option<&str>,
) {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let metadata = match f.metadata() {
        Ok(m) => m,
        Err(_) => return,
    };
    let len = metadata.len();
    let file_id = get_file_id(&metadata);

    let is_new_file = match current_file_id {
        Some(id) => *id != file_id,
        None => true,
    };

    match (len < *pos || is_new_file, is_new_file && current_file_id.is_some(), len < *pos) {
        (true, true, _) => {
            println!("\n  {GOLD}── log file recreated, reading from start ──{RESET}\n");
            *pos = 0;
            buffer.clear();
            *current_file_id = Some(file_id);
        }
        (true, false, true) => {
            println!("\n  {GOLD}── log rotated/truncated, reading from start ──{RESET}\n");
            *pos = 0;
            buffer.clear();
            *current_file_id = Some(file_id);
        }
        (true, false, false) => {
            *pos = 0;
            buffer.clear();
            *current_file_id = Some(file_id);
        }
        (false, _, _) => {}
    }

    match f.seek(SeekFrom::Start(*pos)) {
        Ok(_) => {
            let mut temp_buf = Vec::new();
            match f.read_to_end(&mut temp_buf).is_ok() && !temp_buf.is_empty() {
                true => {
                    *pos += temp_buf.len() as u64;
                    buffer.extend_from_slice(&temp_buf);

                    match buffer.iter().rposition(|&b| b == b'\n') {
                        Some(last_newline_idx) => {
                            let complete_bytes = &buffer[..=last_newline_idx];
                            let text = String::from_utf8_lossy(complete_bytes);
                            for line in text.lines() {
                                print_line_filtered(line, filter, level_filter, search);
                            }
                            *buffer = buffer[last_newline_idx + 1..].to_vec();
                            let _ = std::io::stdout().flush();
                        }
                        None => {}
                    }
                }
                false => {}
            }
        }
        Err(_) => {}
    }
}

/// Check whether auto-followed session changed.
fn update_auto_session(filter: &mut SessionFilter) {
    match filter {
        SessionFilter::Auto(ref current) => {
            let active = detect_active_session();
            match active != *current {
                true => {
                    match &active {
                        Some(new_id) => {
                            println!("\n  {CYAN}{DIM}◉ active session changed: {new_id}{RESET}\n");
                        }
                        None => {
                            println!("\n  {CYAN}{DIM}◉ active session lost (idle){RESET}\n");
                        }
                    }
                    *filter = SessionFilter::Auto(active);
                }
                false => {}
            }
        }
        _ => {}
    }
}

async fn recv_notify_event(
    rx: &mut tokio::sync::mpsc::Receiver<notify::Result<notify::Event>>,
    enabled: bool,
) -> Option<notify::Result<notify::Event>> {
    match enabled {
        true => rx.recv().await,
        false => std::future::pending().await,
    }
}

fn maybe_refresh_auto_session(
    last_session_check: &mut std::time::Instant,
    filter: &mut SessionFilter,
) {
    match last_session_check.elapsed() >= Duration::from_secs(1) {
        true => {
            *last_session_check = std::time::Instant::now();
            update_auto_session(filter);
        }
        false => {}
    }
}

// ── Follow loop (notify-based, falls back to polling) ────────────────────

async fn follow(
    path: &PathBuf,
    mut pos: u64,
    mut filter: SessionFilter,
    level_filter: LogLevelFilter,
    search: Option<&str>,
) -> Result<()> {
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    let mut current_file_id = std::fs::metadata(path).ok().map(|m| get_file_id(&m));

    let mut buffer = Vec::new();
    let mut last_session_check = std::time::Instant::now();

    let (notify_tx, mut notify_rx) =
        tokio::sync::mpsc::channel::<notify::Result<notify::Event>>(256);
    let notify_path = path.clone();

    let watcher_guard = RecommendedWatcher::new(
        move |res| {
            let _ = notify_tx.blocking_send(res);
        },
        Config::default(),
    )
    .ok()
    .and_then(|mut watcher| {
        watcher
            .watch(&notify_path, RecursiveMode::NonRecursive)
            .ok()
            .map(|()| watcher)
    });

    let notify_available = watcher_guard.is_some();

    let poll_ms = match notify_available {
        true => 1000,
        false => 100,
    };
    let mut interval = tokio::time::interval(Duration::from_millis(poll_ms));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;
            _ = &mut ctrl_c => {
                let _ = &watcher_guard;
                println!("\n\n  {SLATE}{DIM}── openz logs stopped{RESET}\n");
                break Ok(());
            }
            _ = recv_notify_event(&mut notify_rx, notify_available) => {
                read_new_bytes(path, &mut pos, &mut buffer, &filter, &level_filter, &mut current_file_id, search);
                maybe_refresh_auto_session(&mut last_session_check, &mut filter);
            }
            _ = interval.tick() => {
                match notify_available {
                    false => read_new_bytes(path, &mut pos, &mut buffer, &filter, &level_filter, &mut current_file_id, search),
                    true => {}
                }
                maybe_refresh_auto_session(&mut last_session_check, &mut filter);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn print_row(
    timestamp: &str,
    level: &str,
    target: &str,
    message: &str,
    session: Option<&str>,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
    search: Option<&str>,
) {
    let sessions_vec = session.map(|s| vec![s.to_string()]).unwrap_or_default();
    render_log_row(
        timestamp,
        level,
        target,
        message,
        &sessions_vec,
        filter,
        level_filter,
        search,
    );
}

pub(crate) fn print_tail_sqlite(
    db_path: &Path,
    tail: usize,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
    search: Option<&str>,
) -> Result<i64> {
    let conn = rusqlite::Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, level, target, message, session FROM logs ORDER BY id DESC LIMIT ?1",
    )?;

    struct DbRow {
        id: i64,
        timestamp: String,
        level: String,
        target: String,
        message: String,
        session: Option<String>,
    }

    let rows_iter = stmt.query_map([tail], |row| {
        Ok(DbRow {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            level: row.get(2)?,
            target: row.get(3)?,
            message: row.get(4)?,
            session: row.get(5)?,
        })
    })?;

    let mut rows: Vec<DbRow> = rows_iter.flatten().collect();
    rows.reverse();

    let mut last_id = 0;
    for r in &rows {
        print_row(
            &r.timestamp,
            &r.level,
            &r.target,
            &r.message,
            r.session.as_deref(),
            filter,
            level_filter,
            search,
        );
        last_id = r.id;
    }

    Ok(last_id)
}

fn poll_sqlite_rows(
    db_path: &Path,
    last_id: &mut i64,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
    search: Option<&str>,
) {
    let conn = match rusqlite::Connection::open(db_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut stmt = match conn.prepare(
        "SELECT id, timestamp, level, target, message, session FROM logs WHERE id > ?1 ORDER BY id ASC",
    ) {
        Ok(s) => s,
        Err(_) => return,
    };
    let rows_iter = match stmt.query_map([*last_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return,
    };
    for (id, timestamp, level, target, message, session) in rows_iter.flatten() {
        print_row(
            &timestamp,
            &level,
            &target,
            &message,
            session.as_deref(),
            filter,
            level_filter,
            search,
        );
        *last_id = id;
    }
}

async fn follow_sqlite(
    db_path: &Path,
    mut last_id: i64,
    mut filter: SessionFilter,
    level_filter: LogLevelFilter,
    search: Option<&str>,
) -> Result<()> {
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut last_session_check = std::time::Instant::now();

    loop {
        tokio::select! {
            biased;
            _ = &mut ctrl_c => {
                println!("\n\n  {SLATE}{DIM}── openz logs stopped{RESET}\n");
                break Ok(());
            }
            _ = interval.tick() => {
                poll_sqlite_rows(db_path, &mut last_id, &filter, &level_filter, search);
                maybe_refresh_auto_session(&mut last_session_check, &mut filter);
            }
        }
    }
}

// ── Public entrypoint ────────────────────────────────────────────────────────

pub async fn run_logs_viewer(
    log_path: Option<PathBuf>,
    tail: usize,
    filter: SessionFilter,
    level_filter: LogLevelFilter,
    search: Option<String>,
) -> Result<()> {
    let path = log_path.unwrap_or_else(default_db_path);
    let is_sqlite = path.extension().is_some_and(|ext| ext == "db");

    let effective_filter = match &filter {
        SessionFilter::Only(_) | SessionFilter::Auto(_) => filter.clone(),
        SessionFilter::All => {
            match detect_active_session() {
                Some(active) => {
                    println!("\n  {CYAN}{DIM}◉ active session detected: {active}{RESET}");
                }
                None => {}
            }
            filter.clone()
        }
    };

    print_header(&path, tail, &effective_filter, &level_filter);
    match is_sqlite {
        true => {
            let last_id = print_tail_sqlite(
                &path,
                tail,
                &effective_filter,
                &level_filter,
                search.as_deref(),
            )?;
            println!("\n  {PURPLE}{DIM}── live ──{RESET}\n");
            follow_sqlite(
                &path,
                last_id,
                effective_filter,
                level_filter,
                search.as_deref(),
            )
            .await
        }
        false => {
            let pos = print_tail(
                &path,
                tail,
                &effective_filter,
                &level_filter,
                search.as_deref(),
            )?;
            println!("\n  {PURPLE}{DIM}── live ──{RESET}\n");
            follow(
                &path,
                pos,
                effective_filter,
                level_filter,
                search.as_deref(),
            )
            .await
        }
    }
}

pub fn print_session_recent_logs(
    tail: usize,
    session: Option<String>,
    level: Option<String>,
) -> Result<()> {
    let path = default_db_path();
    let is_sqlite = path.extension().is_some_and(|ext| ext == "db");

    let filter = match session {
        Some(s) => match s.to_lowercase().as_str() {
            "all" => SessionFilter::All,
            _ => SessionFilter::Only(s),
        },
        None => SessionFilter::All,
    };

    let level_filter = match level {
        Some(l) => match l.to_lowercase().as_str() {
            "trace" => LogLevelFilter::Trace,
            "debug" => LogLevelFilter::Debug,
            "info" => LogLevelFilter::Info,
            "warn" => LogLevelFilter::Warn,
            "error" => LogLevelFilter::Error,
            _ => LogLevelFilter::Debug,
        },
        None => LogLevelFilter::Debug,
    };

    match is_sqlite {
        true => {
            let _ = print_tail_sqlite(&path, tail, &filter, &level_filter, None)?;
        }
        false => {
            let _ = print_tail(&path, tail, &filter, &level_filter, None)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tui_tests.rs"]
mod tests;

