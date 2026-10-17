use anyhow::Result;

use super::storage::default_db_path;

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
            None | Some("") => SessionFilter::All,
            Some(k) => match k.to_ascii_lowercase().as_str() {
                "all" => SessionFilter::All,
                "auto" => SessionFilter::Auto(detect_active_session()),
                _ => SessionFilter::Only(k.to_string()),
            },
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevelFilter {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevelFilter {
    #[allow(non_upper_case_globals)]
    pub const All: Self = Self::Trace;

    pub fn from_opt(s: Option<&str>) -> Self {
        match s.map(|x| x.to_uppercase()).as_deref() {
            Some("ERROR") => LogLevelFilter::Error,
            Some("WARN") => LogLevelFilter::Warn,
            Some("INFO") => LogLevelFilter::Info,
            Some("DEBUG") => LogLevelFilter::Debug,
            Some("TRACE") => LogLevelFilter::Trace,
            Some("ALL") => LogLevelFilter::All,
            _ => LogLevelFilter::All,
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

fn matches_session_prefix(line_sessions: &[String], wanted: &str) -> bool {
    line_sessions.is_empty() || line_sessions.iter().any(|s| s.starts_with(wanted))
}

/// Returns true when any of the line's sessions match the filter.
pub(crate) fn session_matches(line_sessions: &[String], filter: &SessionFilter) -> bool {
    match filter {
        SessionFilter::All | SessionFilter::Auto(None) => true,
        SessionFilter::Only(wanted) | SessionFilter::Auto(Some(wanted)) => {
            matches_session_prefix(line_sessions, wanted)
        }
    }
}

/// Read activity.json and return the session_id of the most recently active
/// agent session (excluding idle ones that have been inactive > 60 s).
pub fn detect_active_session() -> Option<String> {
    let activity = crate::agent::activity::get_activity()?;
    match activity.status.as_str() {
        "Idle" => chrono::DateTime::parse_from_rfc3339(&activity.timestamp)
            .ok()
            .and_then(|ts| {
                let age = chrono::Utc::now().signed_duration_since(ts.with_timezone(&chrono::Utc));
                (age.num_seconds() < 60).then_some(activity.session_id)
            }),
        _ => Some(activity.session_id),
    }
}

pub struct RunningSession {
    pub session_id: String,
    pub session_type: String,
    pub last_log_message: String,
    pub last_seen: String,
}

const TARGET_SESSION_TYPE_RULES: &[(&[&str], &str)] = &[
    (&["websocket", "gateway"], "Gateway"),
    (&["telegram"], "Telegram Bot"),
    (&["discord"], "Discord Bot"),
    (&["whatsapp"], "WhatsApp Bot"),
    (&["email"], "Email Handler"),
    (&["cli"], "CLI Agent"),
];

fn classify_target_session_type(target: &str) -> Option<&'static str> {
    TARGET_SESSION_TYPE_RULES
        .iter()
        .find_map(|(needles, label)| needles.iter().any(|n| target.contains(n)).then_some(*label))
}

pub fn get_running_sessions() -> Result<Vec<RunningSession>> {
    let db_path = default_db_path();
    let conn = rusqlite::Connection::open(&db_path)?;

    let mut stmt = conn.prepare(
        "SELECT session, MAX(timestamp) as last_seen, COUNT(*) as log_count 
         FROM logs 
         WHERE session IS NOT NULL 
         GROUP BY session 
         ORDER BY last_seen DESC 
         LIMIT 15",
    )?;

    struct SessionMeta {
        session_id: String,
        last_seen: String,
    }

    let rows_iter = stmt.query_map([], |row| {
        Ok(SessionMeta {
            session_id: row.get(0)?,
            last_seen: row.get(1)?,
        })
    })?;

    let mut sessions = Vec::new();
    for meta in rows_iter.flatten() {
        let mut type_stmt = conn.prepare(
            "SELECT target, message FROM logs WHERE session = ?1 ORDER BY id DESC LIMIT 5",
        )?;

        let mut target_type = "Agent".to_string();
        let mut last_msg = String::new();

        match type_stmt.query([&meta.session_id]) {
            Ok(mut type_rows) => {
                while let Ok(Some(r)) = type_rows.next() {
                    let target: String = r.get(0)?;
                    let message: String = r.get(1)?;

                    match last_msg.is_empty() {
                        true => last_msg = message,
                        false => {}
                    }

                    match classify_target_session_type(&target) {
                        Some(kind) => target_type = kind.to_string(),
                        None => {}
                    }
                }
            }
            Err(_) => {}
        }

        sessions.push(RunningSession {
            session_id: meta.session_id,
            session_type: target_type,
            last_log_message: last_msg,
            last_seen: meta.last_seen,
        });
    }

    Ok(sessions)
}

pub fn get_latest_session_id() -> Option<String> {
    let db_path = default_db_path();
    let conn = rusqlite::Connection::open(&db_path).ok()?;
    let mut stmt = conn
        .prepare("SELECT session FROM logs WHERE session IS NOT NULL ORDER BY id DESC LIMIT 1")
        .ok()?;
    stmt.query_row([], |row| row.get(0)).ok()
}
