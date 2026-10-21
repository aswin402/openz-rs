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
    crate::core::http::global_http_client()
}

pub fn get_sqlite_db_path() -> PathBuf {
    #[cfg(test)]
    {
        static TEST_DB_PATH: OnceLock<PathBuf> = OnceLock::new();
        TEST_DB_PATH
            .get_or_init(|| {
                std::env::temp_dir().join(format!(
                    "openz_test_shared_memory_{}.db",
                    uuid::Uuid::new_v4()
                ))
            })
            .clone()
    }
    #[cfg(not(test))]
    {
        crate::config::loader::runtime_db_path("memory.db")
    }
}

pub(crate) fn db_static() -> &'static OnceLock<Mutex<Connection>> {
    static DB: OnceLock<Mutex<Connection>> = OnceLock::new();
    &DB
}

pub fn init_db() -> Result<Connection> {
    let path = get_sqlite_db_path();
    match path.parent() {
        Some(parent) => fs::create_dir_all(parent)?,
        None => {}
    }
    let conn = Connection::open(&path)?;

    crate::core::sqlite::apply_standard_pragmas(&conn)?;

    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap_or_else(|_| "ok".to_string());
    match integrity.as_str() {
        "ok" => {
            create_schema(&conn)?;
            Ok(conn)
        }
        _ => {
            tracing::warn!(
                "Memory database integrity check failed: {}. Recreating database.",
                integrity
            );
            drop(conn);
            let backup = path.with_extension("db.corrupt");
            let _ = std::fs::rename(&path, &backup);
            let _ = std::fs::remove_file(format!("{}.wal", path.display()));
            let _ = std::fs::remove_file(format!("{}.shm", path.display()));
            let conn = Connection::open(&path)?;
            crate::core::sqlite::apply_standard_pragmas(&conn)?;
            create_schema(&conn)?;
            Ok(conn)
        }
    }
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
    conn.execute(
        "CREATE TABLE IF NOT EXISTS source_bookmarks (
            id TEXT PRIMARY KEY,
            label TEXT NOT NULL,
            kind TEXT NOT NULL,
            uri TEXT NOT NULL,
            aliases TEXT NOT NULL,
            summary TEXT NOT NULL,
            trust_score REAL NOT NULL DEFAULT 0.5,
            last_checked TEXT,
            stale_after_secs INTEGER NOT NULL DEFAULT 604800,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            use_count INTEGER NOT NULL DEFAULT 0,
            UNIQUE(uri)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_source_bookmarks_label ON source_bookmarks(label)",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS research_briefs (
            id TEXT PRIMARY KEY,
            topic TEXT NOT NULL UNIQUE,
            summary TEXT NOT NULL,
            source_ids TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.5,
            stale_after_secs INTEGER NOT NULL DEFAULT 86400,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            use_count INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS web_fetch_cache (
            url TEXT PRIMARY KEY,
            body_text TEXT NOT NULL,
            etag TEXT,
            last_modified TEXT,
            cache_control TEXT,
            fetched_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            status_code INTEGER NOT NULL,
            use_count INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workflow_cards (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            triggers TEXT NOT NULL,
            summary TEXT NOT NULL,
            steps_json TEXT NOT NULL,
            preconditions TEXT NOT NULL,
            verification TEXT NOT NULL,
            risk TEXT NOT NULL DEFAULT 'normal',
            status TEXT NOT NULL DEFAULT 'draft',
            success_count INTEGER NOT NULL DEFAULT 0,
            failure_count INTEGER NOT NULL DEFAULT 0,
            last_used TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workflow_runs (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL,
            session_key TEXT NOT NULL,
            task TEXT NOT NULL,
            success INTEGER NOT NULL,
            error TEXT,
            timestamp TEXT NOT NULL,
            FOREIGN KEY(workflow_id) REFERENCES workflow_cards(id)
        )",
        [],
    )?;
    Ok(())
}

pub fn with_db<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut Connection) -> Result<T>,
{
    crate::core::sqlite::with_static_db(db_static(), "Shared memory", init_db, f)
}

fn canonical_or_lossy(dir: &std::path::Path) -> String {
    std::fs::canonicalize(dir)
        .unwrap_or_else(|_| dir.to_path_buf())
        .to_string_lossy()
        .to_string()
}

pub fn get_current_workspace() -> String {
    crate::config::loader::ACTIVE_WORKSPACE
        .try_with(|w| canonical_or_lossy(w))
        .or_else(|_| std::env::current_dir().map(|curr_dir| canonical_or_lossy(&curr_dir)))
        .unwrap_or_else(|_| ".".to_string())
}
