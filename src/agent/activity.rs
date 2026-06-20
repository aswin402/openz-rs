use serde::{Deserialize, Serialize};
use std::fs;
use crate::config::loader::resolve_path;

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
    let content = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    serde_json::from_str(&content).ok()
}
