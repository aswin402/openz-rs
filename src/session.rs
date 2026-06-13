use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Message {
    pub fn get_hash(&self) -> Option<&str> {
        self.extra.get("hash").and_then(|v| v.as_str())
    }

    pub fn set_hash(&mut self, hash: String) {
        self.extra.insert("hash".to_string(), serde_json::Value::String(hash));
    }
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

    pub fn calculate_message_hash(role: &str, content: &str, timestamp: Option<&str>, prev_hash: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(role.as_bytes());
        hasher.update(content.as_bytes());
        if let Some(ts) = timestamp {
            hasher.update(ts.as_bytes());
        }
        hasher.update(prev_hash.as_bytes());
        let result = hasher.finalize();
        result.iter().map(|b| format!("{:02x}", b)).collect::<String>()
    }

    pub fn populate_hashes(&mut self) {
        let mut prev_hash = String::new();
        for msg in &mut self.messages {
            let ts_ref = msg.timestamp.as_deref();
            let calculated = Self::calculate_message_hash(&msg.role, &msg.content, ts_ref, &prev_hash);
            msg.set_hash(calculated.clone());
            prev_hash = calculated;
        }
    }

    pub fn verify_hash_chain(&self) -> Result<()> {
        let mut prev_hash = String::new();
        for (i, msg) in self.messages.iter().enumerate() {
            let ts_ref = msg.timestamp.as_deref();
            let calculated = Self::calculate_message_hash(&msg.role, &msg.content, ts_ref, &prev_hash);
            if let Some(stored) = msg.get_hash() {
                if stored != calculated {
                    anyhow::bail!(
                        "Cryptographic verification failed: message at index {} has been tampered with. Stored: {}, Calculated: {}",
                        i, stored, calculated
                    );
                }
            }
            prev_hash = calculated;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SessionManager {
    pub dir: PathBuf,
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
        let session: Session = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse session file at {:?}", path))?;
        session.verify_hash_chain()?;
        Ok(session)
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        if !self.dir.exists() {
            fs::create_dir_all(&self.dir)?;
        }
        let mut session_clone = session.clone();
        session_clone.populate_hashes();
        let path = self.file_path(&session_clone.key);
        let content = serde_json::to_string_pretty(&session_clone)?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write session file to {:?}", path))?;
        Ok(())
    }
}
