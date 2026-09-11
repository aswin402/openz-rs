use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const SESSION_LOCK_STALE_SECS: u64 = 60;
const SESSION_LOCK_RETRY_ATTEMPTS: usize = 5;
const SESSION_LOCK_INITIAL_BACKOFF_MS: u64 = 25;
const SESSION_LOCK_MAX_BACKOFF_MS: u64 = 250;

fn canonical_extra_without_hash(extra: &serde_json::Map<String, serde_json::Value>) -> String {
    let mut filtered = serde_json::Map::new();
    let mut keys: Vec<_> = extra.keys().filter(|key| key.as_str() != "hash").collect();
    keys.sort();
    for key in keys {
        if let Some(value) = extra.get(key) {
            filtered.insert(key.clone(), value.clone());
        }
    }
    serde_json::to_string(&serde_json::Value::Object(filtered)).unwrap_or_else(|_| "{}".to_string())
}

fn legacy_message_hash(
    role: &str,
    content: &str,
    timestamp: Option<&str>,
    prev_hash: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(role.as_bytes());
    hasher.update(content.as_bytes());
    if let Some(ts) = timestamp {
        hasher.update(ts.as_bytes());
    }
    hasher.update(prev_hash.as_bytes());
    let result = hasher.finalize();
    result
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

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
        self.extra
            .insert("hash".to_string(), serde_json::Value::String(hash));
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

    pub fn calculate_message_hash(
        role: &str,
        content: &str,
        timestamp: Option<&str>,
        extra: &serde_json::Map<String, serde_json::Value>,
        prev_hash: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(role.as_bytes());
        hasher.update(content.as_bytes());
        if let Some(ts) = timestamp {
            hasher.update(ts.as_bytes());
        }
        let canonical_extra = canonical_extra_without_hash(extra);
        hasher.update(canonical_extra.as_bytes());
        hasher.update(prev_hash.as_bytes());
        let result = hasher.finalize();
        result
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }

    pub fn populate_hashes(&mut self) {
        let mut prev_hash = String::new();
        for msg in &mut self.messages {
            let ts_ref = msg.timestamp.as_deref();
            let calculated = Self::calculate_message_hash(
                &msg.role,
                &msg.content,
                ts_ref,
                &msg.extra,
                &prev_hash,
            );
            msg.set_hash(calculated.clone());
            prev_hash = calculated;
        }
    }

    pub fn verify_hash_chain(&self) -> Result<()> {
        let mut prev_hash = String::new();
        for (i, msg) in self.messages.iter().enumerate() {
            let ts_ref = msg.timestamp.as_deref();
            let calculated = Self::calculate_message_hash(
                &msg.role,
                &msg.content,
                ts_ref,
                &msg.extra,
                &prev_hash,
            );
            match msg.get_hash() {
                Some(stored) => {
                    let legacy = legacy_message_hash(&msg.role, &msg.content, ts_ref, &prev_hash);
                    if stored == calculated {
                        prev_hash = calculated;
                    } else if stored == legacy {
                        prev_hash = stored.to_string();
                    } else {
                        anyhow::bail!(
                            "Cryptographic verification failed: message at index {} has been tampered with. Stored: {}, Calculated: {}",
                            i,
                            stored,
                            calculated
                        );
                    }
                }
                None => {
                    anyhow::bail!(
                        "Cryptographic verification failed: message at index {} is missing its verification hash.",
                        i
                    );
                }
            }
        }
        Ok(())
    }
}


fn remove_stale_lock_path(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    if metadata.is_file() {
        return Ok(());
    }

    let modified = metadata.modified().unwrap_or(SystemTime::now());
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_else(|_| Duration::from_secs(0));
    if age < Duration::from_secs(SESSION_LOCK_STALE_SECS) {
        return Ok(());
    }

    tracing::warn!(
        path = %path.display(),
        age_secs = age.as_secs(),
        "Removing stale corrupt OpenZ session lock path"
    );
    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn try_open_and_lock_session_file(path: &Path, key: &str) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("Failed to create lock file {:?}", path))?;
    file.try_lock_exclusive().with_context(|| {
        format!(
            "Session '{}' is locked by another openz process. \
             Only one agent can use a session at a time.",
            key
        )
    })?;
    Ok(file)
}

