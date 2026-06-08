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
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(&activity) {
        let _ = fs::write(path, content);
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
