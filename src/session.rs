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

#[cfg(test)]
mod hash_tests {
    use super::*;

    #[test]
    fn hash_changes_when_extra_metadata_changes() {
        let mut extra = serde_json::Map::new();
        extra.insert("tool_call_id".to_string(), serde_json::json!("call_1"));

        let h1 =
            Session::calculate_message_hash("tool", "{}", Some("2026-07-06T00:00:00Z"), &extra, "");
        extra.insert("tool_call_id".to_string(), serde_json::json!("call_2"));
        let h2 =
            Session::calculate_message_hash("tool", "{}", Some("2026-07-06T00:00:00Z"), &extra, "");

        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_ignores_hash_field_itself() {
        let mut extra = serde_json::Map::new();
        extra.insert("name".to_string(), serde_json::json!("read_file"));
        let h1 = Session::calculate_message_hash("tool", "{}", None, &extra, "");

        extra.insert("hash".to_string(), serde_json::json!("old"));
        let h2 = Session::calculate_message_hash("tool", "{}", None, &extra, "");

        assert_eq!(h1, h2);
    }

    #[test]
    fn legacy_hash_chain_still_verifies() {
        let mut session = Session::new("test");
        session.messages.push(Message {
            role: "user".to_string(),
            content: "hello".to_string(),
            timestamp: Some("2026-07-06T00:00:00Z".to_string()),
            extra: serde_json::Map::new(),
        });
        let legacy = legacy_message_hash("user", "hello", Some("2026-07-06T00:00:00Z"), "");
        session.messages[0].set_hash(legacy);

        assert!(session.verify_hash_chain().is_ok());
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

#[cfg(test)]
mod lock_tests {
    use super::*;
    use anyhow::Result;
    use std::time::{Duration, SystemTime};

    #[cfg(unix)]
    fn set_modified_for_test(path: &std::path::Path, modified: SystemTime) -> Result<()> {
        use std::os::unix::ffi::OsStrExt;
        let duration = modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let times = [
            libc::timespec {
                tv_sec: duration.as_secs() as libc::time_t,
                tv_nsec: duration.subsec_nanos() as libc::c_long,
            },
            libc::timespec {
                tv_sec: duration.as_secs() as libc::time_t,
                tv_nsec: duration.subsec_nanos() as libc::c_long,
            },
        ];
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        let rc = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn set_modified_for_test(_path: &std::path::Path, _modified: SystemTime) -> Result<()> {
        Ok(())
    }

    #[test]
    fn acquire_lock_removes_stale_corrupt_lock_path() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_session_stale_lock_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let lock_path = manager.lock_path("session");
        fs::create_dir_all(&lock_path)?;
        set_modified_for_test(
            &lock_path,
            SystemTime::now() - Duration::from_secs(SESSION_LOCK_STALE_SECS + 5),
        )?;

        let _lock = manager.acquire_lock("session")?;

        assert!(lock_path.is_file());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn acquire_lock_preserves_stale_regular_lock_file() -> Result<()> {
        let dir = std::env::temp_dir().join(format!(
            "openz_session_stale_regular_lock_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let lock_path = manager.lock_path("session");
        fs::write(&lock_path, b"old")?;
        set_modified_for_test(
            &lock_path,
            SystemTime::now() - Duration::from_secs(SESSION_LOCK_STALE_SECS + 5),
        )?;

        let _lock = manager.acquire_lock("session")?;

        assert!(lock_path.is_file());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[tokio::test]
    async fn acquire_lock_async_removes_stale_corrupt_lock_path() -> Result<()> {
        let dir = std::env::temp_dir().join(format!(
            "openz_session_async_stale_lock_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let lock_path = manager.lock_path("session");
        fs::create_dir_all(&lock_path)?;
        set_modified_for_test(
            &lock_path,
            SystemTime::now() - Duration::from_secs(SESSION_LOCK_STALE_SECS + 5),
        )?;

        let _lock = manager.acquire_lock_async("session").await?;

        assert!(lock_path.is_file());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn acquire_lock_preserves_fresh_corrupt_lock_path() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_session_fresh_lock_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let lock_path = manager.lock_path("session");
        fs::create_dir_all(&lock_path)?;

        let result = manager.acquire_lock("session");

        assert!(result.is_err());
        assert!(lock_path.is_dir());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
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
mod delete_tests {
    use super::*;

    #[test]
    fn delete_removes_sanitized_session_file() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_delete_session_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let mut session = Session::new("ws:control");
        session.add_message("user", "hello");
        session.populate_hashes();
        let path = manager.file_path(&session.key);
        fs::write(&path, serde_json::to_string_pretty(&session)?)?;

        assert!(path.exists());
        assert!(manager.delete("ws:control")?);
        assert!(!path.exists());
        assert!(!manager.delete("ws:control")?);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn archive_moves_sanitized_session_file_to_archive_dir() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_archive_session_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());
        let mut session = Session::new("telegram:42");
        session.add_message("user", "archive me");
        session.populate_hashes();
        let path = manager.file_path(&session.key);
        fs::write(&path, serde_json::to_string_pretty(&session)?)?;

        let archived = manager.archive("telegram:42")?.expect("session archived");

        assert!(!path.exists());
        assert!(archived.path.exists());
        assert!(archived.path.starts_with(dir.join("archive")));
        assert_eq!(archived.session.key, "telegram:42");
        assert_eq!(archived.session.messages.len(), 1);
        assert!(manager.archive("telegram:42")?.is_none());

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}

#[cfg(test)]
mod summary_tests {
    use super::*;

    #[test]
    fn list_summaries_extracts_and_sorts() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_summary_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());

        let mut s1 = Session::new("chat:1");
        s1.add_message("user", "Hello world from chat 1");
        let path1 = manager.file_path(&s1.key);
        fs::write(&path1, serde_json::to_string(&s1)?)?;

        let mut s2 = Session::new("chat:2");
        s2.add_message("user", "Second message here");
        s2.add_message("assistant", "Response");
        let path2 = manager.file_path(&s2.key);
        fs::write(&path2, serde_json::to_string(&s2)?)?;

        let summaries = manager.list_summaries();
        assert_eq!(summaries.len(), 2);
        assert!(summaries
            .iter()
            .any(|s| s.key == "chat:1" && s.first_user_message == Some("Hello world from chat 1".into())));
        assert!(summaries
            .iter()
            .any(|s| s.key == "chat:2" && s.message_count == 2));

        let summary1 = summaries.iter().find(|s| s.key == "chat:1").unwrap();
        assert_eq!(summary1.preview_title(10, "fallback"), "Hello w...");

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn list_summaries_paginated_returns_slice_and_total() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("openz_page_summary_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let manager = SessionManager::new(dir.clone());

        for i in 1..=5 {
            let mut s = Session::new(&format!("session:{}", i));
            s.add_message("user", &format!("hello from {}", i));
            s.updated_at = Utc::now() + chrono::Duration::seconds(i);
            let path = manager.file_path(&s.key);
            fs::write(&path, serde_json::to_string(&s)?)?;
        }

        let (page1, total1) = manager.list_summaries_paginated(0, 2);
        assert_eq!(total1, 5);
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].key, "session:5");
        assert_eq!(page1[1].key, "session:4");

        let (page2, total2) = manager.list_summaries_paginated(2, 2);
        assert_eq!(total2, 5);
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].key, "session:3");
        assert_eq!(page2[1].key, "session:2");

        let (page3, total3) = manager.list_summaries_paginated(4, 2);
        assert_eq!(total3, 5);
        assert_eq!(page3.len(), 1);
        assert_eq!(page3[0].key, "session:1");

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
