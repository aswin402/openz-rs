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
                            i, stored, calculated
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

    fn lock_path(&self, key: &str) -> PathBuf {
        let safe_key = key.replace(":", "_").replace("/", "_").replace("\\", "_");
        self.dir.join(format!("{}.lock", safe_key))
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
        let session: Session = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse session file at {:?}", path))?;
        session.verify_hash_chain()?;
        Ok(session)
    }

    pub async fn load_async(&self, key: &str) -> Result<Session> {
        let path = self.file_path(key);
        tokio::task::spawn_blocking(move || {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read session file at {:?}", path))?;
            let session: Session = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse session file at {:?}", path))?;
            session.verify_hash_chain()?;
            Ok(session)
        })
        .await?
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
}
