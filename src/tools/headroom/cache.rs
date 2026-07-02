use anyhow::{anyhow, Result};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::OnceLock;

// ─── DB path & connection ───────────────────────────────────────

fn get_db_path() -> PathBuf {
    if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
        PathBuf::from(override_dir).join("ccr_cache.db")
    } else {
        crate::config::resolve_path("~/.openz/ccr_cache.db")
    }
}

pub fn get_cache_connection() -> Result<std::sync::MutexGuard<'static, Connection>> {
    static DB: OnceLock<std::sync::Mutex<Connection>> = OnceLock::new();
    let mtx = DB.get_or_init(|| {
        let path = get_db_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path).unwrap_or_else(|_| {
            Connection::open_in_memory().unwrap()
        });
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
             CREATE TABLE IF NOT EXISTS cache_entries (
                 ccr_id TEXT PRIMARY KEY,
                 content TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 accessed_at TEXT NOT NULL,
                 size_bytes INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS compression_log (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 tool_name TEXT NOT NULL,
                 original_size INTEGER NOT NULL,
                 compressed_size INTEGER NOT NULL,
                 original_tokens INTEGER NOT NULL,
                 compressed_tokens INTEGER NOT NULL,
                 content_type TEXT NOT NULL,
                 model_hint TEXT,
                 created_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_cache_accessed ON cache_entries(accessed_at);",
        ).ok();
        std::sync::Mutex::new(conn)
    });
    Ok(mtx.lock().map_err(|e| anyhow!("Cache lock error: {}", e))?)
}
