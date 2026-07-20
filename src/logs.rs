use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

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

use std::sync::OnceLock;

pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
    pub session: Option<String>,
}

pub static LOG_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<LogEntry>> = OnceLock::new();

pub fn default_db_path() -> PathBuf {
    crate::config::config_dir().join("logs.db")
}

pub async fn init_db_writer(mut rx: tokio::sync::mpsc::UnboundedReceiver<LogEntry>) {
    let db_path = default_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    tokio::task::spawn_blocking(move || {
        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to open logs.db: {}", e);
                return;
            }
        };

        if let Err(e) = conn.execute(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                message TEXT NOT NULL,
                session TEXT
            )",
            [],
        ) {
            eprintln!("Failed to create logs table: {}", e);
            return;
        }

        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_logs_session ON logs (session)", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs (timestamp)", []);

        while let Some(entry) = rx.blocking_recv() {
            let _ = conn.execute(
                "INSERT INTO logs (timestamp, level, target, message, session) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    entry.timestamp,
                    entry.level,
                    entry.target,
                    entry.message,
                    entry.session,
                ],
            );
        }
    });
}

pub struct SqliteLogLayer;

impl<S> tracing_subscriber::Layer<S> for SqliteLogLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target().to_string();

        let mut visitor = EventFieldVisitor {
            message: String::new(),
            session: None,
        };
        event.record(&mut visitor);

        let session = visitor.session.or_else(|| crate::agent::style::spinner::get_current_session_key());
        let timestamp = chrono::Utc::now().to_rfc3339();

        if let Some(tx) = LOG_TX.get() {
            let _ = tx.send(LogEntry {
                timestamp,
                level,
                target,
                message: visitor.message,
                session,
            });
        }
    }
}

struct EventFieldVisitor {
    message: String,
    session: Option<String>,
}

impl tracing::field::Visit for EventFieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let val_str = format!("{:?}", value);
        let cleaned = if val_str.starts_with('"') && val_str.ends_with('"') && val_str.len() >= 2 {
            val_str[1..val_str.len() - 1].to_string()
        } else {
            val_str
        };
        if field.name() == "message" {
            self.message = cleaned;
        } else if field.name() == "session" {
            self.session = Some(cleaned);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else if field.name() == "session" {
            self.session = Some(value.to_string());
        }
    }
}

/// Resolve the default log file path: ~/.openz/openz.log (or OPENZ_CONFIG_DIR/openz.log)
pub fn default_log_path() -> PathBuf {
    crate::config::config_dir().join("openz.log")
}

/// Which sessions to show.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionFilter {
    /// Show all sessions (no filter).
    All,
    /// Show only lines that match this session key (prefix match).
    Only(String),
    /// Automatically follow the most recently active session
    Auto(Option<String>),
}

impl SessionFilter {
    /// Build from an optional CLI `--session` string.
    pub fn from_opt(s: Option<&str>) -> Self {
        match s {
            None => SessionFilter::All,
            Some("auto") => SessionFilter::Auto(detect_active_session()),
            Some(k) if k.is_empty() => SessionFilter::All,
            Some(k) => SessionFilter::Only(k.to_string()),
        }
    }

