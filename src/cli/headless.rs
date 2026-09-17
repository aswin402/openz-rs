use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessRunOutput {
    pub status: String,
    pub content: String,
    pub session_id: String,
    pub tools_used: Vec<String>,
    pub tool_iterations: usize,
    pub duration_ms: u64,
    pub model: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlessFormat {
    Text,
    Json,
    StreamJson,
}

impl HeadlessFormat {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "json" => Self::Json,
            "stream-json" | "stream_json" | "ndjson" => Self::StreamJson,
            _ => Self::Text,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeadlessSecurityPolicy {
    pub auto_approve_all: bool,
    pub allowed_tools: std::collections::HashSet<String>,
}

impl HeadlessSecurityPolicy {
    pub fn new(auto_approve_all: bool, allowed_tools_csv: Option<&str>) -> Self {
        let mut allowed_tools = std::collections::HashSet::new();
        if let Some(csv) = allowed_tools_csv {
            for tool in csv.split(',') {
                let trimmed = tool.trim().to_lowercase();
                if !trimmed.is_empty() {
                    allowed_tools.insert(trimmed);
                }
            }
        }
        Self {
            auto_approve_all,
            allowed_tools,
        }
    }

    pub fn is_tool_permitted(&self, tool_name: &str, is_sensitive: bool) -> bool {
        if !is_sensitive {
            return true;
        }
        if self.auto_approve_all {
            return true;
        }
        self.allowed_tools.contains(&tool_name.to_lowercase())
    }
}

pub fn resolve_session_key(
    explicit: Option<&str>,
    _continue: bool,
    _manager: Option<&crate::session::SessionManager>,
) -> String {
    if let Some(key) = explicit.map(str::trim).filter(|k| !k.is_empty()) {
        return key.to_string();
    }
    format!(
        "cli:headless_{}_{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S"),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}
