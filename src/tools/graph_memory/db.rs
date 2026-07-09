use anyhow::{anyhow, Result};
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Mutex, OnceLock};

// ─── Constants ───────────────────────────────────────────────────

#[allow(dead_code)]
pub(crate) const DB_FILENAME: &str = "graph_memory.db";

pub(crate) const SCHEMA_DDL: &str = "
         CREATE TABLE IF NOT EXISTS graph_nodes (
             name TEXT NOT NULL,
             entity_type TEXT NOT NULL,
             observations TEXT NOT NULL DEFAULT '[]',
             user_id TEXT NOT NULL DEFAULT '*',
             session_id TEXT NOT NULL DEFAULT '*',
             agent_id TEXT NOT NULL DEFAULT '*',
             created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
             PRIMARY KEY (name, user_id, session_id, agent_id)
         );
         CREATE TABLE IF NOT EXISTS graph_edges (
             from_name TEXT NOT NULL,
             to_name TEXT NOT NULL,
             relation_type TEXT NOT NULL,
             valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
             valid_until TEXT,
             confidence REAL NOT NULL DEFAULT 1.0,
             user_id TEXT NOT NULL DEFAULT '*',
             session_id TEXT NOT NULL DEFAULT '*',
             agent_id TEXT NOT NULL DEFAULT '*',
             PRIMARY KEY (from_name, to_name, relation_type, user_id, session_id, agent_id, valid_from)
         );
         CREATE INDEX IF NOT EXISTS idx_graph_nodes_scope ON graph_nodes(user_id, session_id, agent_id);
         CREATE INDEX IF NOT EXISTS idx_graph_edges_scope ON graph_edges(user_id, session_id, agent_id);
         CREATE INDEX IF NOT EXISTS idx_graph_edges_active ON graph_edges(valid_until);

         CREATE TABLE IF NOT EXISTS episodic_logs (
             id TEXT NOT NULL,
             task_description TEXT NOT NULL,
             execution_status TEXT NOT NULL,
             steps_taken TEXT NOT NULL,
             error_message TEXT,
             reflection TEXT,
             created_at TEXT NOT NULL,
             user_id TEXT NOT NULL DEFAULT '*',
             session_id TEXT NOT NULL DEFAULT '*',
             agent_id TEXT NOT NULL DEFAULT '*',
             PRIMARY KEY (id, user_id, session_id, agent_id)
         );
         CREATE TABLE IF NOT EXISTS reflection_memory (
             id TEXT NOT NULL,
             task_description TEXT NOT NULL,
             status TEXT NOT NULL,
             attempt_number INTEGER NOT NULL DEFAULT 1,
             steps_taken TEXT NOT NULL,
             error_encountered TEXT,
             root_cause TEXT,
             solution_applied TEXT,
             reflection TEXT NOT NULL,
             created_at TEXT NOT NULL,
             user_id TEXT NOT NULL DEFAULT '*',
             session_id TEXT NOT NULL DEFAULT '*',
             agent_id TEXT NOT NULL DEFAULT '*',
             PRIMARY KEY (id, user_id, session_id, agent_id)
         );
         CREATE TABLE IF NOT EXISTS tool_performance (
             tool_name TEXT NOT NULL,
             model_name TEXT NOT NULL,
             task_type TEXT NOT NULL,
             success_count INTEGER NOT NULL DEFAULT 0,
             failure_count INTEGER NOT NULL DEFAULT 0,
             average_latency REAL NOT NULL DEFAULT 0.0,
             last_used TEXT NOT NULL,
             user_id TEXT NOT NULL DEFAULT '*',
             session_id TEXT NOT NULL DEFAULT '*',
             agent_id TEXT NOT NULL DEFAULT '*',
             PRIMARY KEY (tool_name, model_name, task_type, user_id, session_id, agent_id)
         );
         CREATE TABLE IF NOT EXISTS shared_agent_memory (
             memory_key TEXT NOT NULL,
             memory_value TEXT NOT NULL,
             source_agent TEXT NOT NULL,
             target_agents TEXT NOT NULL DEFAULT '[]',
             importance REAL NOT NULL DEFAULT 1.0,
             timestamp TEXT NOT NULL,
             user_id TEXT NOT NULL DEFAULT '*',
             session_id TEXT NOT NULL DEFAULT '*',
             agent_id TEXT NOT NULL DEFAULT '*',
             PRIMARY KEY (memory_key, user_id, session_id, agent_id)
         );
         CREATE TABLE IF NOT EXISTS semantic_metadata (
             node_id TEXT NOT NULL,
             raw_text TEXT NOT NULL,
             embedding BLOB,
             timestamp TEXT NOT NULL,
             importance REAL NOT NULL DEFAULT 0.8,
             user_id TEXT NOT NULL DEFAULT '*',
             session_id TEXT NOT NULL DEFAULT '*',
             agent_id TEXT NOT NULL DEFAULT '*',
             valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
             valid_until TEXT,
             PRIMARY KEY (node_id, valid_from, user_id, session_id, agent_id)
         );
         CREATE INDEX IF NOT EXISTS idx_semantic_scope ON semantic_metadata(user_id, session_id, agent_id);
         CREATE INDEX IF NOT EXISTS idx_semantic_valid ON semantic_metadata(valid_until);
         CREATE INDEX IF NOT EXISTS idx_episodic_scope ON episodic_logs(user_id, session_id, agent_id);
         CREATE INDEX IF NOT EXISTS idx_reflection_scope ON reflection_memory(user_id, session_id, agent_id);
         CREATE INDEX IF NOT EXISTS idx_toolperf_scope ON tool_performance(user_id, session_id, agent_id);
         CREATE INDEX IF NOT EXISTS idx_shared_scope ON shared_agent_memory(user_id, session_id, agent_id);
         CREATE TABLE IF NOT EXISTS code_elements (
             element_id    TEXT PRIMARY KEY,
             file_path     TEXT NOT NULL,
             element_type  TEXT NOT NULL,
             name          TEXT NOT NULL,
             signature     TEXT NOT NULL,
             ast_json      TEXT,
             parent_id     TEXT,
             start_line    INTEGER NOT NULL,
             end_line      INTEGER NOT NULL,
             user_id       TEXT NOT NULL DEFAULT '*',
             session_id    TEXT NOT NULL DEFAULT '*',
             agent_id      TEXT NOT NULL DEFAULT '*'
         );
         CREATE TABLE IF NOT EXISTS code_calls (
             caller_id     TEXT NOT NULL,
             callee_id     TEXT NOT NULL,
             call_site     TEXT,
             PRIMARY KEY (caller_id, callee_id)
         );
         CREATE INDEX IF NOT EXISTS idx_code_elements_scope ON code_elements(user_id, session_id, agent_id);
         CREATE INDEX IF NOT EXISTS idx_code_elements_file ON code_elements(file_path);
         CREATE INDEX IF NOT EXISTS idx_code_elements_name ON code_elements(name);
         CREATE TABLE IF NOT EXISTS repo_evolution (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             file_path TEXT NOT NULL,
             version TEXT NOT NULL,
             commit_hash TEXT,
             author TEXT,
             change_type TEXT NOT NULL,
             summary TEXT NOT NULL,
             bug_introduced INTEGER NOT NULL DEFAULT 0,
             bug_fixed INTEGER NOT NULL DEFAULT 0,
             created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS semantic_fts USING fts5(
             node_id UNINDEXED,
             raw_text,
             tokenize='porter'
         );
";

// ─── Shared DB static ──────────────────────────────────────────

pub(crate) fn db_static() -> &'static OnceLock<Mutex<Connection>> {
    static DB: OnceLock<Mutex<Connection>> = OnceLock::new();
    &DB
}

