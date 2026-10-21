pub mod colors;
pub mod icons;
pub mod markdown;
pub mod menu;
pub mod spinner;

pub use colors::*;
pub use icons::*;
pub use markdown::*;
pub use menu::*;
pub use spinner::*;

use std::io::Write;

#[macro_export]
macro_rules! tui_println {
    () => {
        $crate::agent::style::tui_println_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_println_fn(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! tui_print {
    () => {
        $crate::agent::style::tui_print_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_print_fn(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! tui_eprintln {
    () => {
        $crate::agent::style::tui_eprintln_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_eprintln_fn(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! tui_eprint {
    () => {
        $crate::agent::style::tui_eprint_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_eprint_fn(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    () => { $crate::tui_println!() };
    ($($arg:tt)*) => { $crate::tui_println!($($arg)*) };
}

#[macro_export]
macro_rules! print {
    () => { $crate::tui_print!() };
    ($($arg:tt)*) => { $crate::tui_print!($($arg)*) };
}

#[macro_export]
macro_rules! eprintln {
    () => { $crate::tui_println!() };
    ($($arg:tt)*) => { $crate::tui_println!($($arg)*) };
}

#[macro_export]
macro_rules! eprint {
    () => { $crate::tui_print!() };
    ($($arg:tt)*) => { $crate::tui_print!($($arg)*) };
}

fn write_tui_stream(msg: &str, to_stderr: bool, append_newline: bool) {
    (!is_silent()).then(|| {
        let replaced = msg.replace("\r\n", "\n").replace('\n', "\r\n");
        let _guard = stdout_lock();
        is_spinner_active().then(|| {
            std::print!("\r\x1b[2K");
        });
        match (to_stderr, append_newline) {
            (false, true) => {
                std::print!("{}\r\n", replaced);
                let _ = std::io::stdout().flush();
            }
            (false, false) => {
                std::print!("{}", replaced);
                let _ = std::io::stdout().flush();
            }
            (true, true) => {
                std::eprint!("{}\r\n", replaced);
                let _ = std::io::stderr().flush();
            }
            (true, false) => {
                std::eprint!("{}", replaced);
                let _ = std::io::stderr().flush();
            }
        }
    });
}

pub fn tui_println_fn<T: AsRef<str>>(msg: T) {
    write_tui_stream(msg.as_ref(), false, true);
}

pub fn tui_print_fn<T: AsRef<str>>(msg: T) {
    write_tui_stream(msg.as_ref(), false, false);
}

pub fn tui_eprintln_fn<T: AsRef<str>>(msg: T) {
    write_tui_stream(msg.as_ref(), true, true);
}

pub fn tui_eprint_fn<T: AsRef<str>>(msg: T) {
    write_tui_stream(msg.as_ref(), true, false);
}

/// Returns the prefix string for a tree trace line at the specified delegation depth.
pub fn get_tree_prefix_for_depth(is_leaf: bool, depth: usize) -> String {
    match (depth > 0, is_leaf) {
        (true, _) => format!("{}L ", "  ".repeat(depth)),
        (false, true) => "  L ".to_string(),
        (false, false) => String::new(),
    }
}

/// Returns the prefix string for a tree trace line at the current delegation depth.
pub fn get_tree_prefix(is_leaf: bool) -> String {
    let depth = crate::tools::subagent::DELEGATION_DEPTH
        .try_with(|d| *d)
        .unwrap_or(0);
    get_tree_prefix_for_depth(is_leaf, depth)
}

/// Converts a tool name into a clean, visual title.
pub fn get_tool_clean_name(name: &str) -> String {
    crate::tools::presentation_name(name)
}

/// Generates a styled spinner message indicating a tool is running under the tree bullet.
pub fn get_tree_spinner_msg(_name: &str, _formatted_args: &str) -> String {
    let prefix = get_tree_prefix(true);
    format!(
        "{}{}{}Running...{}",
        colors::AURA_SLATE,
        prefix,
        colors::AURA_SLATE,
        colors::COLOR_RESET
    )
}

/// Strips standard CSI ANSI escape sequences (\x1b[...<alpha>) from a string.
pub fn strip_ansi_escapes(s: &str) -> String {
    let (result, _) = s.chars().fold(
        (String::with_capacity(s.len()), false),
        |(mut acc, in_escape), c| match (in_escape, c == '\x1b', c.is_ascii_alphabetic()) {
            (_, true, _) => (acc, true),
            (true, false, true) => (acc, false),
            (true, false, false) => (acc, true),
            (false, false, _) => {
                acc.push(c);
                (acc, false)
            }
        },
    );
    result
}

/// Helper to clean up friendly name duplication from formatted tool arguments.
pub fn clean_tool_args_msg(name: &str, formatted_args: &str) -> String {
    let stripped = strip_ansi_escapes(formatted_args);
    let trimmed = stripped.trim();
    match trimmed.is_empty() {
        true => String::new(),
        false => {
            let paren_inner = trimmed
                .strip_suffix(')')
                .and_then(|before_close| before_close.find('(').map(|idx| before_close[idx + 1..].trim()));
            match paren_inner {
                Some(details) => details.to_string(),
                None => {
                    let clean = get_tool_clean_name(name)
                        .to_lowercase()
                        .replace([' ', '_'], "");
                    let norm_args = trimmed.to_lowercase().replace([' ', '_'], "");
                    let is_redundant = clean == norm_args
                        || (norm_args.len() >= 3
                            && (clean.contains(&norm_args) || norm_args.contains(&clean)));
                    match is_redundant {
                        true => String::new(),
                        false => trimmed.to_string(),
                    }
                }
            }
        }
    }
}

/// Generates the styled start indicator of a tool execution with tree prefixes.
pub fn get_tree_tool_start_msg(name: &str, formatted_args: &str) -> String {
    let prefix = get_tree_prefix(false);
    let clean_name = get_tool_clean_name(name);
    let details = clean_tool_args_msg(name, formatted_args);
    match details.is_empty() {
        true => format!(
            "{}{}{}{}• {}{}{}{}{}",
            colors::AURA_SLATE,
            prefix,
            colors::COLOR_RESET,
            colors::RED_ORANGE,
            colors::COLOR_RESET,
            colors::COLOR_BOLD,
            colors::LIGHT_WHITE,
            clean_name,
            colors::COLOR_RESET
        ),
        false => format!(
            "{}{}{}{}• {}{}{}{}{} {}{}{}",
            colors::AURA_SLATE,
            prefix,
            colors::COLOR_RESET,
            colors::RED_ORANGE,
            colors::COLOR_RESET,
            colors::COLOR_BOLD,
            colors::LIGHT_WHITE,
            clean_name,
            colors::COLOR_RESET,
            colors::AURA_SLATE,
            details,
            colors::COLOR_RESET
        ),
    }
}

/// Prints the colored start indicator of a tool execution with tree prefixes.
pub fn print_tree_tool_start(name: &str, formatted_args: &str) {
    let should_print = !is_silent() && !is_profile_subagent(name) && name != "parallel_research";
    should_print.then(|| {
        let output = get_tree_tool_start_msg(name, formatted_args);
        let replaced = output.replace("\r\n", "\n").replace('\n', "\r\n");
        std::print!("{}\r\n", replaced);
        let _ = std::io::stdout().flush();
    });
}

pub fn is_profile_subagent(name: &str) -> bool {
    crate::subagents::load_profiles()
        .map(|profiles| profiles.iter().any(|p| p.name == name))
        .unwrap_or(false)
}

pub fn get_command_error_summary(stdout: &str, stderr: &str) -> String {
    let combined = || stderr.lines().chain(stdout.lines()).map(str::trim);

    let compiler_err = combined().find_map(|trimmed| {
        trimmed
            .starts_with("error[E")
            .then(|| {
                trimmed.find("]: ").map(|pos| {
                    let code = &trimmed[..pos + 2];
                    let msg = &trimmed[pos + 3..];
                    replace_with_em_dash(&format!("compiler error \u{2014} {} {}", code, msg))
                })
            })
            .flatten()
            .or_else(|| {
                trimmed
                    .strip_prefix("error:")
                    .map(|msg| replace_with_em_dash(&format!("compiler error \u{2014} {}", msg.trim())))
            })
    });

    let failure_err = || {
        combined().find_map(|trimmed| {
            let has_timeout = trimmed.to_lowercase().contains("timeout");
            let has_err_kw = trimmed.contains("Failed")
                || trimmed.contains("Error:")
                || trimmed.contains("error ");
            match (has_timeout, has_err_kw) {
                (true, _) => Some(replace_with_em_dash("timeout not handled")),
                (false, true) => Some(replace_with_em_dash(trimmed)),
                (false, false) => None,
            }
        })
    };

    let first_non_empty = || {
        combined()
            .find(|trimmed| !trimmed.is_empty())
            .map(replace_with_em_dash)
    };

    compiler_err
        .or_else(failure_err)
        .or_else(first_non_empty)
        .unwrap_or_else(|| "command failed".to_string())
}

fn strip_list_bullet_prefix(trimmed: &str) -> &str {
    trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| {
            trimmed.find(". ").and_then(|pos| {
                (pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()))
                    .then_some(&trimmed[pos + 2..])
            })
        })
        .unwrap_or(trimmed)
}

pub fn escape_xml(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

pub fn escape_html(s: &str) -> String {
    escape_xml(s)
}

pub fn truncate_summary_text(summary: &str, max_chars: usize, suffix: &str) -> String {
    match summary.len() > max_chars {
        true => {
            let cut = crate::tools::headroom::floor_char_boundary(
                summary,
                max_chars.saturating_sub(suffix.chars().count()),
            );
            format!("{}{suffix}", &summary[..cut])
        }
        false => summary.to_string(),
    }
}

pub fn format_subagent_summary(content: &str) -> String {
    content
        .trim()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(strip_list_bullet_prefix)
        .map(|line| line.trim_matches('*').trim())
        .find(|line| !line.is_empty())
        .map(replace_with_em_dash)
        .map(|summary| truncate_summary_text(&summary, 80, "..."))
        .unwrap_or_else(|| "completed task".to_string())
}

pub fn format_reasoning_summary(reasoning: &str) -> String {
    reasoning
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            line.strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .unwrap_or(line)
        })
        .map(|line| line.trim_matches('*').trim())
        .find(|line| !line.is_empty())
        .map(replace_with_em_dash)
        .map(|summary| truncate_summary_text(&summary, 100, "..."))
        .unwrap_or_else(|| "analyzing requirements".to_string())
}

fn replace_with_em_dash(s: &str) -> String {
    s.replace(" - ", " \u{2014} ")
        .replace(" -- ", " \u{2014} ")
        .replace(" \u{2013} ", " \u{2014} ")
}

fn append_diagnostics_badge(base: String, res: &serde_json::Value) -> String {
    let unbalanced = res
        .get("diagnostics")
        .and_then(|d| d.get("balanced_delimiters"))
        .and_then(|v| v.as_bool())
        == Some(false);
    match unbalanced {
        true => format!("{} (\u{26a0} unbalanced delimiters)", base),
        false => base,
    }
}

pub fn format_tool_outcome_summary(
    name: &str,
    arguments: &serde_json::Value,
    res: &serde_json::Value,
) -> String {
    match name {
        "write_file" => {
            let line_count = res
                .get("line_count")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or_else(|| {
                    arguments
                        .get("content")
                        .or_else(|| arguments.get("code"))
                        .or_else(|| arguments.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .lines()
                        .count()
                });
            append_diagnostics_badge(format!("{} lines", line_count), res)
        }
        "patch_file" => {
            let patch_str = arguments
                .get("patch")
                .or_else(|| arguments.get("content"))
                .or_else(|| arguments.get("diff"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let base = match !patch_str.is_empty() {
                true => summarize_patch(patch_str),
                false => arguments
                    .get("replacement")
                    .or_else(|| arguments.get("new_text"))
                    .and_then(|v| v.as_str())
                    .and_then(|r| r.lines().map(str::trim).find(|l| !l.is_empty()))
                    .map(|first| format!("replace with {}", first))
                    .unwrap_or_else(|| "modified file".to_string()),
            };
            append_diagnostics_badge(base, res)
        }
        "replace_lines" => {
            let hunks = res
                .get("hunks_applied")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            let base = match hunks > 1 {
                true => {
                    let lines_mod = res
                        .get("lines_modified")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(hunks);
                    format!("{} hunks \u{00b7} {} lines", hunks, lines_mod)
                }
                false => {
                    let replacement = arguments
                        .get("replacement")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let trimmed = replacement.trim();
                    match (trimmed.is_empty(), trimmed.starts_with("use ")) {
                        (true, _) => "removed lines".to_string(),
                        (false, true) => {
                            let first = trimmed.split('\n').next().unwrap_or(trimmed).trim();
                            let cleaned = first.strip_suffix(';').unwrap_or(first);
                            format!("add {}", cleaned)
                        }
                        (false, false) => {
                            let first_line = trimmed.split('\n').next().unwrap_or(trimmed).trim();
                            format!("replace with {}", first_line)
                        }
                    }
                }
            };
            append_diagnostics_badge(base, res)
        }
        "tool_catalog" => {
            let batch_count = res
                .get("batch_executed_count")
                .and_then(|v| v.as_u64())
                .filter(|&n| n > 0);
            let mounted_count = res
                .get("mounted_tools")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .filter(|&n| n > 0);
            match (batch_count, mounted_count) {
                (Some(n), _) => {
                    let mode = res
                        .get("batch_execution_mode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("sequential");
                    format!(
                        "{}\u{2713} batch: {} tools ({}){}",
                        colors::AURA_GREEN,
                        n,
                        mode,
                        colors::COLOR_RESET
                    )
                }
                (None, Some(m)) => format!(
                    "{}\u{2713} mounted {} tools{}",
                    colors::AURA_GREEN,
                    m,
                    colors::COLOR_RESET
                ),
                (None, None) => format!("{}\u{2713} completed{}", colors::AURA_GREEN, colors::COLOR_RESET),
            }
        }
        "knowledge_source" => {
            let action = arguments
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match action {
                "add" => {
                    let label = res
                        .get("source")
                        .and_then(|v| v.get("label"))
                        .and_then(|v| v.as_str())
                        .or_else(|| arguments.get("label").and_then(|v| v.as_str()))
                        .unwrap_or("source");
                    format!("Source saved: {}", label)
                }
                "search" => {
                    let count = res
                        .get("matches")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.len())
                        .unwrap_or(0);
                    format!("Sources matched: {}", count)
                }
                "delete" => "Source deleted".to_string(),
                "mark_checked" => "Source refreshed".to_string(),
                _ => "Source memory updated".to_string(),
            }
        }
        "research_brief" => {
            let action = arguments
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match action {
                "save" => {
                    let topic = res
                        .get("brief")
                        .and_then(|v| v.get("topic"))
                        .and_then(|v| v.as_str())
                        .or_else(|| arguments.get("topic").and_then(|v| v.as_str()))
                        .unwrap_or("brief");
                    format!("Research brief saved: {}", topic)
                }
                "search" => {
                    let count = res
                        .get("matches")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.len())
                        .unwrap_or(0);
                    format!("Research briefs matched: {}", count)
                }
                "delete" => "Research brief deleted".to_string(),
                _ => "Research brief updated".to_string(),
            }
        }
        "workflow_memory" => {
            let action = arguments
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match action {
                "add" => {
                    let name = res
                        .get("workflow")
                        .and_then(|v| v.get("name"))
                        .and_then(|v| v.as_str())
                        .or_else(|| arguments.get("name").and_then(|v| v.as_str()))
                        .unwrap_or("workflow");
                    format!("Workflow saved: {}", name)
                }
                "search" => {
                    let count = res
                        .get("matches")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.len())
                        .unwrap_or(0);
                    format!("Workflows matched: {}", count)
                }
                "record_run" => {
                    let success_count = res
                        .get("workflow")
                        .and_then(|v| v.get("success_count"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    format!("Workflow updated: success {}", success_count)
                }
                "activate" => "Workflow activated".to_string(),
                "deactivate" => "Workflow disabled".to_string(),
                "delete" => "Workflow deleted".to_string(),
                _ => "Workflow memory updated".to_string(),
            }
        }
        "exec_command" => {
            let status_code = res.get("status_code").and_then(|v| v.as_i64()).unwrap_or(0);
            let command_str = arguments
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match status_code == 0 {
                true => {
                    let summary = [
                        ("build", "build succeeded"),
                        ("test", "all tests passing"),
                        ("clippy", "clippy passed"),
                    ]
                    .iter()
                    .find_map(|&(kw, label)| command_str.contains(kw).then_some(label))
                    .unwrap_or("command succeeded");
                    format!(
                        "{}\u{2713} {}{}",
                        colors::AURA_GREEN,
                        summary,
                        colors::COLOR_RESET
                    )
                }
                false => {
                    let stdout = res
                        .get("stdout")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let stderr = res
                        .get("stderr")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let err_summary = get_command_error_summary(stdout, stderr);
                    format!(
                        "{}\u{2715} {}{}",
                        colors::AURA_ROSE,
                        err_summary,
                        colors::COLOR_RESET
                    )
                }
            }
        }
        _ => match res.get("error").and_then(|v| v.as_str()) {
            Some(err) => format!(
                "{}\u{2715} Failed: {}{}",
                colors::AURA_ROSE,
                err,
                colors::COLOR_RESET
            ),
            None => format!(
                "{}\u{2713} completed{}",
                colors::AURA_GREEN,
                colors::COLOR_RESET
            ),
        },
    }
}

fn summarize_patch(patch_str: &str) -> String {
    let first_added = patch_str
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .map(|line| line[1..].trim())
        .find(|trimmed| !trimmed.is_empty());
    let first_removed = patch_str
        .lines()
        .filter(|line| line.starts_with('-') && !line.starts_with("---"))
        .map(|line| line[1..].trim())
        .find(|trimmed| !trimmed.is_empty());

    match (first_added, first_removed) {
        (Some(added), _) => {
            let cleaned = added
                .starts_with("use ")
                .then(|| added.strip_suffix(';'))
                .flatten()
                .unwrap_or(added);
            format!("add {}", cleaned)
        }
        (None, Some(removed)) => format!("remove {}", removed),
        (None, None) => "modified file".to_string(),
    }
}

fn split_ascii_word_into_chunks(word: &str, max_width: usize) -> (Vec<String>, String) {
    let mut chunks = Vec::new();
    let mut temp = word;
    while temp.len() > max_width {
        let (left, right) = temp.split_at(max_width);
        chunks.push(left.to_string());
        temp = right;
    }
    (chunks, temp.to_string())
}

/// Wraps a single line of text into multiple lines, none exceeding `max_width`.
pub fn wrap_line(line: &str, max_width: usize) -> Vec<String> {
    match max_width {
        0 => vec![line.to_string()],
        _ => {
            let (mut lines, rem_line) = line.split_whitespace().fold(
                (Vec::<String>::new(), String::new()),
                |(mut acc, cur), word| {
                    let fits_cur = !cur.is_empty() && (cur.len() + 1 + word.len() <= max_width);
                    let fits_fresh = word.len() <= max_width;
                    match (cur.is_empty(), fits_cur, fits_fresh) {
                        (true, _, true) => (acc, word.to_string()),
                        (true, _, false) => {
                            let (chunks, tail) = split_ascii_word_into_chunks(word, max_width);
                            acc.extend(chunks);
                            (acc, tail)
                        }
                        (false, true, _) => {
                            let mut next = cur;
                            next.push(' ');
                            next.push_str(word);
                            (acc, next)
                        }
                        (false, false, true) => {
                            acc.push(cur);
                            (acc, word.to_string())
                        }
                        (false, false, false) => {
                            acc.push(cur);
                            let (chunks, tail) = split_ascii_word_into_chunks(word, max_width);
                            acc.extend(chunks);
                            (acc, tail)
                        }
                    }
                },
            );
            (!rem_line.is_empty()).then(|| lines.push(rem_line));
            lines.is_empty().then(|| lines.push(String::new()));
            lines
        }
    }
}

/// Prints a monologue/thought block with the tree prefix, wrapping long lines and aligning wrapped sub-lines after the prefix.
pub fn print_tree_monologue(leaf_prefix: &str, text: &str) {
    let terminal_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80) as usize;
    let prefix_len = leaf_prefix.chars().count();
    let max_width = terminal_width.saturating_sub(prefix_len);
    let indent_pad = " ".repeat(prefix_len);

    let _ = text.trim().lines().fold(true, |mut is_first_line, line| {
        let trimmed = line.trim();
        match trimmed.is_empty() {
            true => {
                crate::tui_println!("{}{}{}", colors::AURA_SLATE, indent_pad, colors::COLOR_RESET);
                is_first_line
            }
            false => {
                for sub_line in wrap_line(trimmed, max_width) {
                    let active_prefix = match is_first_line {
                        true => leaf_prefix,
                        false => indent_pad.as_str(),
                    };
                    crate::tui_println!(
                        "{}{}{}{}",
                        colors::AURA_SLATE,
                        active_prefix,
                        sub_line,
                        colors::COLOR_RESET
                    );
                    is_first_line = false;
                }
                is_first_line
            }
        }
    });
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
