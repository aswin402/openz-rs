use crate::config::loader::{activity_file, runtime_data_dir};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentActivity {
    pub session_id: String,
    pub status: String,
    pub current_tool: Option<String>,
    pub timestamp: String,
}

const ACTIVITY_WRITE_THROTTLE: Duration = Duration::from_millis(200);

#[derive(Debug, Default)]
struct ActivityWriteState {
    last_written_at: Option<Instant>,
    pending: Option<AgentActivity>,
    flush_scheduled: bool,
}

static ACTIVITY_WRITE_STATE: OnceLock<Mutex<ActivityWriteState>> = OnceLock::new();

fn activity_write_state() -> &'static Mutex<ActivityWriteState> {
    ACTIVITY_WRITE_STATE.get_or_init(|| Mutex::new(ActivityWriteState::default()))
}

fn activity_status_forces_write(status: &str) -> bool {
    let lower = status.to_lowercase();
    status == "Idle"
        || lower.contains("cancel")
        || lower.contains("error")
        || lower.contains("failed")
}

fn write_activity_file(path: &Path, activity: &AgentActivity) {
    match path.parent() {
        Some(parent) => match fs::create_dir_all(parent) {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!("Failed to create activity directory: {}", e);
                return;
            }
        },
        None => {}
    }
    match serde_json::to_string_pretty(activity) {
        Ok(content) => {
            let tmp_path = path.with_extension(format!("json.tmp.{}", uuid::Uuid::new_v4()));
            match fs::write(&tmp_path, &content) {
                Ok(()) => match fs::rename(&tmp_path, path) {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::warn!("Failed to rename activity file {:?}: {}", tmp_path, e);
                        let _ = fs::remove_file(&tmp_path);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to write activity file {:?}: {}", path, e);
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to serialize activity: {}", e);
        }
    }
}

fn schedule_activity_flush(path: PathBuf, delay: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let pending = {
            let mut state = activity_write_state()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.flush_scheduled = false;
            let pending = state.pending.take();
            match pending.is_some() {
                true => state.last_written_at = Some(Instant::now()),
                false => {}
            }
            pending
        };
        match pending {
            Some(activity) => write_activity_file(&path, &activity),
            None => {}
        }
    });
}

fn update_activity_at_path(path: PathBuf, activity: AgentActivity) {
    let force_write = activity_status_forces_write(&activity.status);
    let now = Instant::now();
    let mut write_now = None;
    let mut schedule_after = None;

    {
        let mut state = activity_write_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let elapsed = state
            .last_written_at
            .map(|last| now.saturating_duration_since(last));
        let due = elapsed
            .map(|elapsed| elapsed >= ACTIVITY_WRITE_THROTTLE)
            .unwrap_or(true);

        match force_write || due {
            true => {
                state.last_written_at = Some(now);
                state.pending = None;
                write_now = Some(activity);
            }
            false => {
                state.pending = Some(activity);
                match state.flush_scheduled {
                    true => {}
                    false => {
                        state.flush_scheduled = true;
                        schedule_after = Some(ACTIVITY_WRITE_THROTTLE - elapsed.unwrap_or_default());
                    }
                }
            }
        }
    }

    match write_now {
        Some(activity) => write_activity_file(&path, &activity),
        None => {}
    }
    match schedule_after {
        Some(delay) => schedule_activity_flush(path, delay),
        None => {}
    }
}

