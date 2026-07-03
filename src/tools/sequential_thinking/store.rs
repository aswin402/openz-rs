use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

// ─── Data types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtData {
    pub thought: String,
    #[serde(rename = "thoughtNumber")]
    pub thought_number: usize,
    #[serde(rename = "totalThoughts")]
    pub total_thoughts: usize,
    #[serde(rename = "nextThoughtNeeded")]
    pub next_thought_needed: bool,

    #[serde(rename = "isRevision")]
    pub is_revision: Option<bool>,
    #[serde(rename = "revisesThought")]
    pub revises_thought: Option<usize>,
    #[serde(rename = "branchFromThought")]
    pub branch_from_thought: Option<usize>,
    #[serde(rename = "branchId")]
    pub branch_id: Option<String>,
    #[serde(rename = "needsMoreThoughts")]
    pub needs_more_thoughts: Option<bool>,

    #[serde(rename = "parentThoughts")]
    pub parent_thoughts: Option<Vec<usize>>,
    pub assumptions: Option<Vec<String>>,
    #[serde(rename = "verifiedAssumptions")]
    pub verified_assumptions: Option<Vec<String>>,
    #[serde(rename = "confidenceScore")]
    pub confidence_score: Option<f64>,
    pub criticism: Option<String>,
    pub hypothesis: Option<String>,
    #[serde(rename = "verificationMethod")]
    pub verification_method: Option<String>,
    #[serde(rename = "leftToBeDone")]
    pub left_to_be_done: Option<Vec<String>>,
    pub timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResult {
    #[serde(rename = "thoughtNumber")]
    pub thought_number: usize,
    #[serde(rename = "totalThoughts")]
    pub total_thoughts: usize,
    #[serde(rename = "nextThoughtNeeded")]
    pub next_thought_needed: bool,
    pub branches: Vec<String>,
    #[serde(rename = "thoughtHistoryLength")]
    pub thought_history_length: usize,
    #[serde(rename = "thoughtGraphMermaid")]
    pub thought_graph_mermaid: String,
    #[serde(rename = "confidenceHistory")]
    pub confidence_history: Vec<Option<f64>>,
    #[serde(rename = "leftToBeDone")]
    pub left_to_be_done: Vec<String>,
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QualityReport {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "totalThoughts")]
    pub total_thoughts: usize,
    #[serde(rename = "averageConfidence")]
    pub average_confidence: f64,
    #[serde(rename = "assumptionsCount")]
    pub assumptions_count: usize,
    #[serde(rename = "verifiedAssumptionsCount")]
    pub verified_assumptions_count: usize,
    #[serde(rename = "verifiedAssumptionsRatio")]
    pub verified_assumptions_ratio: f64,
    #[serde(rename = "contradictionsCount")]
    pub contradictions_count: usize,
    pub contradictions: Vec<String>,
    #[serde(rename = "loopDetected")]
    pub loop_detected: bool,
    #[serde(rename = "loopPath")]
    pub loop_path: Option<Vec<usize>>,
    #[serde(rename = "qualityScore")]
    pub quality_score: f64,
    pub grade: String,
}

// ─── Thought store trait ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "totalThoughts")]
    pub total_thoughts: usize,
}

pub trait ThoughtStore: Send {
    fn save_thought(&mut self, session_id: &str, thought: &ThoughtData) -> Result<(), String>;
    fn load_session(&self, session_id: &str) -> Result<Vec<ThoughtData>, String>;
    #[allow(dead_code)]
    fn list_sessions(&self) -> Result<Vec<SessionInfo>, String>;
    #[allow(dead_code)]
    fn delete_session(&mut self, session_id: &str) -> Result<(), String>;
}

// ─── In-memory store ─────────────────────────────────────────────

pub struct MemoryThoughtStore {
    sessions: HashMap<String, Vec<ThoughtData>>,
    created_at: HashMap<String, DateTime<Utc>>,
    updated_at: HashMap<String, DateTime<Utc>>,
}

impl MemoryThoughtStore {
    pub fn new() -> Self {
        Self { sessions: HashMap::new(), created_at: HashMap::new(), updated_at: HashMap::new() }
    }
}

