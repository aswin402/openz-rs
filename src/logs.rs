use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

// ── AURA palette (raw ANSI — no crossterm needed) ──────────────────────────
const RESET:   &str = "\x1b[0m";
const BOLD:    &str = "\x1b[1m";
const DIM:     &str = "\x1b[2m";

const PURPLE:  &str = "\x1b[38;2;199;146;234m";  // AURA_PURPLE  — brand / header
const BLUE:    &str = "\x1b[38;2;130;170;255m";   // AURA_BLUE    — target module
const GREEN:   &str = "\x1b[38;2;195;232;141m";   // AURA_GREEN   — INFO
const GOLD:    &str = "\x1b[38;2;255;203;107m";   // AURA_GOLD    — WARN
const ROSE:    &str = "\x1b[38;2;240;113;120m";   // AURA_ROSE    — ERROR
const SLATE:   &str = "\x1b[38;2;107;122;153m";   // AURA_SLATE   — timestamp / dim
const WHITE:   &str = "\x1b[38;2;220;220;220m";   // LIGHT_WHITE  — message body
const ORANGE:  &str = "\x1b[38;2;255;133;75m";    // warm accent  — DEBUG
const CYAN:    &str = "\x1b[38;2;137;221;255m";   // AURA_CYAN    — session tag

/// Resolve the default log file path: ~/.openz/openz.log (or OPENZ_CONFIG_DIR/openz.log)
pub fn default_log_path() -> PathBuf {
    crate::config::config_dir().join("openz.log")
}

/// Which sessions to show.
#[derive(Clone, Debug)]
pub enum SessionFilter {
    /// Show all sessions (no filter).
    All,
    /// Show only lines that match this session key (prefix match).
    Only(String),
}

impl SessionFilter {
    /// Build from an optional CLI `--session` string.
    pub fn from_opt(s: Option<&str>) -> Self {
        match s {
            None | Some("") => SessionFilter::All,
            Some(k) => SessionFilter::Only(k.to_string()),
        }
    }

    /// Return a short label for the header banner.
    pub fn label(&self) -> String {
        match self {
            SessionFilter::All => "all sessions".to_string(),
            SessionFilter::Only(k) => format!("session: {}", k),
        }
    }
}

// ── Line parser ─────────────────────────────────────────────────────────────

struct ParsedLine<'a> {
    timestamp: &'a str,
    level:     &'a str,
    target:    &'a str,
    message:   &'a str,
    /// Extracted from `session=<key>` span field appended by tracing-subscriber.
    session:   Option<String>,
}

/// Parse a tracing-subscriber line:
/// `2026-06-16T17:20:43.215712Z  INFO openz::tools::mcp: message  session=cli:direct`
///
/// The `session=` field is optional — old log entries before the span was added
/// will not have it and are always shown regardless of filter.
fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let line = line.trim();
    if line.is_empty() { return None; }

    // timestamp is the first token (ISO 8601, ends with 'Z')
    let (ts, rest) = line.split_once(' ')?;
    if !ts.ends_with('Z') { return None; }

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
    let (message, session) = extract_session_field(message_raw);

    Some(ParsedLine { timestamp: ts, level, target, message, session })
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
    } else if raw.starts_with("session=") {
        // Edge: no message body, only the field
        let value = raw["session=".len()..].trim().to_string();
        ("", Some(value))
    } else {
        (raw, None)
    }
}

/// Returns true if `line_session` matches the filter.
fn session_matches(line_session: &Option<String>, filter: &SessionFilter) -> bool {
    match filter {
        SessionFilter::All => true,
        SessionFilter::Only(wanted) => {
            match line_session {
                // Lines without a session tag predate the feature — always show them
                // so old history is not silently dropped.
                None => true,
                Some(s) => s.starts_with(wanted.as_str()),
            }
        }
    }
}

// ── Pretty-print a single line ──────────────────────────────────────────────

fn print_line_filtered(raw: &str, filter: &SessionFilter) {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let Some(p) = parse_line(raw) else {
        // Continuation / unstructured line — print dimmed, always show
        let _ = writeln!(out, "  {DIM}{SLATE}{raw}{RESET}");
        return;
    };

    if !session_matches(&p.session, filter) {
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
        "ERROR" => (ROSE,   "ERROR"),
        "WARN"  => (GOLD,   "WARN "),
        "INFO"  => (GREEN,  "INFO "),
        "DEBUG" => (ORANGE, "DEBUG"),
        "TRACE" => (SLATE,  "TRACE"),
        other   => (SLATE,  other),
    };

    // Message colour varies by level
    let msg_col = match p.level {
        "ERROR" => ROSE,
        "WARN"  => GOLD,
        _       => WHITE,
    };

    // Truncate target to keep it readable
    let target = if p.target.len() > 35 {
        format!("…{}", &p.target[p.target.len().saturating_sub(34)..])
    } else {
        p.target.to_string()
    };

    // Session badge (optional) — shown in cyan after the message
    let session_badge = match &p.session {
        Some(s) => {
            // Shorten long session keys: `cli:direct` → `cli`, `gateway:ws:abc` → `gateway`
            let short = s.split(':').next().unwrap_or(s.as_str());
            format!("  {CYAN}{DIM}[{short}]{RESET}")
        }
        None => String::new(),
    };

    let _ = writeln!(
        out,
        "{SLATE}{DIM}{ts}{RESET}  {BOLD}{level_col}{level_label}{RESET}  {BLUE}{target:<35}{RESET}  {msg_col}{}{RESET}{session_badge}",
        p.message,
        ts = ts,
        level_col = level_col,
        level_label = level_label,
        target = target,
        msg_col = msg_col,
    );
}

