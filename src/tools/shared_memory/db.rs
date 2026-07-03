use anyhow::Result;
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub fn get_db_mutex() -> &'static tokio::sync::Mutex<()> {
    static DB_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    DB_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

pub fn get_shared_client() -> &'static reqwest::Client {
    static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    SHARED_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default()
    })
}

pub fn get_sqlite_db_path() -> PathBuf {
    #[cfg(test)]
    {
        static TEST_DB_PATH: OnceLock<PathBuf> = OnceLock::new();
        TEST_DB_PATH.get_or_init(|| {
            std::env::temp_dir().join(format!("openz_test_shared_memory_{}.db", uuid::Uuid::new_v4()))
        }).clone()
    }
    #[cfg(not(test))]
    {
        if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
            PathBuf::from(override_dir).join("memory.db")
        } else {
            crate::config::resolve_path("~/.openz/memory.db")
        }
    }
}

pub(crate) fn db_static() -> &'static OnceLock<Mutex<Connection>> {
    static DB: OnceLock<Mutex<Connection>> = OnceLock::new();
    &DB
}

pub fn init_db() -> Result<Connection> {
    let path = get_sqlite_db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;

    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;

    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap_or_else(|_| "ok".to_string());
    if integrity != "ok" {
        tracing::warn!("Memory database integrity check failed: {}. Recreating database.", integrity);
        drop(conn);
        let backup = path.with_extension("db.corrupt");
        let _ = std::fs::rename(&path, &backup);
        let _ = std::fs::remove_file(format!("{}.wal", path.display()));
        let _ = std::fs::remove_file(format!("{}.shm", path.display()));
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        create_schema(&conn)?;
        return Ok(conn);
    }

    create_schema(&conn)?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cognitive_memory (
            id TEXT PRIMARY KEY,
            text TEXT NOT NULL,
            embedding TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            workspace TEXT NOT NULL,
            tags TEXT NOT NULL,
            importance REAL NOT NULL,
            last_accessed TEXT NOT NULL,
            access_count INTEGER DEFAULT 1,
            decay_rate REAL DEFAULT 0.05
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS research_archive (
            id TEXT PRIMARY KEY,
            query TEXT NOT NULL,
            content TEXT NOT NULL,
            source TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            embedding TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS interaction_history (
            id TEXT PRIMARY KEY,
            session_key TEXT NOT NULL,
            query TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            success INTEGER DEFAULT 1,
            errors TEXT
        )",
        [],
    )?;
    Ok(())
}

pub fn with_db<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut Connection) -> Result<T>,
{
    if let Some(mtx) = db_static().get() {
        let mut guard = mtx.lock().map_err(|e| anyhow::anyhow!("Shared memory lock error: {}", e))?;
        return f(&mut guard);
    }
    let conn = init_db()?;
    let mtx = db_static().get_or_init(|| Mutex::new(conn));
    let mut guard = mtx.lock().map_err(|e| anyhow::anyhow!("Shared memory lock error: {}", e))?;
    f(&mut guard)
}

pub fn get_current_workspace() -> String {
    if let Ok(dir) = crate::config::loader::ACTIVE_WORKSPACE.try_with(|w| w.clone()) {
        if let Ok(abs_path) = std::fs::canonicalize(&dir) {
            return abs_path.to_string_lossy().to_string();
        }
        return dir.to_string_lossy().to_string();
    }
    if let Ok(curr_dir) = std::env::current_dir() {
        if let Ok(abs_path) = std::fs::canonicalize(&curr_dir) {
            return abs_path.to_string_lossy().to_string();
        }
        return curr_dir.to_string_lossy().to_string();
    }
    ".".to_string()
}