impl ThoughtStore for MemoryThoughtStore {
    fn save_thought(&mut self, session_id: &str, thought: &ThoughtData) -> Result<(), String> {
        self.sessions.entry(session_id.to_string()).or_default().push(thought.clone());
        let now = Utc::now();
        self.created_at.entry(session_id.to_string()).or_insert(now);
        self.updated_at.insert(session_id.to_string(), now);
        Ok(())
    }
    fn load_session(&self, session_id: &str) -> Result<Vec<ThoughtData>, String> {
        Ok(self.sessions.get(session_id).cloned().unwrap_or_default())
    }
    fn list_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        let mut list: Vec<SessionInfo> = self
            .sessions
            .iter()
            .map(|(id, thoughts)| {
                let created = self.created_at.get(id).copied().unwrap_or_else(Utc::now);
                let updated = self.updated_at.get(id).copied().unwrap_or_else(Utc::now);
                SessionInfo { id: id.clone(), created_at: created, updated_at: updated, total_thoughts: thoughts.len() }
            })
            .collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(list)
    }
    fn delete_session(&mut self, session_id: &str) -> Result<(), String> {
        self.sessions.remove(session_id);
        self.created_at.remove(session_id);
        self.updated_at.remove(session_id);
        Ok(())
    }
}

// ─── SQLite store ────────────────────────────────────────────────

pub struct SqliteThoughtStore {
    conn: Connection,
}

impl SqliteThoughtStore {
    pub fn new(conn: Connection) -> Result<Self, String> {
        conn.execute("PRAGMA foreign_keys = ON;", []).map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY, created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL, total_thoughts INTEGER DEFAULT 0
            );", [],
        ).map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS thoughts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                thought_number INTEGER NOT NULL,
                thought_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );", [],
        ).map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_thoughts_session ON thoughts(session_id);", [],
        ).map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }
}

impl ThoughtStore for SqliteThoughtStore {
    fn save_thought(&mut self, session_id: &str, thought: &ThoughtData) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO sessions (id, created_at, updated_at, total_thoughts)
             VALUES (?1, ?2, ?3, 1) ON CONFLICT(id) DO UPDATE SET
             updated_at = excluded.updated_at, total_thoughts = total_thoughts + 1;",
            params![session_id, now, now],
        ).map_err(|e| e.to_string())?;
        let thought_json = serde_json::to_string(thought).map_err(|e| e.to_string())?;
        self.conn.execute(
            "INSERT INTO thoughts (session_id, thought_number, thought_json, created_at) VALUES (?1, ?2, ?3, ?4);",
            params![session_id, thought.thought_number, thought_json, now],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn load_session(&self, session_id: &str) -> Result<Vec<ThoughtData>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT thought_json FROM thoughts WHERE session_id = ?1 ORDER BY thought_number ASC, id ASC;",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![session_id], |row| {
            let json_str: String = row.get(0)?;
            Ok(json_str)
        }).map_err(|e| e.to_string())?;
        let mut thoughts = Vec::new();
        for r in rows {
            let json_str = r.map_err(|e| e.to_string())?;
            thoughts.push(serde_json::from_str(&json_str).map_err(|e| e.to_string())?);
        }
        Ok(thoughts)
    }
    fn list_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, updated_at, total_thoughts FROM sessions ORDER BY updated_at DESC;",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?))
        }).map_err(|e| e.to_string())?;
        let mut sessions = Vec::new();
        for r in rows {
            let (id, created_str, updated_str, total) = r.map_err(|e| e.to_string())?;
            let created_at = DateTime::parse_from_rfc3339(&created_str).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_str).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
            sessions.push(SessionInfo { id, created_at, updated_at, total_thoughts: total as usize });
        }
        Ok(sessions)
    }
    fn delete_session(&mut self, session_id: &str) -> Result<(), String> {
        self.conn.execute("DELETE FROM sessions WHERE id = ?1;", params![session_id]).map_err(|e| e.to_string())?;
        Ok(())
    }
}

// ─── DB path helper ──────────────────────────────────────────────

pub fn get_db_path() -> PathBuf {
    if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
        PathBuf::from(override_dir).join("thoughts.db")
    } else {
        crate::config::resolve_path("~/.openz/thoughts.db")
    }
}

// ─── Shared Database Lock and Store Accessors ────────────────────

pub fn get_db_mutex() -> &'static tokio::sync::Mutex<()> {
    static DB_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    DB_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

static STORE: OnceLock<Arc<tokio::sync::Mutex<Box<dyn ThoughtStore>>>> = OnceLock::new();

pub fn get_store() -> &'static Arc<tokio::sync::Mutex<Box<dyn ThoughtStore>>> {
    STORE.get_or_init(|| {
        let db_path = get_db_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let store: Box<dyn ThoughtStore> = match Connection::open(&db_path) {
            Ok(conn) => {
                match SqliteThoughtStore::new(conn) {
                    Ok(s) => Box::new(s),
                    Err(_) => Box::new(MemoryThoughtStore::new()),
                }
            }
            Err(_) => Box::new(MemoryThoughtStore::new()),
        };
        Arc::new(tokio::sync::Mutex::new(store))
    })
}
