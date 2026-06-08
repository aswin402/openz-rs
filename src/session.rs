use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub key: String,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub last_consolidated: usize,
}

impl Session {
    pub fn new(key: &str) -> Self {
        let now = Utc::now();
        Session {
            key: key.to_string(),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
            metadata: serde_json::Map::new(),
            last_consolidated: 0,
        }
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        let msg = Message {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: Some(Utc::now().to_rfc3339()),
            extra: serde_json::Map::new(),
        };
        self.messages.push(msg);
        self.updated_at = Utc::now();
    }
}

#[derive(Clone)]
pub struct SessionManager {
    dir: PathBuf,
}

impl SessionManager {
    pub fn new(dir: PathBuf) -> Self {
        SessionManager { dir }
    }

    fn file_path(&self, key: &str) -> PathBuf {
        let safe_key = key.replace(":", "_").replace("/", "_").replace("\\", "_");
        self.dir.join(format!("{}.json", safe_key))
    }

    pub fn get_or_create(&self, key: &str) -> Session {
        self.load(key).unwrap_or_else(|_| Session::new(key))
    }

    pub fn load(&self, key: &str) -> Result<Session> {
        let path = self.file_path(key);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read session file at {:?}", path))?;
        let session = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse session file at {:?}", path))?;
        Ok(session)
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        if !self.dir.exists() {
            fs::create_dir_all(&self.dir)?;
        }
        let path = self.file_path(&session.key);
        let content = serde_json::to_string_pretty(session)?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write session file to {:?}", path))?;
        Ok(())
    }
}
