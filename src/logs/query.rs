use anyhow::Result;

use super::storage::default_db_path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogCategory {
    Session,
    Tool,
    Cron,
    Subagent,
    Curator,
    Channel,
    Security,
    System,
}

impl LogCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Tool => "tool",
            Self::Cron => "cron",
            Self::Subagent => "subagent",
            Self::Curator => "curator",
            Self::Channel => "channel",
            Self::Security => "security",
            Self::System => "system",
        }
    }

    pub fn from_alias(raw: &str) -> Option<Self> {
        let normalized: String = raw
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        match normalized.as_str() {
            "session" | "sessions" | "sessionlog" | "sessionlogs" | "turn" | "turns" | "working"
            | "workinglog" | "workinglogs" => Some(Self::Session),
            "tool" | "tools" | "toollog" | "toollogs" => Some(Self::Tool),
            "cron" | "crons" | "cronlog" | "cronlogs" | "corn" | "corns" | "cornlog"
            | "cornlogs" | "schedule" | "scheduler" => Some(Self::Cron),
            "subagent" | "subagents" | "subagentlog" | "subagentlogs" | "orchestrator" => {
                Some(Self::Subagent)
            }
            "curator" | "curators" | "curatorlog" | "curatorlogs" | "skill" | "skills"
            | "memory" | "selfimprovement" => Some(Self::Curator),
            "channel" | "channels" | "channellog" | "channellogs" | "gateway" | "webui"
            | "websocket" | "telegram" | "discord" | "whatsapp" | "email" => Some(Self::Channel),
            "security" | "securitylog" | "securitylogs" | "guard" | "audit" | "sandbox" => {
                Some(Self::Security)
            }
            "system" | "systemlog" | "systemlogs" | "lifecycle" | "core" => Some(Self::System),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogCategoryFilter {
    All,
    Only(LogCategory),
}

impl LogCategoryFilter {
    pub fn from_opt(s: Option<&str>) -> Self {
        match s.map(str::trim).filter(|v| !v.is_empty()) {
            None => Self::All,
            Some(v) => match v.eq_ignore_ascii_case("all") {
                true => Self::All,
                false => LogCategory::from_alias(v)
                    .map(Self::Only)
                    .unwrap_or(Self::All),
            },
        }
    }

    pub fn matches(&self, category_str: &str) -> bool {
        match self {
            Self::All => true,
            Self::Only(wanted) => LogCategory::from_alias(category_str) == Some(*wanted),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::All => "all categories".to_string(),
            Self::Only(cat) => format!("category: {}", cat.as_str()),
        }
    }
}

pub fn classify_log_category(
    target: &str,
    session: Option<&str>,
    message: &str,
) -> LogCategory {
    let session_str = session.unwrap_or("");
    let is_cron_session = session_str.starts_with("cron:") || target.starts_with("openz::cron");
    let is_subagent_session = session_str.starts_with("subagent:")
        || target.starts_with("openz::subagents")
        || target.starts_with("openz::orchestrator");
    let is_security = target.starts_with("openz::agent::security")
        || target.starts_with("openz::core::secrets")
        || message.contains("forbidden by security")
        || message.contains("Tool denied by");
    let is_curator = target.starts_with("openz::agent::agent_loop::save")
        && message.contains("curator")
        || target.starts_with("openz::agent::skills")
        || target.starts_with("openz::agent::core_memory")
        || target.starts_with("openz::memory")
        || target.starts_with("openz::tools::shared_memory");
    let is_tool = target.starts_with("openz::agent::agent_loop::tool_execution")
        || target.starts_with("openz::tools::")
        || message.starts_with("Executing tool call")
        || message.starts_with("Tool call ");
    let is_channel = target.starts_with("openz::channels::");
    let is_session_turn = target.starts_with("openz::agent::agent_loop::")
        || target.starts_with("openz::providers::")
        || (!session_str.is_empty() && !is_cron_session && !is_subagent_session);

    let rules: [(bool, LogCategory); 7] = [
        (is_security, LogCategory::Security),
        (is_cron_session, LogCategory::Cron),
        (is_subagent_session, LogCategory::Subagent),
        (is_curator, LogCategory::Curator),
        (is_tool, LogCategory::Tool),
        (is_channel, LogCategory::Channel),
        (is_session_turn, LogCategory::Session),
    ];

    rules
        .into_iter()
        .find_map(|(matched, cat)| matched.then_some(cat))
        .unwrap_or(LogCategory::System)
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
            None | Some("") => SessionFilter::All,
            Some(k) => match k.to_ascii_lowercase().as_str() {
                "all" => SessionFilter::All,
                "auto" => SessionFilter::Auto(detect_active_session()),
                "gateway" | "webui" | "ws" => SessionFilter::Only("ws:".to_string()),
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
    line_sessions.iter().any(|s| s.starts_with(wanted))
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
/// interactive agent session (excluding idle ones inactive > 60 s).
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
    (&["cron"], "Cron Job"),
    (&["subagent"], "Subagent"),
    (&["websocket", "gateway"], "Gateway"),
    (&["telegram"], "Telegram Bot"),
    (&["discord"], "Discord Bot"),
    (&["whatsapp"], "WhatsApp Bot"),
    (&["email"], "Email Handler"),
    (&["cli"], "CLI Agent"),
];

fn classify_target_session_type(target: &str, session_id: &str) -> Option<&'static str> {
    let prefix_kind = [
        ("cron:", "Cron Job"),
        ("subagent:", "Subagent"),
        ("ws:", "WebUI Session"),
        ("cli:", "CLI Session"),
        ("tg:", "Telegram Session"),
    ]
    .into_iter()
    .find_map(|(prefix, label)| session_id.starts_with(prefix).then_some(label));

    prefix_kind.or_else(|| {
        TARGET_SESSION_TYPE_RULES
            .iter()
            .find_map(|(needles, label)| needles.iter().any(|n| target.contains(n)).then_some(*label))
    })
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
         LIMIT 20",
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

                    match classify_target_session_type(&target, &meta.session_id) {
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
        .prepare(
            "SELECT session FROM logs
             WHERE session IS NOT NULL AND session NOT LIKE 'cron:%'
             ORDER BY id DESC LIMIT 1",
        )
        .ok()?;
    stmt.query_row([], |row| row.get(0))
        .ok()
        .or_else(|| {
            conn.prepare("SELECT session FROM logs WHERE session IS NOT NULL ORDER BY id DESC LIMIT 1")
                .ok()?
                .query_row([], |row| row.get(0))
                .ok()
        })
}
