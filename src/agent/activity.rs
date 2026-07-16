use crate::config::loader::{resolve_path, runtime_data_dir};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentActivity {
    pub session_id: String,
    pub status: String,
    pub current_tool: Option<String>,
    pub timestamp: String,
}

pub fn update_activity(session_id: &str, status: &str, current_tool: Option<&str>) {
    let path = resolve_path("~/.openz/activity.json");
    let activity = AgentActivity {
        session_id: session_id.to_string(),
        status: status.to_string(),
        current_tool: current_tool.map(|s| s.to_string()),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            tracing::warn!("Failed to create activity directory: {}", e);
            return;
        }
    }
    match serde_json::to_string_pretty(&activity) {
        Ok(content) => {
            // Atomic write: write to temp file then rename to prevent partial reads
            let tmp_path = path.with_extension("json.tmp");
            match fs::write(&tmp_path, &content) {
                Ok(()) => {
                    if let Err(e) = fs::rename(&tmp_path, &path) {
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

pub fn send_inbox_message(session_id: &str, message: &str, sender: &str) -> anyhow::Result<()> {
    let slug = session_id.replace(':', "_");
    let path = resolve_path(&format!("~/.openz/inbox_{}.json", slug));
    let msg = InboxMessage {
        message: message.to_string(),
        sender: sender.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = serde_json::to_string_pretty(&msg)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn pop_inbox_message(session_id: &str) -> Option<InboxMessage> {
    let slug = session_id.replace(':', "_");
    let path = resolve_path(&format!("~/.openz/inbox_{}.json", slug));
    if !path.exists() {
        return None;
    }
    let temp_name = format!("inbox_{}.json.tmp.{}", slug, uuid::Uuid::new_v4());
    let temp_path = path.with_file_name(temp_name);
    // Atomic rename: if successful, this thread owns this message and can read it
    if fs::rename(&path, &temp_path).is_ok() {
        let content = fs::read_to_string(&temp_path).ok()?;
        let _ = fs::remove_file(&temp_path);
        serde_json::from_str(&content).ok()
    } else {
        None
    }
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