fn acquire_lock_blocking(dir: &Path, path: &Path, key: &str) -> Result<File> {
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }

    let mut delay = Duration::from_millis(SESSION_LOCK_INITIAL_BACKOFF_MS);
    let mut last_error = None;
    for attempt in 0..SESSION_LOCK_RETRY_ATTEMPTS {
        remove_stale_lock_path(path)?;
        match try_open_and_lock_session_file(path, key) {
            Ok(file) => return Ok(file),
            Err(err) => {
                last_error = Some(err);
                if attempt + 1 < SESSION_LOCK_RETRY_ATTEMPTS {
                    std::thread::sleep(delay);
                    delay = std::cmp::min(
                        delay.saturating_mul(2),
                        Duration::from_millis(SESSION_LOCK_MAX_BACKOFF_MS),
                    );
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to acquire session lock")))
}


#[derive(Clone)]
pub struct ArchivedSession {
    pub session: Session,
    pub path: PathBuf,
}

/// Lightweight summary of a stored session without reading full message history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub key: String,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub message_count: usize,
    pub first_user_message: Option<String>,
}

impl SessionSummary {
    /// Return a preview title truncated to `max_len` characters, falling back to `fallback`.
    pub fn preview_title(&self, max_len: usize, fallback: &str) -> String {
        match &self.first_user_message {
            Some(msg) => {
                let cleaned = msg.split_whitespace().collect::<Vec<_>>().join(" ");
                if cleaned.chars().count() > max_len {
                    let mut s: String = cleaned.chars().take(max_len.saturating_sub(3)).collect();
                    s.push_str("...");
                    s
                } else if cleaned.is_empty() {
                    fallback.to_string()
                } else {
                    cleaned
                }
            }
            None => fallback.to_string(),
        }
    }
}

#[derive(Deserialize)]
struct SessionSummaryParser {
    key: String,
    updated_at: DateTime<Utc>,
    #[serde(default)]
    created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    messages: Vec<SessionSummaryMessageParser>,
}

#[derive(Deserialize)]
struct SessionSummaryMessageParser {
    role: String,
    content: String,
}

#[derive(Clone)]
pub struct SessionManager {
    pub dir: PathBuf,
}

impl SessionManager {
    pub fn new(dir: PathBuf) -> Self {
        SessionManager { dir }
    }

    pub fn safe_key(key: &str) -> String {
        key.replace(":", "_").replace("/", "_").replace("\\", "_")
    }

    pub fn file_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.json", Self::safe_key(key)))
    }

    fn lock_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.lock", Self::safe_key(key)))
    }

    pub fn acquire_lock(&self, key: &str) -> Result<File> {
        let path = self.lock_path(key);
        acquire_lock_blocking(&self.dir, &path, key)
    }

    pub async fn acquire_lock_async(&self, key: &str) -> Result<File> {
        let dir = self.dir.clone();
        let path = self.lock_path(key);
        let key_owned = key.to_string();
        tokio::task::spawn_blocking(move || acquire_lock_blocking(&dir, &path, &key_owned)).await?
    }

    pub fn get_or_create(&self, key: &str) -> Session {
        self.load(key).unwrap_or_else(|_| Session::new(key))
    }

    pub async fn get_or_create_async(&self, key: &str) -> Session {
        match self.load_async(key).await {
            Ok(session) => session,
            Err(_) => Session::new(key),
        }
    }

    pub fn load(&self, key: &str) -> Result<Session> {
        let path = self.file_path(key);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read session file at {:?}", path))?;
        let mut session: Session = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse session file at {:?}", path))?;
        if let Err(e) = session.verify_hash_chain() {
            tracing::warn!(
                "Session hash chain verification failed for key '{}': {}. Reconstructing.",
                key,
                e
            );
            session.populate_hashes();
            if let Ok(pretty) = serde_json::to_string_pretty(&session) {
                let _ = std::fs::write(&path, pretty);
            }
        }
        Ok(session)
    }

    pub async fn load_async(&self, key: &str) -> Result<Session> {
        let path = self.file_path(key);
        let key_owned = key.to_string();
        tokio::task::spawn_blocking(move || {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read session file at {:?}", path))?;
            let mut session: Session = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse session file at {:?}", path))?;
            if let Err(e) = session.verify_hash_chain() {
                tracing::warn!(
                    "Session hash chain verification failed for key '{}': {}. Reconstructing.",
                    key_owned,
                    e
                );
                session.populate_hashes();
                if let Ok(pretty) = serde_json::to_string_pretty(&session) {
                    let _ = std::fs::write(&path, pretty);
                }
            }
            Ok(session)
        })
        .await?
    }

    pub fn delete(&self, key: &str) -> Result<bool> {
        let path = self.file_path(key);
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => {
                Err(err).with_context(|| format!("Failed to delete session file {:?}", path))
            }
        }
    }

    pub async fn delete_async(&self, key: &str) -> Result<bool> {
        let manager = self.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || manager.delete(&key)).await?
    }

    pub fn archive(&self, key: &str) -> Result<Option<ArchivedSession>> {
        let source_path = self.file_path(key);
        if !source_path.exists() {
            return Ok(None);
        }

        let session = self.load(key)?;
        let archive_dir = self.dir.join("archive");
        fs::create_dir_all(&archive_dir)?;
        let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
        let archive_path = archive_dir.join(format!(
            "{}_{}_{}.json",
            timestamp,
            uuid::Uuid::new_v4(),
            Self::safe_key(key)
        ));
        fs::rename(&source_path, &archive_path).with_context(|| {
            format!(
                "Failed to archive session file {:?} to {:?}",
                source_path, archive_path
            )
        })?;

        Ok(Some(ArchivedSession {
            session,
            path: archive_path,
        }))
    }

    pub async fn archive_async(&self, key: &str) -> Result<Option<ArchivedSession>> {
        let manager = self.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || manager.archive(&key)).await?
    }

    pub async fn save(&self, session: &Session) -> Result<()> {
        if tokio::fs::metadata(&self.dir).await.is_err() {
            tokio::fs::create_dir_all(&self.dir)
                .await
                .with_context(|| format!("Failed to create directory {:?}", self.dir))?;
        }
        let mut session_clone = session.clone();
        session_clone.populate_hashes();
        let path = self.file_path(&session_clone.key);
        let content = serde_json::to_string_pretty(&session_clone)?;

        // Atomic write: write to temp file then rename to prevent corruption
        let temp_path = path.with_extension("json.tmp");
        tokio::fs::write(&temp_path, &content)
            .await
            .with_context(|| format!("Failed to write temp session file to {:?}", temp_path))?;
        tokio::fs::rename(&temp_path, &path)
            .await
            .with_context(|| {
                format!(
                    "Failed to rename temp session file {:?} to {:?}",
                    temp_path, path
                )
            })?;
        Ok(())
    }

    /// List lightweight summaries for all saved sessions, sorted newest first.
    pub fn list_summaries(&self) -> Vec<SessionSummary> {
        let mut summaries = Vec::new();
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return summaries;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<SessionSummaryParser>(&content) {
                        let first_user = session
                            .messages
                            .iter()
                            .find(|m| m.role == "user")
                            .map(|m| m.content.clone());
                        summaries.push(SessionSummary {
                            key: session.key,
                            updated_at: session.updated_at,
                            created_at: session.created_at.unwrap_or(session.updated_at),
                            message_count: session.messages.len(),
                            first_user_message: first_user,
                        });
                    }
                }
            }
        }
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        summaries
    }

    /// Asynchronously list lightweight summaries for all saved sessions.
    pub async fn list_summaries_async(&self) -> Vec<SessionSummary> {
        let manager = self.clone();
        tokio::task::spawn_blocking(move || manager.list_summaries())
            .await
            .unwrap_or_default()
    }

    /// List lightweight summaries for saved sessions with offset and limit, sorted newest first.
    /// Returns `(items, total_count)`.
    pub fn list_summaries_paginated(
        &self,
        offset: usize,
        limit: usize,
    ) -> (Vec<SessionSummary>, usize) {
        let all = self.list_summaries();
        let total = all.len();
        let page = all.into_iter().skip(offset).take(limit).collect();
        (page, total)
    }

    /// Asynchronously list lightweight summaries with pagination.
    /// Returns `(items, total_count)`.
    pub async fn list_summaries_paginated_async(
        &self,
        offset: usize,
        limit: usize,
    ) -> (Vec<SessionSummary>, usize) {
        let manager = self.clone();
        tokio::task::spawn_blocking(move || manager.list_summaries_paginated(offset, limit))
            .await
            .unwrap_or_else(|_| (Vec::new(), 0))
    }
}


#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