pub fn update_activity(session_id: &str, status: &str, current_tool: Option<&str>) {
    let path = activity_file();
    let activity = AgentActivity {
        session_id: session_id.to_string(),
        status: status.to_string(),
        current_tool: current_tool.map(|s| s.to_string()),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    update_activity_at_path(path, activity);
}

pub fn get_activity() -> Option<AgentActivity> {
    let path = activity_file();
    path.exists().then_some(())?;
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InboxMessage {
    pub message: String,
    pub sender: String,
    pub timestamp: String,
}

const REMOTE_INBOX_TTL_SECS: i64 = 5 * 60;

fn inbox_slug(session_id: &str) -> String {
    session_id.replace(':', "_")
}

fn inbox_prefix(session_id: &str) -> String {
    format!("inbox_{}_", inbox_slug(session_id))
}

fn inbox_legacy_path(session_id: &str) -> PathBuf {
    runtime_data_dir().join(format!("inbox_{}.json", inbox_slug(session_id)))
}

fn inbox_message_is_expired(message: &InboxMessage) -> bool {
    match chrono::DateTime::parse_from_rfc3339(&message.timestamp) {
        Ok(timestamp) => {
            chrono::Utc::now()
                .signed_duration_since(timestamp.with_timezone(&chrono::Utc))
                .num_seconds()
                > REMOTE_INBOX_TTL_SECS
        }
        Err(_) => true,
    }
}

fn quarantine_inbox_file(path: &Path) {
    let quarantine = path.with_file_name(format!(
        "{}.invalid.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("inbox"),
        uuid::Uuid::new_v4()
    ));
    match fs::rename(path, quarantine) {
        Ok(()) => {}
        Err(_) => {
            let _ = fs::remove_file(path);
        }
    }
}

fn cleanup_stale_inbox_files_in(dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_inbox_json = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with("inbox_") && name.ends_with(".json"))
            .unwrap_or(false);
        match is_inbox_json {
            false => continue,
            true => {}
        }
        let parsed = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<InboxMessage>(&content).ok());
        match parsed {
            None => quarantine_inbox_file(&path),
            Some(message) => match inbox_message_is_expired(&message) {
                true => {
                    let _ = fs::remove_file(path);
                }
                false => {}
            },
        }
    }
}

pub fn send_inbox_message(session_id: &str, message: &str, sender: &str) -> anyhow::Result<()> {
    let dir = runtime_data_dir();
    fs::create_dir_all(&dir)?;
    cleanup_stale_inbox_files_in(&dir);

    let msg = InboxMessage {
        message: message.to_string(),
        sender: sender.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let slug = inbox_slug(session_id);
    let filename = format!("inbox_{}_{}.json", slug, uuid::Uuid::new_v4());
    crate::config::loader::save_pretty_json_atomic(&dir.join(filename), &msg)
}

pub fn pop_inbox_message(session_id: &str) -> Option<InboxMessage> {
    let dir = runtime_data_dir();
    let prefix = inbox_prefix(session_id);
    let mut paths = fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with(&prefix) && name.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    let legacy = inbox_legacy_path(session_id);
    match legacy.exists() {
        true => paths.push(legacy),
        false => {}
    }

    let mut candidates = Vec::new();
    for path in paths {
        let parsed = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<InboxMessage>(&content).ok());
        match parsed {
            None => quarantine_inbox_file(&path),
            Some(message) => match inbox_message_is_expired(&message) {
                true => {
                    let _ = fs::remove_file(&path);
                }
                false => candidates.push((message.timestamp.clone(), path)),
            },
        }
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0));

    for (_, path) in candidates {
        let claim_path = path.with_file_name(format!(
            "{}.claimed.{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("inbox"),
            uuid::Uuid::new_v4()
        ));
        match fs::rename(&path, &claim_path) {
            Ok(()) => {}
            Err(_) => continue,
        }

        let parsed = fs::read_to_string(&claim_path)
            .ok()
            .and_then(|content| serde_json::from_str::<InboxMessage>(&content).ok());
        let message = match parsed {
            Some(msg) => msg,
            None => {
                quarantine_inbox_file(&claim_path);
                continue;
            }
        };

        match inbox_message_is_expired(&message) {
            true => {
                let _ = fs::remove_file(&claim_path);
                continue;
            }
            false => {
                let _ = fs::remove_file(&claim_path);
                return Some(message);
            }
        }
    }
    None
}

const ACTIVE_TUI_STALE_SECS: i64 = 30;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ActiveTuiSession {
    pub session_key: String,
    pub pid: u32,
    pub cwd: String,
    pub started_at: String,
    pub last_seen_at: String,
    pub model: String,
    pub provider: String,
    pub preview: String,
}

