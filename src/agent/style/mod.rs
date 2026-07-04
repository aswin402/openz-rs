pub mod colors;
pub mod icons;
pub mod spinner;
pub mod menu;

pub use colors::*;
pub use icons::*;
pub use spinner::*;
pub use menu::*;

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

pub fn tui_println_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}\r\n", replaced);
    let _ = std::io::stdout().flush();
}

pub fn tui_print_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}", replaced);
    let _ = std::io::stdout().flush();
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

pub fn tui_eprintln_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    eprint!("{}\r\n", replaced);
    let _ = std::io::stderr().flush();
}

pub fn tui_eprint_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    eprint!("{}", replaced);
    let _ = std::io::stderr().flush();
}

/// Returns the prefix string for a tree trace line at the specified delegation depth.
pub fn get_tree_prefix_for_depth(is_leaf: bool, depth: usize) -> String {
    if depth > 0 {
        // For subagent tools, both starts and outcomes get the "L " connector
        format!("{}L ", "  ".repeat(depth))
    } else {
        // Root agent depth (depth == 0)
        if is_leaf {
            "  L ".to_string()
        } else {
            "".to_string()
        }
    }
}

/// Returns the prefix string for a tree trace line at the current delegation depth.
pub fn get_tree_prefix(is_leaf: bool) -> String {
    let depth = crate::tools::subagent::DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
    get_tree_prefix_for_depth(is_leaf, depth)
}

/// Converts a tool name into a clean, visual title.
pub fn get_tool_clean_name(name: &str) -> String {
    match name {
        "exec_command" => "Bash".to_string(),
        "write_file" => "Write".to_string(),
        "patch_file" | "replace_lines" => "Edit".to_string(),
        "read_file" => "Read".to_string(),
        _ => {
            let mut result = Vec::new();
            for word in name.split('_') {
                if word.is_empty() {
                    continue;
                }
                let mut chars = word.chars();
                if let Some(first) = chars.next() {
                    let capitalized = first.to_uppercase().to_string() + chars.as_str();
                    result.push(capitalized);
                }
            }
            result.join(" ")
        }
    }
}

/// Generates a styled spinner message indicating a tool is running under the tree bullet.
pub fn get_tree_spinner_msg(_name: &str, _formatted_args: &str) -> String {
    let prefix = get_tree_prefix(true);
    format!("{}{}{}Running...{}", colors::AURA_SLATE, prefix, colors::AURA_SLATE, colors::COLOR_RESET)
}

/// Strips standard graphic SGR ANSI escape sequences (\x1b[...m) from a string.
pub fn strip_ansi_escapes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    let chars = s.chars();
    
    for c in chars {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Helper to clean up friendly name duplication from formatted tool arguments.
pub fn clean_tool_args_msg(name: &str, formatted_args: &str) -> String {
    let stripped = strip_ansi_escapes(formatted_args);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    
    // Check if it ends with ')' and contains '('
    if trimmed.ends_with(')') {
        if let Some(open_idx) = trimmed.find('(') {
            let details = &trimmed[open_idx + 1..trimmed.len() - 1];
            return details.trim().to_string();
        }
    }
    
    // If it doesn't have parentheses, check if it is just a friendly name variant of the tool.
    let clean = get_tool_clean_name(name).to_lowercase().replace(" ", "").replace("_", "");
    let norm_args = trimmed.to_lowercase().replace(" ", "").replace("_", "");
    
    if clean == norm_args || (norm_args.len() >= 3 && (clean.contains(&norm_args) || norm_args.contains(&clean))) {
        String::new()
    } else {
        trimmed.to_string()
    }
}

/// Generates the styled start indicator of a tool execution with tree prefixes.
pub fn get_tree_tool_start_msg(name: &str, formatted_args: &str) -> String {
    let prefix = get_tree_prefix(false);
    let clean_name = get_tool_clean_name(name);
    let details = clean_tool_args_msg(name, formatted_args);
    if details.is_empty() {
        format!(
            "{}{}{}{}● {}{}{}{}{}",
            colors::AURA_SLATE, prefix, colors::COLOR_RESET,
            colors::RED_ORANGE,
            colors::COLOR_RESET,
            colors::COLOR_BOLD, colors::LIGHT_WHITE, clean_name, colors::COLOR_RESET
        )
    } else {
        format!(
            "{}{}{}{}● {}{}{}{}{} {}{}{}",
            colors::AURA_SLATE, prefix, colors::COLOR_RESET,
            colors::RED_ORANGE,
            colors::COLOR_RESET,
            colors::COLOR_BOLD, colors::LIGHT_WHITE, clean_name, colors::COLOR_RESET,
            colors::AURA_SLATE, details, colors::COLOR_RESET
        )
    }
}

/// Prints the colored start indicator of a tool execution with tree prefixes.
pub fn print_tree_tool_start(name: &str, formatted_args: &str) {
    if is_silent() {
        return;
    }
    if is_profile_subagent(name) || name == "parallel_research" {
        return;
    }
    let output = get_tree_tool_start_msg(name, formatted_args);
    let replaced = output.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}\r\n", replaced);
    let _ = std::io::stdout().flush();
}