pub(crate) fn init_db() -> Result<Connection> {
    let path = get_db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path).unwrap_or_else(|_| Connection::open_in_memory().unwrap());
    conn.execute_batch(&format!(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; {}",
        SCHEMA_DDL
    ))?;
    Ok(conn)
}

// ─── DB helpers ─────────────────────────────────────────────────

pub(crate) fn get_db_path() -> std::path::PathBuf {
    #[cfg(test)]
    {
        static TEST_DB_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
        TEST_DB_PATH
            .get_or_init(|| {
                std::env::temp_dir().join(format!(
                    "openz_test_graph_memory_{}.db",
                    uuid::Uuid::new_v4()
                ))
            })
            .clone()
    }
    #[cfg(not(test))]
    {
        crate::config::loader::runtime_db_path(DB_FILENAME)
    }
}

pub(crate) fn with_db<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&Connection) -> Result<T>,
{
    if let Some(mtx) = db_static().get() {
        let guard = mtx
            .lock()
            .map_err(|e| anyhow!("Graph memory lock error: {}", e))?;
        return f(&guard);
    }
    let conn = init_db()?;
    let mtx = db_static().get_or_init(|| Mutex::new(conn));
    let guard = mtx
        .lock()
        .map_err(|e| anyhow!("Graph memory lock error: {}", e))?;
    f(&guard)
}

// ─── Scope helpers ──────────────────────────────────────────────

pub(crate) fn scope_from_args(args: &Value) -> (String, String, String) {
    let user_id = args
        .get("userId")
        .and_then(|v| v.as_str())
        .unwrap_or("*")
        .to_string();
    let session_id = args
        .get("sessionId")
        .and_then(|v| v.as_str())
        .unwrap_or("*")
        .to_string();
    let agent_id = args
        .get("agentId")
        .and_then(|v| v.as_str())
        .unwrap_or("*")
        .to_string();
    (user_id, session_id, agent_id)
}