// ── Header banner ───────────────────────────────────────────────────────────

fn print_header(path: &PathBuf, tail: usize, filter: &SessionFilter) {
    let fname = path.display();
    let filter_label = filter.label();
    println!(
        "\n{PURPLE}{BOLD}  ◇ openz{RESET}  {SLATE}live logs{RESET}  {DIM}─{RESET}  {SLATE}{fname}{RESET}  {DIM}(tail {tail}  ·  {filter_label}){RESET}"
    );
    println!(
        "{SLATE}{DIM}  {}{RESET}\n",
        "─".repeat(72)
    );
    println!(
        "  {SLATE}{DIM}{:<8}  {:<5}  {:<35}  {}{RESET}",
        "TIME", "LEVEL", "TARGET", "MESSAGE"
    );
    println!(
        "  {SLATE}{DIM}{}{RESET}\n",
        "─".repeat(72)
    );
}

// ── Tail initial lines ───────────────────────────────────────────────────────

fn print_tail(path: &PathBuf, tail: usize, filter: &SessionFilter) -> Result<u64> {
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
    let _ = f.seek(SeekFrom::Start(0));

    let reader = BufReader::new(f.take(file_len));
    let all: Vec<String> = reader.lines().map_while(|l| l.ok()).collect();
    let start = all.len().saturating_sub(tail);

    if start > 0 {
        println!(
            "  {SLATE}{DIM}↑ {} older lines not shown  (pass --tail N to see more){RESET}\n",
            start
        );
    }

    for line in &all[start..] {
        print_line_filtered(line, filter);
    }

    // Return current end-of-file position
    Ok(file_len)
}

// ── Follow loop ─────────────────────────────────────────────────────────────

#[cfg(unix)]
fn get_file_id(metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.ino()
}

#[cfg(not(unix))]
fn get_file_id(_metadata: &std::fs::Metadata) -> u64 {
    0
}

async fn follow(path: &PathBuf, mut pos: u64, filter: SessionFilter) -> Result<()> {
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut current_file_id = None;
    if let Ok(metadata) = std::fs::metadata(path) {
        current_file_id = Some(get_file_id(&metadata));
    }

    let mut buffer = Vec::new();

    loop {
        tokio::select! {
            biased;
            _ = &mut ctrl_c => {
                println!("\n\n  {SLATE}{DIM}── openz logs stopped{RESET}\n");
                break;
            }
            _ = interval.tick() => {
                if let Ok(mut f) = File::open(path) {
                    if let Ok(metadata) = f.metadata() {
                        let len = metadata.len();
                        let file_id = get_file_id(&metadata);
                        
                        let is_new_file = match current_file_id {
                            Some(id) => id != file_id,
                            None => true,
                        };
                        
                        if len < pos || is_new_file {
                            if is_new_file && current_file_id.is_some() {
                                println!("\n  {GOLD}── log file recreated, reading from start ──{RESET}\n");
                            } else if len < pos {
                                println!("\n  {GOLD}── log rotated/truncated, reading from start ──{RESET}\n");
                            }
                            pos = 0;
                            buffer.clear();
                            current_file_id = Some(file_id);
                        }
                        
                        if let Ok(_) = f.seek(SeekFrom::Start(pos)) {
                            let mut temp_buf = Vec::new();
                            if f.read_to_end(&mut temp_buf).is_ok() && !temp_buf.is_empty() {
                                pos += temp_buf.len() as u64;
                                buffer.extend_from_slice(&temp_buf);
                                
                                if let Some(last_newline_idx) = buffer.iter().rposition(|&b| b == b'\n') {
                                    let complete_bytes = &buffer[..=last_newline_idx];
                                    let text = String::from_utf8_lossy(complete_bytes);
                                    for line in text.lines() {
                                        print_line_filtered(line, &filter);
                                    }
                                    
                                    // Keep only the incomplete trailing bytes in the buffer
                                    buffer = buffer[last_newline_idx + 1..].to_vec();
                                    let _ = std::io::stdout().flush();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
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

// ── Public entrypoint ────────────────────────────────────────────────────────

pub async fn run_logs_viewer(
    log_path: Option<PathBuf>,
    tail: usize,
    filter: SessionFilter,
) -> Result<()> {
    let path = log_path.unwrap_or_else(default_log_path);

    // If filtering by a specific session, say so in the header.
    // If All, check activity.json to see if there is a hot session to highlight.
    let effective_filter = match &filter {
        SessionFilter::Only(_) => filter.clone(),
        SessionFilter::All => {
            // We still show all lines; just note if something is active.
            if let Some(active) = detect_active_session() {
                println!(
                    "\n  {CYAN}{DIM}◉ active session detected: {active}{RESET}"
                );
            }
            filter.clone()
        }
    };

    print_header(&path, tail, &effective_filter);

    let pos = print_tail(&path, tail, &effective_filter)?;

    // Print live-follow separator
    println!(
        "\n  {PURPLE}{DIM}── live ──{RESET}\n"
    );

    follow(&path, pos, effective_filter).await
}