pub fn is_profile_subagent(name: &str) -> bool {
    if let Ok(profiles) = crate::subagents::load_profiles() {
        profiles.iter().any(|p| p.name == name)
    } else {
        false
    }
}

pub fn get_command_error_summary(stdout: &str, stderr: &str) -> String {
    for line in stderr.lines().chain(stdout.lines()) {
        let trimmed = line.trim();
        if trimmed.starts_with("error[E") {
            if let Some(pos) = trimmed.find("]: ") {
                let code = &trimmed[..pos + 2];
                let msg = &trimmed[pos + 3..];
                return replace_with_em_dash(&format!("compiler error \u{2014} {} {}", code, msg));
            }
        } else if trimmed.starts_with("error:") {
            let msg = trimmed.strip_prefix("error:").unwrap().trim();
            return replace_with_em_dash(&format!("compiler error \u{2014} {}", msg));
        }
    }
    for line in stderr.lines().chain(stdout.lines()) {
        let trimmed = line.trim();
        if trimmed.to_lowercase().contains("timeout") {
            return replace_with_em_dash("timeout not handled");
        }
        if trimmed.contains("Failed") || trimmed.contains("Error:") || trimmed.contains("error ") {
            return replace_with_em_dash(trimmed);
        }
    }
    for line in stderr.lines().chain(stdout.lines()) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return replace_with_em_dash(trimmed);
        }
    }
    "command failed".to_string()
}

pub fn format_subagent_summary(content: &str) -> String {
    let cleaned = content.trim();
    for line in cleaned.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut final_line = trimmed;
        if final_line.starts_with("- ") {
            final_line = &final_line[2..];
        } else if final_line.starts_with("* ") {
            final_line = &final_line[2..];
        } else if let Some(pos) = final_line.find(". ") {
            if final_line[..pos].chars().all(|c| c.is_ascii_digit()) {
                final_line = &final_line[pos + 2..];
            }
        }
        
        let final_line = final_line.trim_matches('*').trim();
        if !final_line.is_empty() {
            let summary = replace_with_em_dash(final_line);
            if summary.len() > 80 {
                return format!("{}...", &summary[..77]);
            }
            return summary;
        }
    }
    "completed task".to_string()
}

pub fn format_reasoning_summary(reasoning: &str) -> String {
    for line in reasoning.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut cleaned = trimmed;
        if cleaned.starts_with("- ") {
            cleaned = &cleaned[2..];
        } else if cleaned.starts_with("* ") {
            cleaned = &cleaned[2..];
        }
        let cleaned = cleaned.trim_matches('*').trim();
        if !cleaned.is_empty() {
            let summary = replace_with_em_dash(cleaned);
            if summary.len() > 100 {
                return format!("{}...", &summary[..97]);
            }
            return summary;
        }
    }
    "analyzing requirements".to_string()
}

fn replace_with_em_dash(s: &str) -> String {
    s.replace(" - ", " \u{2014} ")
     .replace(" -- ", " \u{2014} ")
     .replace(" \u{2013} ", " \u{2014} ")
}

