use regex::Regex;
use serde_json::Value;

pub fn compress_json(raw_json: &str) -> anyhow::Result<String> {
    let value: Value = serde_json::from_str(raw_json)?;
    if let Value::Array(arr) = value {
        if arr.is_empty() {
            return Ok("[]".to_string());
        }
        let total_count = arr.len();
        let mut keys = std::collections::BTreeSet::new();
        for item in &arr {
            if let Value::Object(map) = item {
                for k in map.keys() {
                    keys.insert(k.clone());
                }
            }
        }
        
        let keys_str = keys.into_iter().collect::<Vec<String>>().join(", ");
        let first_item_str = serde_json::to_string_pretty(&arr[0]).unwrap_or_default();
        
        Ok(format!(
            "[JSON Array: {} objects. Keys: [{}]. \nFirst element:\n{}]",
            total_count, keys_str, first_item_str
        ))
    } else {
        let minified = serde_json::to_string(&value)?;
        if minified.len() > 1000 {
            Ok(format!("{}...", minified.chars().take(1000).collect::<String>()))
        } else {
            Ok(minified)
        }
    }
}

fn re_block() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)/\*.*?\*/").unwrap())
}

fn re_line() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)(^|[^:])//.*").unwrap())
}

fn re_lines() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n\s*\n").unwrap())
}

fn re_ansi() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\x1B\[[0-9;]*[a-zA-Z]").unwrap())
}

fn re_backtrace_line() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*at\s+|^\s*\d+:\s+").unwrap())
}

fn re_rust_backtrace() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"stack backtrace:|backtrace::").unwrap())
}

fn re_cargo_warning() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)warning:").unwrap())
}

fn re_cargo_error() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)error\[E\d+\]:|error:").unwrap())
}

pub fn compress_code(raw_code: &str) -> String {
    let no_blocks = re_block().replace_all(raw_code, "");
    let no_comments = re_line().replace_all(&no_blocks, "$1");
    let collapsed = re_lines().replace_all(&no_comments, "\n");
    collapsed.trim().to_string()
}

pub fn compress_logs(raw_logs: &str) -> String {
    let clean_logs = re_ansi().replace_all(raw_logs, "");

    let mut filtered_lines = Vec::new();
    let mut warning_count = 0;
    let mut error_count = 0;
    let mut is_backtrace = false;

    let re_backtrace_line = re_backtrace_line();
    let re_rust_backtrace = re_rust_backtrace();
    let re_cargo_warning = re_cargo_warning();
    let re_cargo_error = re_cargo_error();

    for line in clean_logs.lines() {
        let trimmed = line.trim();
        
        if re_rust_backtrace.is_match(trimmed) {
            is_backtrace = true;
            filtered_lines.push("[Backtrace detected - stripping stack frames for token reduction]".to_string());
            continue;
        }
        if is_backtrace {
            if trimmed.is_empty() {
                is_backtrace = false;
            } else if re_backtrace_line.is_match(trimmed) || trimmed.starts_with("frame #") || trimmed.starts_with("at ") {
                continue;
            }
        }

        if re_cargo_warning.is_match(trimmed) {
            warning_count += 1;
            if warning_count > 10 {
                continue;
            }
        }
        
        if re_cargo_error.is_match(trimmed) {
            error_count += 1;
            if error_count > 5 {
                continue;
            }
        }

        filtered_lines.push(line.to_string());
    }

    let mut filtered_logs = filtered_lines.join("\n");
    if warning_count > 10 {
        filtered_logs.push_str(&format!("\n... [Skipped {} additional cargo warnings to save tokens] ...", warning_count - 10));
    }
    if error_count > 5 {
        filtered_logs.push_str(&format!("\n... [Skipped {} additional cargo errors to save tokens] ...", error_count - 5));
    }

    if filtered_logs.len() > 2000 {
        let first_part: String = filtered_logs.chars().take(1000).collect();
        let last_part: String = filtered_logs.chars().skip(filtered_logs.chars().count().saturating_sub(1000)).collect();
        format!(
            "{}\n\n... [TRUNCATED LOGS] ...\n\n{}",
            first_part, last_part
        )
    } else {
        filtered_logs
    }
}

pub fn compress_tool_output(tool_name: &str, raw_output: &str) -> String {
    let raw_trimmed = raw_output.trim();
    if raw_trimmed.is_empty() {
        return "Empty output.".to_string();
    }

    if raw_trimmed.starts_with('[') || raw_trimmed.starts_with('{') {
        if let Ok(compressed) = compress_json(raw_trimmed) {
            return compressed;
        }
    }

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

    if is_code_tool {
        // Only strip comments/whitespace if it exceeds a reasonable size
        if raw_trimmed.len() > 2000 {
            return compress_code(raw_trimmed);
        }
    }

    compress_logs(raw_trimmed)
}