    /// Return a short label for the header banner.
    pub fn label(&self) -> String {
        match self {
            SessionFilter::All => "all sessions".to_string(),
            SessionFilter::Only(k) => format!("session: {}", k),
            SessionFilter::Auto(None) => "session: auto (detecting...)".to_string(),
            SessionFilter::Auto(Some(k)) => format!("session: auto ({})", k),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevelFilter {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevelFilter {
    pub fn from_opt(s: Option<&str>) -> Self {
        match s.map(|x| x.to_uppercase()).as_deref() {
            Some("ERROR") => LogLevelFilter::Error,
            Some("WARN") => LogLevelFilter::Warn,
            Some("INFO") => LogLevelFilter::Info,
            Some("DEBUG") => LogLevelFilter::Debug,
            Some("TRACE") => LogLevelFilter::Trace,
            _ => LogLevelFilter::Trace,
        }
    }

    pub fn matches(&self, level_str: &str) -> bool {
        let line_level = match level_str.to_uppercase().as_str() {
            "ERROR" => LogLevelFilter::Error,
            "WARN" => LogLevelFilter::Warn,
            "INFO" => LogLevelFilter::Info,
            "DEBUG" => LogLevelFilter::Debug,
            "TRACE" => LogLevelFilter::Trace,
            _ => return true,
        };
        line_level >= *self
    }
}

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
            .find(|c: char| c == ' ' || c == ',' || c == '}' || c == ']' || c == '\n' || c == ')')
            .unwrap_or(val_slice.len());
        let val = val_slice[..end].trim().to_string();
        if !val.is_empty() && !sessions.contains(&val) {
            sessions.push(val);
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
    if line.is_empty() {
        return None;
    }

    // timestamp is the first token (ISO 8601, ends with 'Z')
    let (ts, rest) = line.split_once(' ')?;
    if !ts.ends_with('Z') {
        return None;
    }

    // level is the next word
    let rest = rest.trim_start();
    let (level, rest) = rest.split_once(' ')?;
    let level = level.trim();

    // target ends at the first ': '
    let rest = rest.trim_start();
    let (target, message_raw) = if let Some(idx) = rest.find(": ") {
        (&rest[..idx], &rest[idx + 2..])
    } else {
        ("", rest)
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
/// Returns `(original, None)` if the field is absent.
fn extract_session_field(raw: &str) -> (&str, Option<String>) {
    // Look for `session=` as a trailing span field (tracing appends these at the end).
    // It may be preceded by at least one space or appear after other fields.
    if let Some(pos) = raw.rfind(" session=") {
        let value = raw[pos + " session=".len()..].trim().to_string();
        let msg = raw[..pos].trim_end();
        (msg, Some(value))
    } else if let Some(stripped) = raw.strip_prefix("session=") {
        // Edge: no message body, only the field
        let value = stripped.trim().to_string();
        ("", Some(value))
    } else {
        (raw, None)
    }
}

/// Returns true if any of the line's sessions match the filter.
fn session_matches(line_sessions: &[String], filter: &SessionFilter) -> bool {
    match filter {
        SessionFilter::All => true,
        SessionFilter::Only(wanted) => {
            if line_sessions.is_empty() {
                // Lines without a session tag predate the feature — always show them
                // so old history is not silently dropped.
                true
            } else {
                line_sessions.iter().any(|s| s.starts_with(wanted.as_str()))
            }
        }
        SessionFilter::Auto(opt_wanted) => match opt_wanted {
            None => true,
            Some(wanted) => {
                if line_sessions.is_empty() {
                    true
                } else {
                    line_sessions.iter().any(|s| s.starts_with(wanted.as_str()))
                }
            }
        },
    }
}

// ── Pretty-print a single line ──────────────────────────────────────────────

fn highlight_message(msg: &str) -> String {
    static RE_KEY: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    let re_key = RE_KEY
        .get_or_init(|| regex::Regex::new(r#""([^"]+)":\s*"#).ok())
        .as_ref();

    static RE_VAL_NUM: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    let re_val_num = RE_VAL_NUM
        .get_or_init(|| regex::Regex::new(r#"\b(true|false|null|\d+(\.\d+)?)\b"#).ok())
        .as_ref();

    static RE_STR_LITERAL: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    let re_str_literal = RE_STR_LITERAL
        .get_or_init(|| regex::Regex::new(r#""([^\x1b"]+)""#).ok())
        .as_ref();

    if !msg.contains('{') && !msg.contains('[') {
        return msg.to_string();
    }

    let start_idx = msg.find('{').or_else(|| msg.find('[')).unwrap_or(0);
    let prefix = &msg[..start_idx];
    let json_part = &msg[start_idx..];

    let mut highlighted_json = json_part.to_string();
    if let Some(re_key) = re_key {
        highlighted_json = re_key
            .replace_all(&highlighted_json, |caps: &regex::Captures| {
                format!("\"{}{}{}\": ", CYAN, &caps[1], RESET)
            })
            .to_string();
    }

    if let Some(re_val_num) = re_val_num {
        highlighted_json = re_val_num
            .replace_all(&highlighted_json, |caps: &regex::Captures| {
                format!("{}{}{}", ORANGE, &caps[1], RESET)
            })
            .to_string();
    }

    if let Some(re_str_literal) = re_str_literal {
        highlighted_json = re_str_literal
            .replace_all(&highlighted_json, |caps: &regex::Captures| {
                format!("\"{}{}{}\"", GREEN, &caps[1], RESET)
            })
            .to_string();
    }

    format!("{}{}", prefix, highlighted_json)
}

fn is_subagent_tool(tool_name: &str) -> bool {
    if tool_name == "delegate_task"
        || tool_name == "parallel_research"
        || tool_name == "evaluator_optimizer_loop"
    {
        return true;
    }
    if let Ok(profiles) = crate::subagents::load_profiles() {
        profiles.iter().any(|p| p.name == tool_name)
    } else {
        false
    }
}

fn pretty_format_log(message: &str) -> Option<(String, String, String, &'static str)> {
    // 1. USER prompt
    if message.contains("User prompt:") || message.contains("User input:") {
        let prompt = if let Some(idx) = message.find("User prompt:") {
            &message[idx + "User prompt:".len()..]
        } else if let Some(idx) = message.find("User input:") {
            &message[idx + "User input:".len()..]
        } else {
            message
        };
        return Some((
            "👤".to_string(),
            "USER".to_string(),
            prompt.trim().to_string(),
            CYAN,
        ));
    }

    // 2. LLM CALL
    if message.contains("Sending completion request to LLM")
        || message.contains("Sending request to LLM")
    {
        let model = if let Some(idx) = message.find("model:") {
            let rest = &message[idx + "model:".len()..];
            rest.trim_matches(|c| c == ')' || c == '"' || c == '(')
                .trim()
        } else {
            ""
        };
        let msg = if model.is_empty() {
            "Sending request to LLM".to_string()
        } else {
            format!("Sending request to LLM (model: {})", model)
        };
        return Some(("📡".to_string(), "LLM CALL".to_string(), msg, SLATE));
    }

    // 3. THINKING (LLM reasoning)
    if message.contains("LLM reasoning content:") {
        let thought = if let Some(idx) = message.find("LLM reasoning content:") {
            &message[idx + "LLM reasoning content:".len()..]
        } else {
            message
        };
        return Some((
            "🧠".to_string(),
            "THINKING".to_string(),
            thought.trim().to_string(),
            ORANGE,
        ));
    }

    // 4. RESPONSE (LLM content)
    if message.contains("LLM text content:") {
        let resp = if let Some(idx) = message.find("LLM text content:") {
            &message[idx + "LLM text content:".len()..]
        } else {
            message
        };
        return Some((
            "🤖".to_string(),
            "RESPONSE".to_string(),
            resp.trim().to_string(),
            WHITE,
        ));
    }

    // 4b. LLM RESPONSE RECEIVED (finish reason)
    if message.contains("Received LLM response") {
        let finish = if let Some(idx) = message.find("finish_reason:") {
            let rest = &message[idx + "finish_reason:".len()..];
            rest.trim_matches(|c| c == ')' || c == '"' || c == '(')
                .trim()
        } else {
            ""
        };
        let msg = if finish.is_empty() {
            "Received LLM response".to_string()
        } else {
            format!("Received LLM response (finish_reason: {})", finish)
        };
        return Some(("🤖".to_string(), "RESPONSE".to_string(), msg, WHITE));
    }

    // 5. TOOL START / SUBAGENT START
    if message.contains("Executing tool call") {
        let tool = extract_quoted_field(message, "tool=");
        let args = extract_quoted_field(message, "arguments=");
        let is_subagent = tool.as_ref().map(|t| is_subagent_tool(t)).unwrap_or(false);

        let formatted = match (tool, args) {
            (Some(t), Some(a)) => format!("{} with args: {}", t, a),
            (Some(t), None) => t.to_string(),
            _ => "tool execution".to_string(),
        };

        if is_subagent {
            return Some((
                "🤖".to_string(),
                "SUBAGENT START".to_string(),
                formatted,
                PURPLE,
            ));
        } else {
            return Some(("🛠️".to_string(), "TOOL START".to_string(), formatted, GOLD));
        }
    }

    // 6. TOOL DONE / SUBAGENT DONE
    if message.contains("Tool call completed") {
        let tool = extract_quoted_field(message, "tool=");
        let is_subagent = tool.as_ref().map(|t| is_subagent_tool(t)).unwrap_or(false);

        let formatted = match tool {
            Some(t) => format!("{} completed", t),
            None => "tool completed".to_string(),
        };

        if is_subagent {
            return Some((
                "🤖".to_string(),
                "SUBAGENT DONE".to_string(),
                formatted,
                GREEN,
            ));
        } else {
            return Some(("✅".to_string(), "TOOL DONE".to_string(), formatted, GREEN));
        }
    }

    // 7. TOOL FAIL / SUBAGENT FAIL
    if message.contains("Tool call failed") || message.contains("Tool call timed out") {
        let tool = extract_quoted_field(message, "tool=");
        let err = extract_quoted_field(message, "error=");
        let base = if message.contains("timed out") {
            "timed out"
        } else {
            "failed"
        };
        let is_subagent = tool.as_ref().map(|t| is_subagent_tool(t)).unwrap_or(false);

        let formatted = match (tool, err) {
            (Some(t), Some(e)) => format!("{} {} - error: {}", t, base, e),
            (Some(t), None) => format!("{} {}", t, base),
            _ => format!("tool {}", base),
        };

        let icon = if message.contains("timed out") {
            "⏱️"
        } else {
            "✕"
        };
        let label = if is_subagent {
            "SUBAGENT FAIL"
        } else {
            "TOOL FAIL"
        };

        return Some((icon.to_string(), label.to_string(), formatted, ROSE));
    }

    // 8. BLOCKED
    if message.contains("Tool execution blocked")
        || message.contains("forbidden by security")
        || message.contains("denied by user")
    {
        let tool = extract_quoted_field(message, "tool=");
        let reason = if message.contains("blocked") {
            "loop/repetition detected"
        } else if message.contains("forbidden") {
            "forbidden by security policies"
        } else {
            "denied by user"
        };
        let formatted = match tool {
            Some(t) => format!("{} blocked - reason: {}", t, reason),
            _ => format!("tool blocked - reason: {}", reason),
        };
        return Some(("🛡️".to_string(), "BLOCKED".to_string(), formatted, GOLD));
    }

    // 9. CURATOR
    if message.contains("Self-improvement curator:") || message.contains("Self-improvement curator")
    {
        let rest = if let Some(idx) = message.find("Self-improvement curator:") {
            &message[idx + "Self-improvement curator:".len()..]
        } else if let Some(idx) = message.find("Self-improvement curator") {
            &message[idx + "Self-improvement curator".len()..]
        } else {
            message
        };
        return Some((
            "🧹".to_string(),
            "CURATOR".to_string(),
            rest.trim()
                .trim_start_matches(['-', ':'])
                .trim()
                .to_string(),
            PURPLE,
        ));
    }

    // 10. COMPACTING / SAVED / EXTRA LIFECYCLE
    if message.contains("Session saved successfully") || message.contains("Session saved") {
        return Some((
            "💾".to_string(),
            "SAVED".to_string(),
            "Session saved. Turn complete.".to_string(),
            GREEN,
        ));
    }

    if message.contains("Compacting history") {
        return Some((
            "🗜️".to_string(),
            "COMPACT".to_string(),
            message.to_string(),
            SLATE,
        ));
    }

    if message.contains("Compacted summary length")
        || message.contains("Consolidated long-term memory")
    {
        return Some((
            "💾".to_string(),
            "COMPACTED".to_string(),
            message.to_string(),
            SLATE,
        ));
    }

    None
}

fn extract_quoted_field(text: &str, field_prefix: &str) -> Option<String> {
    if let Some(pos) = text.find(field_prefix) {
        let start = pos + field_prefix.len();
        if text[start..].starts_with('"') {
            let rest = &text[start + 1..];
            let mut val = String::new();
            let mut chars = rest.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    if let Some(&next_c) = chars.peek() {
                        if next_c == '"' || next_c == '\\' {
                            if let Some(escaped) = chars.next() {
                                val.push(escaped);
                            }
                        } else {
                            val.push(c);
                        }
                    } else {
                        val.push(c);
                    }
                } else if c == '"' {
                    break;
                } else {
                    val.push(c);
                }
            }
            return Some(val);
        } else {
            // Unquoted field (till space or end of string)
            let rest = &text[start..];
            let end = rest.find(' ').unwrap_or(rest.len());
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn print_line_filtered(raw: &str, filter: &SessionFilter, level_filter: &LogLevelFilter) {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let Some(p) = parse_line(raw) else {
        // Continuation / unstructured line — print dimmed, always show
        let _ = writeln!(out, "  {DIM}{SLATE}{raw}{RESET}");
        return;
    };

    if !session_matches(&p.sessions, filter) {
        return;
    }

    if !level_filter.matches(p.level) {
        return;
    }

    // Shorten timestamp: keep "HH:MM:SS" only (chars 11..19)
    let ts = if p.timestamp.len() >= 19 {
        &p.timestamp[11..19]
    } else {
        p.timestamp
    };

    // Level badge — fixed 5-char, coloured
    let (level_col, level_label) = match p.level {
        "ERROR" => (ROSE, "ERROR"),
        "WARN" => (GOLD, "WARN "),
        "INFO" => (GREEN, "INFO "),
        "DEBUG" => (ORANGE, "DEBUG"),
        "TRACE" => (SLATE, "TRACE"),
        other => (SLATE, other),
    };

    // Message colour varies by level
    let msg_col = match p.level {
        "ERROR" => ROSE,
        "WARN" => GOLD,
        _ => WHITE,
    };

    // Truncate target to keep it readable
    let target = if p.target.len() > 35 {
        format!("…{}", &p.target[p.target.len().saturating_sub(34)..])
    } else {
        p.target.to_string()
    };

    // Target grouping colors:
    let target_col = if p.target.starts_with("openz::agent::") {
        PURPLE
    } else if p.target.starts_with("openz::providers::") {
        CYAN
    } else if p.target.starts_with("openz::tools::") {
        GOLD
    } else if p.target.starts_with("openz::channels::") {
        GREEN
    } else {
        BLUE
    };

    // Session badge (optional) — shown in cyan after the message
    let session_badge = match p.sessions.last() {
        Some(s) => {
            // Shorten long session keys: `cli:direct` → `cli`, `gateway:ws:abc` → `gateway`
            let short = s.split(':').next().unwrap_or(s.as_str());
            format!("  {CYAN}{DIM}[{short}]{RESET}")
        }
        None => String::new(),
    };

    if let Some((icon, label, formatted_msg, color)) = pretty_format_log(p.message) {
        let highlighted = highlight_message(&formatted_msg);
        let _ = writeln!(
            out,
            "{SLATE}{DIM}{ts}{RESET}  {BOLD}{level_col}{level_label}{RESET}  {target_col}{target:<35}{RESET}  {icon} {BOLD}{color}[{label}]{RESET} {color}{}{RESET}{session_badge}",
            highlighted,
            ts = ts,
            level_col = level_col,
            level_label = level_label,
            target = &target,
            target_col = target_col,
            icon = icon,
            label = label,
            color = color,
            session_badge = session_badge,
        );
    } else {
        let highlighted_msg = highlight_message(p.message);
        let _ = writeln!(
            out,
            "{SLATE}{DIM}{ts}{RESET}  {BOLD}{level_col}{level_label}{RESET}  {target_col}{target:<35}{RESET}  {msg_col}{}{RESET}{session_badge}",
            highlighted_msg,
            ts = ts,
            level_col = level_col,
            level_label = level_label,
            target = &target,
            target_col = target_col,
            msg_col = msg_col,
            session_badge = session_badge,
        );
    }
}

// ── Header banner ───────────────────────────────────────────────────────────

fn print_header(
    path: &std::path::Path,
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

fn print_tail(
    path: &PathBuf,
    tail: usize,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
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

    if tail == 0 {
        return Ok(file_len);
    }

    // Read file backwards from end using a sliding window to collect the last N lines.
    // Memory usage: O(tail) instead of O(file_size).
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

        // Scan block backwards for newlines
        let block_str = String::from_utf8_lossy(&buf);
        for (_i, c) in block_str.char_indices().rev() {
            if c == '\n' && !leftover.is_empty() {
                let line: String = leftover.iter().rev().collect();
                leftover.clear();
                if !line.trim().is_empty() {
                    lines.push(line);
                    if lines.len() >= tail {
                        break;
                    }
                }
            } else if c == '\n' {
                // empty line — skip
            } else {
                leftover.push(c);
            }
        }
    }

    // Push any remaining partial line
    if !leftover.is_empty() {
        let line: String = leftover.iter().rev().collect();
        if !line.trim().is_empty() {
            lines.push(line);
        }
    }

    lines.reverse();

    for line in &lines {
        print_line_filtered(line, filter, level_filter);
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
) {
    if let Ok(mut f) = File::open(path) {
        if let Ok(metadata) = f.metadata() {
            let len = metadata.len();
            let file_id = get_file_id(&metadata);

            let is_new_file = match current_file_id {
                Some(id) => *id != file_id,
                None => true,
            };

            if len < *pos || is_new_file {
                if is_new_file && current_file_id.is_some() {
                    println!("\n  {GOLD}── log file recreated, reading from start ──{RESET}\n");
                } else if len < *pos {
                    println!("\n  {GOLD}── log rotated/truncated, reading from start ──{RESET}\n");
                }
                *pos = 0;
                buffer.clear();
                *current_file_id = Some(file_id);
            }

            if f.seek(SeekFrom::Start(*pos)).is_ok() {
                let mut temp_buf = Vec::new();
                if f.read_to_end(&mut temp_buf).is_ok() && !temp_buf.is_empty() {
                    *pos += temp_buf.len() as u64;
                    buffer.extend_from_slice(&temp_buf);

                    if let Some(last_newline_idx) = buffer.iter().rposition(|&b| b == b'\n') {
                        let complete_bytes = &buffer[..=last_newline_idx];
                        let text = String::from_utf8_lossy(complete_bytes);
                        for line in text.lines() {
                            print_line_filtered(line, filter, level_filter);
                        }
                        *buffer = buffer[last_newline_idx + 1..].to_vec();
                        let _ = std::io::stdout().flush();
                    }
                }
            }
        }
    }
}

/// Check if auto-followed session changed.
fn update_auto_session(filter: &mut SessionFilter) {
    if let SessionFilter::Auto(ref current) = filter {
        let active = detect_active_session();
        if active != *current {
            if let Some(ref new_id) = active {
                println!("\n  {CYAN}{DIM}◉ active session changed: {new_id}{RESET}\n");
            } else {
                println!("\n  {CYAN}{DIM}◉ active session lost (idle){RESET}\n");
            }
            *filter = SessionFilter::Auto(active);
        }
    }
}

// ── Follow loop (notify-based, falls back to polling) ────────────────────

async fn follow(
    path: &PathBuf,
    mut pos: u64,
    mut filter: SessionFilter,
    level_filter: LogLevelFilter,
) -> Result<()> {
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    let mut current_file_id = None;
    if let Ok(metadata) = std::fs::metadata(path) {
        current_file_id = Some(get_file_id(&metadata));
    }

    let mut buffer = Vec::new();
    let mut last_session_check = std::time::Instant::now();

    // Set up notify watcher for instant file change notifications.
    // Falls back to polling if watcher creation fails.
    let (notify_tx, mut notify_rx) =
        tokio::sync::mpsc::channel::<notify::Result<notify::Event>>(256);
    let notify_path = path.clone();

    let has_notify = std::sync::atomic::AtomicBool::new(false);

    if let Ok(mut watcher) = RecommendedWatcher::new(
        move |res| {
            let _ = notify_tx.blocking_send(res);
        },
        Config::default(),
    ) {
        if watcher
            .watch(&notify_path, RecursiveMode::NonRecursive)
            .is_ok()
        {
            has_notify.store(true, std::sync::atomic::Ordering::SeqCst);
            // Keep watcher alive for this function's duration
            let _ = watcher;
        }
    }

    let notify_available = has_notify.load(std::sync::atomic::Ordering::SeqCst);

    // Poll interval — fast when no notify, slow heartbeat when notify works
    let mut interval = tokio::time::interval(if notify_available {
        Duration::from_millis(1000)
    } else {
        Duration::from_millis(100)
    });
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;
            _ = &mut ctrl_c => {
                println!("\n\n  {SLATE}{DIM}── openz logs stopped{RESET}\n");
                break Ok(());
            }
            _ = notify_rx.recv(), if notify_available => {
                read_new_bytes(path, &mut pos, &mut buffer, &filter, &level_filter, &mut current_file_id);
                if last_session_check.elapsed() >= Duration::from_secs(1) {
                    last_session_check = std::time::Instant::now();
                    update_auto_session(&mut filter);
                }
            }
            _ = interval.tick() => {
                if !notify_available {
                    read_new_bytes(path, &mut pos, &mut buffer, &filter, &level_filter, &mut current_file_id);
                }
                if last_session_check.elapsed() >= Duration::from_secs(1) {
                    last_session_check = std::time::Instant::now();
                    update_auto_session(&mut filter);
                }
            }
        }
    }
}

// ── Auto-detect the most recently active session ─────────────────────────────

/// Read activity.json and return the session_id of the most recently active
/// agent session (excluding idle ones that have been inactive > 60 s).
pub fn detect_active_session() -> Option<String> {
    let activity = crate::agent::activity::get_activity()?;
    // If the agent is not idle and the activity timestamp is recent (< 60s), use it.
    if activity.status != "Idle" {
        return Some(activity.session_id);
    }
    // Even idle: if updated in last 60 s, still return it as the "active" one.
    if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&activity.timestamp) {
        let age = chrono::Utc::now().signed_duration_since(ts.with_timezone(&chrono::Utc));
        if age.num_seconds() < 60 {
            return Some(activity.session_id);
        }
    }
    None
}

pub fn print_row(
    timestamp: &str,
    level: &str,
    target: &str,
    message: &str,
    session: Option<&str>,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
) {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    // Check session filter
    let sessions_vec = session.map(|s| vec![s.to_string()]).unwrap_or_default();
    if !session_matches(&sessions_vec, filter) {
        return;
    }

    // Check level filter
    if !level_filter.matches(level) {
        return;
    }

    // Shorten timestamp: HH:MM:SS (chars 11..19 of RFC3339)
    let ts = if timestamp.len() >= 19 {
        &timestamp[11..19]
    } else {
        timestamp
    };

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

    let target_str = if target.len() > 35 {
        format!("…{}", &target[target.len().saturating_sub(34)..])
    } else {
        target.to_string()
    };

    let target_col = if target.starts_with("openz::agent::") {
        PURPLE
    } else if target.starts_with("openz::providers::") {
        CYAN
    } else if target.starts_with("openz::tools::") {
        GOLD
    } else if target.starts_with("openz::channels::") {
        GREEN
    } else {
        BLUE
    };

    let session_badge = match session {
        Some(s) => {
            let short = s.split(':').next().unwrap_or(s);
            format!("  {CYAN}{DIM}[{short}]{RESET}")
        }
        None => String::new(),
    };

    if let Some((icon, label, formatted_msg, color)) = pretty_format_log(message) {
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
    } else {
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

fn print_tail_sqlite(
    db_path: &std::path::Path,
    tail: usize,
    filter: &SessionFilter,
    level_filter: &LogLevelFilter,
) -> Result<i64> {
    let conn = rusqlite::Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, level, target, message, session FROM logs ORDER BY id DESC LIMIT ?1"
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

    let mut rows: Vec<DbRow> = Vec::new();
    for row in rows_iter {
        if let Ok(r) = row {
            rows.push(r);
        }
    }

    rows.reverse();

    let mut last_id = 0;
    for r in &rows {
        print_row(&r.timestamp, &r.level, &r.target, &r.message, r.session.as_deref(), filter, level_filter);
        last_id = r.id;
    }

    Ok(last_id)
}

async fn follow_sqlite(
    db_path: &std::path::Path,
    mut last_id: i64,
    mut filter: SessionFilter,
    level_filter: LogLevelFilter,
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
                if let Ok(conn) = rusqlite::Connection::open(db_path) {
                    if let Ok(mut stmt) = conn.prepare(
                        "SELECT id, timestamp, level, target, message, session FROM logs WHERE id > ?1 ORDER BY id ASC"
                    ) {
                        if let Ok(rows_iter) = stmt.query_map([last_id], |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, Option<String>>(5)?,
                            ))
                        }) {
                            for row in rows_iter {
                                if let Ok((id, timestamp, level, target, message, session)) = row {
                                    print_row(&timestamp, &level, &target, &message, session.as_deref(), &filter, &level_filter);
                                    last_id = id;
                                }
                            }
                        }
                    }
                }

                if last_session_check.elapsed() >= Duration::from_secs(1) {
                    last_session_check = std::time::Instant::now();
                    update_auto_session(&mut filter);
                }
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
) -> Result<()> {
    let path = log_path.unwrap_or_else(default_db_path);

    let is_sqlite = path.extension().map_or(false, |ext| ext == "db");

    let effective_filter = match &filter {
        SessionFilter::Only(_) => filter.clone(),
        SessionFilter::Auto(_) => filter.clone(),
        SessionFilter::All => {
            if let Some(active) = detect_active_session() {
                println!("\n  {CYAN}{DIM}◉ active session detected: {active}{RESET}");
            }
            filter.clone()
        }
    };

    if is_sqlite {
        print_header(&path, tail, &effective_filter, &level_filter);
        let last_id = print_tail_sqlite(&path, tail, &effective_filter, &level_filter)?;
        println!("\n  {PURPLE}{DIM}── live ──{RESET}\n");
        follow_sqlite(&path, last_id, effective_filter, level_filter).await
    } else {
        print_header(&path, tail, &effective_filter, &level_filter);
        let pos = print_tail(&path, tail, &effective_filter, &level_filter)?;
        println!("\n  {PURPLE}{DIM}── live ──{RESET}\n");
        follow(&path, pos, effective_filter, level_filter).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_logging_workflow() {
        let db_path = std::env::temp_dir().join(format!("logs_test_{}.db", uuid::Uuid::new_v4()));

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                message TEXT NOT NULL,
                session TEXT
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO logs (timestamp, level, target, message, session) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "2026-07-20T12:00:00Z",
                "INFO",
                "openz::test",
                "Test message 1",
                Some("session-123"),
            ],
        ).unwrap();

        conn.execute(
            "INSERT INTO logs (timestamp, level, target, message, session) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "2026-07-20T12:01:00Z",
                "ERROR",
                "openz::test",
                "Test message 2",
                Some("session-123"),
            ],
        ).unwrap();

        let filter = SessionFilter::Only("session-123".to_string());
        let level_filter = LogLevelFilter::Trace;
        let last_id = print_tail_sqlite(&db_path, 10, &filter, &level_filter).unwrap();
        assert_eq!(last_id, 2);

        let error_level_filter = LogLevelFilter::Error;
        let last_id_error = print_tail_sqlite(&db_path, 10, &filter, &error_level_filter).unwrap();
        assert_eq!(last_id_error, 2);

        let _ = std::fs::remove_file(&db_path);
    }
}