pub fn format_tool_outcome_summary(name: &str, arguments: &serde_json::Value, res: &serde_json::Value) -> String {
    match name {
        "write_file" => {
            let content = arguments.get("content")
                .or(arguments.get("code"))
                .or(arguments.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let line_count = content.lines().count();
            format!("{} lines", line_count)
        }
        "patch_file" => {
            let patch_str = arguments.get("patch")
                .or(arguments.get("content"))
                .or(arguments.get("diff"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            summarize_patch(patch_str)
        }
        "replace_lines" => {
            let replacement = arguments.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
            let trimmed = replacement.trim();
            if trimmed.is_empty() {
                "removed lines".to_string()
            } else if trimmed.starts_with("use ") {
                let cleaned = trimmed.split('\n').next().unwrap_or(trimmed).trim();
                let cleaned = cleaned.strip_suffix(';').unwrap_or(cleaned);
                format!("add {}", cleaned)
            } else {
                let first_line = trimmed.split('\n').next().unwrap_or(trimmed).trim();
                format!("replace with {}", first_line)
            }
        }
        "exec_command" => {
            let status_code = res.get("status_code").and_then(|v| v.as_i64()).unwrap_or(0);
            let command_str = arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
            if status_code == 0 {
                let summary = if command_str.contains("build") {
                    "build succeeded"
                } else if command_str.contains("test") {
                    "all tests passing"
                } else if command_str.contains("clippy") {
                    "clippy passed"
                } else {
                    "command succeeded"
                };
                format!("{}\u{2713} {}{}", colors::AURA_GREEN, summary, colors::COLOR_RESET)
            } else {
                let stdout = res.get("stdout").and_then(|v| v.as_str()).unwrap_or_default();
                let stderr = res.get("stderr").and_then(|v| v.as_str()).unwrap_or_default();
                let err_summary = get_command_error_summary(stdout, stderr);
                format!("{}\u{2715} {}{}", colors::AURA_ROSE, err_summary, colors::COLOR_RESET)
            }
        }
        _ => {
            if let Some(err) = res.get("error").and_then(|v| v.as_str()) {
                format!("{}\u{2715} Failed: {}{}", colors::AURA_ROSE, err, colors::COLOR_RESET)
            } else {
                format!("{}\u{2713} completed{}", colors::AURA_GREEN, colors::COLOR_RESET)
            }
        }
    }
}

fn summarize_patch(patch_str: &str) -> String {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for line in patch_str.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            let trimmed = line[1..].trim();
            if !trimmed.is_empty() {
                added.push(trimmed);
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            let trimmed = line[1..].trim();
            if !trimmed.is_empty() {
                removed.push(trimmed);
            }
        }
    }
    if !added.is_empty() {
        let first = added[0];
        if first.starts_with("use ") {
            let cleaned = first.strip_suffix(';').unwrap_or(first);
            format!("add {}", cleaned)
        } else {
            format!("add {}", first)
        }
    } else if !removed.is_empty() {
        format!("remove {}", removed[0])
    } else {
        "modified file".to_string()
    }
}

/// Wraps a single line of text into multiple lines, none exceeding `max_width`.
/// It performs word wrapping on spaces, falling back to character wrapping if a word is longer than `max_width`.
pub fn wrap_line(line: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![line.to_string()];
    }
    let mut lines = Vec::new();
    let mut current_line = String::new();
    
    for word in line.split_whitespace() {
        if current_line.is_empty() {
            if word.len() <= max_width {
                current_line.push_str(word);
            } else {
                // Word is too long, must split it by characters
                let mut temp = word;
                while temp.len() > max_width {
                    let (left, right) = temp.split_at(max_width);
                    lines.push(left.to_string());
                    temp = right;
                }
                current_line.push_str(temp);
            }
        } else {
            // Check if adding the word fits
            if current_line.len() + 1 + word.len() <= max_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = String::new();
                if word.len() <= max_width {
                    current_line.push_str(word);
                } else {
                    let mut temp = word;
                    while temp.len() > max_width {
                        let (left, right) = temp.split_at(max_width);
                        lines.push(left.to_string());
                        temp = right;
                    }
                    current_line.push_str(temp);
                }
            }
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Prints a monologue/thought block with the tree prefix, wrapping long lines and aligning wrapped sub-lines after the prefix.
pub fn print_tree_monologue(leaf_prefix: &str, text: &str) {
    let terminal_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80) as usize;
    let prefix_len = leaf_prefix.chars().count();
    let max_width = terminal_width.saturating_sub(prefix_len);
    
    let mut is_first_line = true;
    for line in text.trim().lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            crate::tui_println!("{}{}{}", colors::AURA_SLATE, " ".repeat(prefix_len), colors::COLOR_RESET);
            continue;
        }
        let wrapped = wrap_line(trimmed, max_width);
        for sub_line in wrapped {
            if is_first_line {
                crate::tui_println!("{}{}{}{}", colors::AURA_SLATE, leaf_prefix, sub_line, colors::COLOR_RESET);
                is_first_line = false;
            } else {
                crate::tui_println!("{}{}{}{}", colors::AURA_SLATE, " ".repeat(prefix_len), sub_line, colors::COLOR_RESET);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_clean_names() {
        assert_eq!(get_tool_clean_name("exec_command"), "Bash");
        assert_eq!(get_tool_clean_name("write_file"), "Write");
        assert_eq!(get_tool_clean_name("patch_file"), "Edit");
        assert_eq!(get_tool_clean_name("replace_lines"), "Edit");
        assert_eq!(get_tool_clean_name("read_file"), "Read");
        assert_eq!(get_tool_clean_name("delegate_task"), "Delegate Task");
        assert_eq!(get_tool_clean_name("my_custom_tool"), "My Custom Tool");
        assert_eq!(get_tool_clean_name("a_b_c"), "A B C");
    }

    #[test]
    fn test_tree_prefix_depth_0() {
        // Without scoping, DELEGATION_DEPTH should default to 0
        assert_eq!(get_tree_prefix(true), "  L ");
        assert_eq!(get_tree_prefix(false), "");
        
        let spinner = get_tree_spinner_msg("test", "");
        assert!(spinner.contains("  L "));
        assert!(spinner.contains("Running..."));
        assert!(spinner.contains(colors::AURA_SLATE));
    }

    #[tokio::test]
    async fn test_tree_prefix_nested() {
        crate::tools::subagent::DELEGATION_DEPTH.scope(1, async {
            assert_eq!(get_tree_prefix(true), "  L ");
            assert_eq!(get_tree_prefix(false), "  L ");
            
            let spinner = get_tree_spinner_msg("test", "");
            assert!(spinner.contains("  L "));
            assert!(spinner.contains("Running..."));
        }).await;

        crate::tools::subagent::DELEGATION_DEPTH.scope(2, async {
            assert_eq!(get_tree_prefix(true), "    L ");
            assert_eq!(get_tree_prefix(false), "    L ");
            
            let spinner = get_tree_spinner_msg("test", "");
            assert!(spinner.contains("    L "));
            assert!(spinner.contains("Running..."));
        }).await;

        crate::tools::subagent::DELEGATION_DEPTH.scope(3, async {
            assert_eq!(get_tree_prefix(true), "      L ");
            assert_eq!(get_tree_prefix(false), "      L ");
            
            let spinner = get_tree_spinner_msg("test", "");
            assert!(spinner.contains("      L "));
            assert!(spinner.contains("Running..."));
        }).await;
    }

    #[test]
    fn test_tree_tool_start_msg() {
        // Without args
        let msg = get_tree_tool_start_msg("exec_command", "");
        assert!(msg.contains("Bash"));
        assert!(msg.contains('●'));
        
        let bullet_idx = msg.find('●').unwrap();
        // RED_ORANGE should precede bullet
        assert!(msg[..bullet_idx].ends_with(colors::RED_ORANGE));
        // COLOR_RESET should follow bullet (followed by a space)
        let after_bullet = &msg[bullet_idx + '●'.len_utf8()..];
        assert!(after_bullet.starts_with(" "));
        assert!(after_bullet[1..].starts_with(colors::COLOR_RESET));

        // With args
        let msg_args = get_tree_tool_start_msg("write_file", "--force");
        assert!(msg_args.contains("Write"));
        assert!(msg_args.contains("--force"));
        assert!(msg_args.contains('●'));
        
        let bullet_idx_args = msg_args.find('●').unwrap();
        assert!(msg_args[..bullet_idx_args].ends_with(colors::RED_ORANGE));
        let after_bullet_args = &msg_args[bullet_idx_args + '●'.len_utf8()..];
        assert!(after_bullet_args.starts_with(" "));
        assert!(after_bullet_args[1..].starts_with(colors::COLOR_RESET));
    }

    #[test]
    fn test_wrap_line() {
        let line = "hello world this is a test of wrapping";
        let wrapped = wrap_line(line, 10);
        assert_eq!(wrapped, vec!["hello", "world this", "is a test", "of", "wrapping"]);

        let long_word = "supercalifragilistic";
        let wrapped_long = wrap_line(long_word, 10);
        assert_eq!(wrapped_long, vec!["supercalif", "ragilistic"]);
    }

    #[test]
    fn test_clean_tool_args_msg() {
        assert_eq!(clean_tool_args_msg("web_search", "\x1b[1mWebSearch\x1b[0m"), "");
        assert_eq!(clean_tool_args_msg("web_search", "\x1b[1mWebSearch\x1b[0m(\x1b[38;2;107;122;153mquery: \"ZeroClaw\"\x1b[0m)"), "query: \"ZeroClaw\"");
        assert_eq!(clean_tool_args_msg("exec_command", "\x1b[1mBash\x1b[0m(cargo build)"), "cargo build");
        assert_eq!(clean_tool_args_msg("write_file", "--force"), "--force");
    }

    #[test]
    fn test_strip_ansi_escapes() {
        assert_eq!(strip_ansi_escapes("\x1b[1mRead\x1b[0m"), "Read");
        assert_eq!(strip_ansi_escapes("\x1b[38;2;255;0;0mError\x1b[0m details"), "Error details");
    }
}


