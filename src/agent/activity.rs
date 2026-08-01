use crate::config::loader::{resolve_path, runtime_data_dir};
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
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            tracing::warn!("Failed to create activity directory: {}", e);
            return;
        }
    }
    match serde_json::to_string_pretty(activity) {
        Ok(content) => {
            // Atomic write: write to a unique temp file then rename to prevent partial reads.
            let tmp_path = path.with_extension(format!("json.tmp.{}", uuid::Uuid::new_v4()));
            match fs::write(&tmp_path, &content) {
                Ok(()) => {
                    if let Err(e) = fs::rename(&tmp_path, path) {
                        tracing::warn!("Failed to rename activity file {:?}: {}", tmp_path, e);
                        let _ = fs::remove_file(&tmp_path);
                    }
                }
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
            if pending.is_some() {
                state.last_written_at = Some(Instant::now());
            }
            pending
        };
        if let Some(activity) = pending {
            write_activity_file(&path, &activity);
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

        if force_write || due {
            state.last_written_at = Some(now);
            state.pending = None;
            write_now = Some(activity);
        } else {
            state.pending = Some(activity);
            if !state.flush_scheduled {
                state.flush_scheduled = true;
                schedule_after = Some(ACTIVITY_WRITE_THROTTLE - elapsed.unwrap_or_default());
            }
        }
    }

    if let Some(activity) = write_now {
        write_activity_file(&path, &activity);
    }
    if let Some(delay) = schedule_after {
        schedule_activity_flush(path, delay);
    }
}

pub fn update_activity(session_id: &str, status: &str, current_tool: Option<&str>) {
    let path = resolve_path("~/.openz/activity.json");
    let activity = AgentActivity {
        session_id: session_id.to_string(),
        status: status.to_string(),
        current_tool: current_tool.map(|s| s.to_string()),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    update_activity_at_path(path, activity);
}

pub fn get_activity() -> Option<AgentActivity> {
    let path = resolve_path("~/.openz/activity.json");
    if !path.exists() {
        return None;
    }
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
    resolve_path(&format!("~/.openz/inbox_{}.json", inbox_slug(session_id)))
}

fn inbox_message_is_expired(message: &InboxMessage) -> bool {
    let Some(timestamp) = chrono::DateTime::parse_from_rfc3339(&message.timestamp).ok() else {
        return true;
    };
    chrono::Utc::now()
        .signed_duration_since(timestamp.with_timezone(&chrono::Utc))
        .num_seconds()
        > REMOTE_INBOX_TTL_SECS
}

fn quarantine_inbox_file(path: &Path) {
    let quarantine = path.with_file_name(format!(
        "{}.invalid.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("inbox"),
        uuid::Uuid::new_v4()
    ));
    if fs::rename(path, quarantine).is_err() {
        let _ = fs::remove_file(path);
    }
}

pub fn send_inbox_message(session_id: &str, message: &str, sender: &str) -> anyhow::Result<()> {
    let dir = runtime_data_dir();
    fs::create_dir_all(&dir)?;

    let msg = InboxMessage {
        message: message.to_string(),
        sender: sender.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let content = serde_json::to_string_pretty(&msg)?;
    let slug = inbox_slug(session_id);
    let path = dir.join(format!("inbox_{}_{}.json", slug, uuid::Uuid::new_v4()));
    let temp_path = dir.join(format!(
        "{}.tmp.{}",
        path.file_name().unwrap().to_string_lossy(),
        uuid::Uuid::new_v4()
    ));

    fs::write(&temp_path, content)?;
    fs::rename(temp_path, path)?;
    Ok(())
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

    // Read the legacy single-file format during migration.
    let legacy = inbox_legacy_path(session_id);
    if legacy.exists() {
        paths.push(legacy);
    }

    let mut candidates = Vec::new();
    for path in paths {
        let Ok(content) = fs::read_to_string(&path) else {
            quarantine_inbox_file(&path);
            continue;
        };
        let Ok(message) = serde_json::from_str::<InboxMessage>(&content) else {
            quarantine_inbox_file(&path);
            continue;
        };
        if inbox_message_is_expired(&message) {
            let _ = fs::remove_file(&path);
            continue;
        }
        candidates.push((message.timestamp.clone(), path));
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
        if fs::rename(&path, &claim_path).is_err() {
            continue;
        }

        let parsed = fs::read_to_string(&claim_path)
            .ok()
            .and_then(|content| serde_json::from_str::<InboxMessage>(&content).ok());
        let Some(message) = parsed else {
            quarantine_inbox_file(&claim_path);
            continue;
        };

        if inbox_message_is_expired(&message) {
            let _ = fs::remove_file(&claim_path);
            continue;
        }

        let _ = fs::remove_file(&claim_path);
        return Some(message);
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

fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        Path::new("/proc").join(pid.to_string()).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

fn parse_rfc3339_utc(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn active_tui_is_stale(session: &ActiveTuiSession, now: chrono::DateTime<chrono::Utc>) -> bool {
    if !process_is_alive(session.pid) {
        return true;
    }
    let Some(last_seen) = parse_rfc3339_utc(&session.last_seen_at) else {
        return true;
    };
    now.signed_duration_since(last_seen).num_seconds() > ACTIVE_TUI_STALE_SECS
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
    if collapsed.chars().count() > 64 {
        let mut short = collapsed.chars().take(61).collect::<String>();
        short.push_str("...");
        short
    } else {
        collapsed
    }
}

pub fn upsert_active_tui_session(session: &ActiveTuiSession) -> anyhow::Result<()> {
    let dir = active_tui_dir();
    fs::create_dir_all(&dir)?;
    let path = active_tui_path(&session.session_key);
    let tmp_path = path.with_extension(format!("json.tmp.{}", uuid::Uuid::new_v4()));
    let content = serde_json::to_string_pretty(session)?;
    fs::write(&tmp_path, content)?;
    fs::rename(tmp_path, path)?;
    Ok(())
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
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let now = chrono::Utc::now();
    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(session) = serde_json::from_str::<ActiveTuiSession>(&content) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        if active_tui_is_stale(&session, now) {
            let _ = fs::remove_file(&path);
            continue;
        }
        sessions.push(session);
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
mod tests {
    use super::*;

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_test_env() -> std::sync::MutexGuard<'static, ()> {
        TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn reset_activity_write_state_for_test() {
        let mut state = activity_write_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.last_written_at = None;
        state.pending = None;
        state.flush_scheduled = false;
    }

    fn temp_activity_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openz_activity_{}_{}.json",
            name,
            uuid::Uuid::new_v4()
        ))
    }

    fn activity(session_id: &str, status: &str, current_tool: Option<&str>) -> AgentActivity {
        AgentActivity {
            session_id: session_id.to_string(),
            status: status.to_string(),
            current_tool: current_tool.map(|tool| tool.to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn read_activity_at(path: &Path) -> AgentActivity {
        let content = fs::read_to_string(path).expect("activity file should exist");
        serde_json::from_str(&content).expect("activity file should deserialize")
    }

    #[test]
    fn activity_updates_are_throttled_and_coalesced() {
        let _guard = lock_test_env();
        reset_activity_write_state_for_test();
        let path = temp_activity_path("coalesce");

        update_activity_at_path(path.clone(), activity("s1", "Processing user prompt", None));
        assert_eq!(read_activity_at(&path).status, "Processing user prompt");

        update_activity_at_path(
            path.clone(),
            activity("s1", "Executing tool", Some("grep_search")),
        );
        let immediate = read_activity_at(&path);
        assert_eq!(immediate.status, "Processing user prompt");
        assert_eq!(immediate.current_tool, None);

        let start = std::time::Instant::now();
        let mut flushed = read_activity_at(&path);
        while flushed.status != "Executing tool" && start.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(50));
            flushed = read_activity_at(&path);
        }
        assert_eq!(flushed.status, "Executing tool");
        assert_eq!(flushed.current_tool.as_deref(), Some("grep_search"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn idle_activity_forces_immediate_write() {
        let _guard = lock_test_env();
        reset_activity_write_state_for_test();
        let path = temp_activity_path("idle");

        update_activity_at_path(path.clone(), activity("s1", "Processing user prompt", None));
        update_activity_at_path(path.clone(), activity("s1", "Idle", None));

        assert_eq!(read_activity_at(&path).status, "Idle");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn session_preview_uses_latest_user_message_and_truncates() {
        let mut messages = Vec::new();
        messages.push(crate::session::Message {
            role: "user".to_string(),
            content: "first prompt".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        });
        messages.push(crate::session::Message {
            role: "assistant".to_string(),
            content: "answer".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        });
        messages.push(crate::session::Message {
            role: "user".to_string(),
            content: "this is the latest prompt with many words that should be used for preview because it is the newest".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        });

        let preview = session_preview_from_messages(&messages);
        assert!(preview.starts_with("this is the latest prompt"));
        assert!(preview.len() <= 67);
    }

    #[test]
    fn direct_target_requires_exactly_one_session() {
        assert!(resolve_direct_target_from_keys(&[]).is_err());
        assert_eq!(
            resolve_direct_target_from_keys(&["cli:one".to_string()]).unwrap(),
            "cli:one"
        );
        assert!(
            resolve_direct_target_from_keys(&["cli:one".to_string(), "cli:two".to_string()])
                .is_err()
        );
    }

    #[test]
    fn inbox_expiry_rejects_old_messages() {
        let old = InboxMessage {
            message: "old".to_string(),
            sender: "test".to_string(),
            timestamp: (chrono::Utc::now() - chrono::Duration::minutes(6)).to_rfc3339(),
        };
        assert!(inbox_message_is_expired(&old));
    }

    #[tokio::test]
    async fn inbox_queue_preserves_fifo_order() {
        let temp_dir =
            std::env::temp_dir().join(format!("openz_inbox_fifo_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        crate::config::loader::CONFIG_DIR_OVERRIDE
            .scope(temp_dir.clone(), async {
                send_inbox_message("cli:test", "first", "test").unwrap();
                tokio::time::sleep(Duration::from_millis(2)).await;
                send_inbox_message("cli:test", "second", "test").unwrap();

                assert_eq!(pop_inbox_message("cli:test").unwrap().message, "first");
                assert_eq!(pop_inbox_message("cli:test").unwrap().message, "second");
                assert!(pop_inbox_message("cli:test").is_none());
            })
            .await;

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn inbox_queue_quarantines_malformed_entries() {
        let temp_dir =
            std::env::temp_dir().join(format!("openz_inbox_invalid_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        crate::config::loader::CONFIG_DIR_OVERRIDE
            .scope(temp_dir.clone(), async {
                std::fs::write(
                    temp_dir.join("inbox_cli_test_invalid.json"),
                    "{not valid json",
                )
                .unwrap();
                send_inbox_message("cli:test", "valid", "test").unwrap();

                assert_eq!(pop_inbox_message("cli:test").unwrap().message, "valid");
                let quarantined = std::fs::read_dir(&temp_dir)
                    .unwrap()
                    .flatten()
                    .any(|entry| entry.file_name().to_string_lossy().contains(".invalid."));
                assert!(quarantined);
            })
            .await;

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn active_tui_stale_when_pid_is_dead_or_timestamp_invalid() {
        let now = chrono::Utc::now();
        let mut session = ActiveTuiSession {
            session_key: "cli:test".to_string(),
            pid: 0,
            cwd: "/tmp".to_string(),
            started_at: now.to_rfc3339(),
            last_seen_at: now.to_rfc3339(),
            model: "model".to_string(),
            provider: "provider".to_string(),
            preview: "preview".to_string(),
        };
        assert!(active_tui_is_stale(&session, now));

        session.pid = std::process::id();
        session.last_seen_at = "not-a-date".to_string();
        assert!(active_tui_is_stale(&session, now));
    }
}