fn active_tui_dir() -> PathBuf {
    runtime_data_dir().join("active_tui")
}

fn active_tui_path(session_key: &str) -> PathBuf {
    let slug = session_key.replace(':', "_");
    active_tui_dir().join(format!("{slug}.json"))
}

use crate::core::process::process_is_alive;

fn parse_rfc3339_utc(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn active_tui_is_stale(session: &ActiveTuiSession, now: chrono::DateTime<chrono::Utc>) -> bool {
    match (
        process_is_alive(session.pid),
        parse_rfc3339_utc(&session.last_seen_at),
    ) {
        (true, Some(last_seen)) => {
            now.signed_duration_since(last_seen).num_seconds() > ACTIVE_TUI_STALE_SECS
        }
        _ => true,
    }
}

pub fn session_preview_from_messages(messages: &[crate::session::Message]) -> String {
    let preview = messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.content.trim())
        .filter(|content| !content.is_empty())
        .unwrap_or("No user prompt yet");
    let collapsed = preview.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.chars().count() > 64 {
        true => {
            let mut short = collapsed.chars().take(61).collect::<String>();
            short.push_str("...");
            short
        }
        false => collapsed,
    }
}

pub fn upsert_active_tui_session(session: &ActiveTuiSession) -> anyhow::Result<()> {
    crate::config::loader::save_pretty_json_atomic(&active_tui_path(&session.session_key), session)
}

pub fn remove_active_tui_session(session_key: &str) {
    let _ = fs::remove_file(active_tui_path(session_key));
}

fn resolve_direct_target_from_keys(keys: &[String]) -> anyhow::Result<String> {
    match keys {
        [] => Err(anyhow::anyhow!("No active TUI sessions are available")),
        [key] => Ok(key.clone()),
        _ => Err(anyhow::anyhow!(
            "Multiple active TUI sessions exist; select a specific session"
        )),
    }
}

pub fn resolve_cli_direct_target() -> anyhow::Result<String> {
    let keys = list_active_tui_sessions()
        .into_iter()
        .map(|session| session.session_key)
        .collect::<Vec<_>>();
    resolve_direct_target_from_keys(&keys)
}

pub fn direct_inbox_belongs_to(session_id: &str) -> bool {
    list_active_tui_sessions()
        .into_iter()
        .map(|session| session.session_key)
        .collect::<Vec<_>>()
        .as_slice()
        == [session_id.to_string()]
}

pub fn active_tui_session_exists(session_id: &str) -> bool {
    list_active_tui_sessions()
        .iter()
        .any(|session| session.session_key == session_id)
}

pub fn list_active_tui_sessions() -> Vec<ActiveTuiSession> {
    let dir = active_tui_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let now = chrono::Utc::now();
    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        match path.extension().and_then(|s| s.to_str()) == Some("json") {
            false => continue,
            true => {}
        }
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let session = match serde_json::from_str::<ActiveTuiSession>(&content) {
            Ok(session) => session,
            Err(_) => {
                let _ = fs::remove_file(&path);
                continue;
            }
        };
        match active_tui_is_stale(&session, now) {
            true => {
                let _ = fs::remove_file(&path);
            }
            false => sessions.push(session),
        }
    }
    sessions.sort_by(|a, b| b.last_seen_at.cmp(&a.last_seen_at));
    sessions
}

pub fn make_active_tui_session(
    session_key: &str,
    cwd: &Path,
    started_at: &str,
    model: &str,
    provider: &str,
    preview: &str,
) -> ActiveTuiSession {
    ActiveTuiSession {
        session_key: session_key.to_string(),
        pid: std::process::id(),
        cwd: cwd.display().to_string(),
        started_at: started_at.to_string(),
        last_seen_at: chrono::Utc::now().to_rfc3339(),
        model: model.to_string(),
        provider: provider.to_string(),
        preview: preview.to_string(),
    }
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
