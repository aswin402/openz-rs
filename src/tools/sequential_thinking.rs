use crate::tools::Tool;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::Arc;

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

trait ThoughtStore: Send {
    fn save_thought(&mut self, session_id: &str, thought: &ThoughtData) -> Result<(), String>;
    fn load_session(&self, session_id: &str) -> Result<Vec<ThoughtData>, String>;
    #[allow(dead_code)]
    fn list_sessions(&self) -> Result<Vec<SessionInfo>, String>;
    #[allow(dead_code)]
    fn delete_session(&mut self, session_id: &str) -> Result<(), String>;
}

// ─── In-memory store ─────────────────────────────────────────────

struct MemoryThoughtStore {
    sessions: HashMap<String, Vec<ThoughtData>>,
    created_at: HashMap<String, DateTime<Utc>>,
    updated_at: HashMap<String, DateTime<Utc>>,
}

impl MemoryThoughtStore {
    fn new() -> Self {
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

struct SqliteThoughtStore {
    conn: Connection,
}

impl SqliteThoughtStore {
    fn new(conn: Connection) -> Result<Self, String> {
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

fn get_db_path() -> PathBuf {
    if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
        PathBuf::from(override_dir).join("thoughts.db")
    } else {
        crate::config::resolve_path("~/.openz/thoughts.db")
    }
}

// ─── Engine (shared mutable state) ───────────────────────────────

struct SequentialThinkingEngine {
    store: Box<dyn ThoughtStore>,
    current_session_id: String,
    thought_history: Vec<ThoughtData>,
    branches: HashMap<String, Vec<ThoughtData>>,
}

static ENGINE: OnceLock<Arc<tokio::sync::Mutex<SequentialThinkingEngine>>> = OnceLock::new();

fn get_engine() -> &'static Arc<tokio::sync::Mutex<SequentialThinkingEngine>> {
    ENGINE.get_or_init(|| {
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
        Arc::new(tokio::sync::Mutex::new(SequentialThinkingEngine {
            store,
            current_session_id: String::new(),
            thought_history: Vec::new(),
            branches: HashMap::new(),
        }))
    })
}

impl SequentialThinkingEngine {
    fn load_session(&mut self, session_id: &str) -> Result<(), String> {
        if self.current_session_id != session_id {
            let thoughts = self.store.load_session(session_id)?;
            self.current_session_id = session_id.to_string();
            self.thought_history = thoughts;
            self.branches.clear();
            for t in &self.thought_history {
                if let (Some(_), Some(branch_id)) = (t.branch_from_thought, t.branch_id.as_ref()) {
                    self.branches.entry(branch_id.clone()).or_default().push(t.clone());
                }
            }
        }
        Ok(())
    }

    fn process_thought(&mut self, mut input: ThoughtData) -> Result<ToolResult, String> {
        let session_id = match input.session_id.as_ref() {
            Some(id) => id.clone(),
            None => {
                let generated = uuid::Uuid::new_v4().to_string();
                input.session_id = Some(generated.clone());
                generated
            }
        };
        self.load_session(&session_id)?;
        if input.thought_number > input.total_thoughts {
            input.total_thoughts = input.thought_number;
        }
        if input.timestamp.is_none() {
            input.timestamp = Some(Utc::now());
        }
        self.store.save_thought(&session_id, &input)?;
        if let (Some(_), Some(branch_id)) = (input.branch_from_thought, input.branch_id.as_ref()) {
            self.branches.entry(branch_id.clone()).or_default().push(input.clone());
        }

        let thought_number = input.thought_number;
        let total_thoughts = input.total_thoughts;
        let next_thought_needed = input.next_thought_needed;
        let left_to_be_done = input.left_to_be_done.clone().unwrap_or_default();

        self.thought_history.push(input);

        let branches = self.branches.keys().cloned().collect::<Vec<String>>();
        let confidence_history = self.thought_history.iter().map(|t| t.confidence_score).collect();
        let thought_graph_mermaid = generate_mermaid(&self.thought_history);

        Ok(ToolResult {
            thought_number,
            total_thoughts,
            next_thought_needed,
            branches,
            thought_history_length: self.thought_history.len(),
            thought_graph_mermaid,
            confidence_history,
            left_to_be_done,
            session_id,
        })
    }
}

// ─── Mermaid generation ──────────────────────────────────────────

fn generate_mermaid(thoughts: &[ThoughtData]) -> String {
    let mut mermaid = String::from("graph TD\n");
    mermaid.push_str("    classDef revision fill:#fafd7c,stroke:#d4b200,stroke-width:2px,color:#000;\n");
    mermaid.push_str("    classDef branch fill:#a1e887,stroke:#3b7a14,stroke-width:2px,color:#000;\n");
    mermaid.push_str("    classDef hypothesis fill:#d1b3ff,stroke:#6a3d9a,stroke-width:2px,color:#000;\n");
    mermaid.push_str("    classDef standard fill:#a5ccf7,stroke:#265c96,stroke-width:2px,color:#000;\n\n");

    for (i, t) in thoughts.iter().enumerate() {
        let id = format!("T{}", t.thought_number);
        let mut preview: String = t.thought.chars().take(30).collect();
        preview = preview.replace('\"', "'").replace('[', "(").replace(']', ")");
        if t.thought.len() > 30 { preview.push_str("..."); }

        let class = if t.is_revision.unwrap_or(false) { "revision" }
        else if t.branch_from_thought.is_some() { "branch" }
        else if t.hypothesis.is_some() { "hypothesis" }
        else { "standard" };

        mermaid.push_str(&format!("    {id}[\"T{num}: {preview}\"]\n", id = id, num = t.thought_number, preview = preview));
        mermaid.push_str(&format!("    class {id} {class}\n", id = id, class = class));

        if let Some(ref parents) = t.parent_thoughts {
            if !parents.is_empty() {
                for parent in parents {
                    mermaid.push_str(&format!("    T{parent} --> {id}\n", parent = parent, id = id));
                }
                continue;
            }
        }
        if let Some(branch_from) = t.branch_from_thought {
            mermaid.push_str(&format!("    T{branch_from} --> {id}\n", branch_from = branch_from, id = id));
        } else if t.is_revision.unwrap_or(false) {
            if let Some(revises) = t.revises_thought {
                mermaid.push_str(&format!("    T{revises} -.->|revises| {id}\n", revises = revises, id = id));
            }
        } else if i > 0 {
            let prev = thoughts[i - 1].thought_number;
            mermaid.push_str(&format!("    T{prev} --> {id}\n", prev = prev, id = id));
        }
    }
    mermaid
}

// ─── Quality calculation ─────────────────────────────────────────

fn detect_cycle(thoughts: &[ThoughtData]) -> Option<Vec<usize>> {
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for t in thoughts {
        let mut deps = Vec::new();
        if let Some(ref parents) = t.parent_thoughts { deps.extend(parents.iter().copied()); }
        if let Some(branch_from) = t.branch_from_thought { deps.push(branch_from); }
        if let Some(revises) = t.revises_thought { deps.push(revises); }
        adj.insert(t.thought_number, deps);
    }
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut path = Vec::new();
    for &node in adj.keys() {
        if !visited.contains(&node) {
            if let Some(cycle) = dfs_cycle(node, &adj, &mut visited, &mut rec_stack, &mut path) {
                return Some(cycle);
            }
        }
    }
    None
}

fn dfs_cycle(
    node: usize, adj: &HashMap<usize, Vec<usize>>,
    visited: &mut HashSet<usize>, rec_stack: &mut HashSet<usize>, path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    visited.insert(node);
    rec_stack.insert(node);
    path.push(node);
    if let Some(neighbors) = adj.get(&node) {
        for &neighbor in neighbors {
            if !visited.contains(&neighbor) {
                if let Some(cycle) = dfs_cycle(neighbor, adj, visited, rec_stack, path) { return Some(cycle); }
            } else if rec_stack.contains(&neighbor) {
                if let Some(pos) = path.iter().position(|&x| x == neighbor) {
                    let mut cycle_path = path[pos..].to_vec();
                    cycle_path.push(neighbor);
                    return Some(cycle_path);
                }
            }
        }
    }
    rec_stack.remove(&node);
    path.pop();
    None
}

fn calculate_quality(session_id: &str, thoughts: &[ThoughtData]) -> QualityReport {
    let total_thoughts = thoughts.len();

    let confidences: Vec<f64> = thoughts.iter().filter_map(|t| t.confidence_score).collect();
    let average_confidence = if confidences.is_empty() { 0.75 } else { confidences.iter().sum::<f64>() / confidences.len() as f64 };

    let mut assumed = HashSet::new();
    let mut verified = HashSet::new();
    let mut refuted = HashSet::new();

    for t in thoughts {
        if let Some(ref ass) = t.assumptions {
            for a in ass { assumed.insert(a.trim().to_lowercase()); }
        }
        if let Some(ref ver) = t.verified_assumptions {
            for v in ver {
                let vc = v.trim().to_lowercase();
                if vc.contains("refuted") || vc.contains("false") || vc.contains("falsified") {
                    let core = vc.replace("refuted:", "").replace("refuted", "").replace("false:", "").replace("false", "").replace("falsified:", "").replace("falsified", "").trim().to_string();
                    refuted.insert(core);
                } else {
                    let core = vc.replace("verified:", "").replace("verified", "").trim().to_string();
                    verified.insert(core);
                    verified.insert(vc);
                }
            }
        }
    }

    let assumptions_count = assumed.len();
    let verified_assumptions_count = assumed.iter().filter(|a| verified.contains(*a) || refuted.contains(*a)).count();
    let verified_assumptions_ratio = if assumptions_count == 0 { 1.0 } else { verified_assumptions_count as f64 / assumptions_count as f64 };

    let contradictions: Vec<String> = assumed.intersection(&refuted).map(|s| format!("Assumption '{}' is declared but refuted/falsified.", s)).collect();
    let contradictions_count = contradictions.len();
    let loop_path = detect_cycle(thoughts);
    let loop_detected = loop_path.is_some();

    let mut score = average_confidence * 40.0 + verified_assumptions_ratio * 40.0;
    if total_thoughts > 0 { score += 20.0; }
    score -= (contradictions_count as f64 * 20.0).min(40.0);
    if loop_detected { score -= 30.0; }
    let quality_score = score.clamp(0.0, 100.0);

    let grade = if quality_score >= 90.0 { "A" } else if quality_score >= 80.0 { "B" } else if quality_score >= 70.0 { "C" } else if quality_score >= 60.0 { "D" } else { "F" }.to_string();

    QualityReport { session_id: session_id.to_string(), total_thoughts, average_confidence, assumptions_count, verified_assumptions_count, verified_assumptions_ratio, contradictions_count, contradictions, loop_detected, loop_path, quality_score, grade }
}

// ─── Tool 1: SequentialThinkingTool ──────────────────────────────

pub struct SequentialThinkingTool;

#[async_trait::async_trait]
impl Tool for SequentialThinkingTool {
    fn name(&self) -> &str { "sequentialthinking" }

    fn description(&self) -> &str {
        "A detailed tool for dynamic and reflective problem-solving through thoughts. Supports branching, revisions, Graph of Thoughts (GoT) merging, and Clear Thought parameters."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "thought": { "type": "string", "description": "Your current thinking step" },
                "nextThoughtNeeded": { "type": "boolean", "description": "Whether another thought step is needed" },
                "thoughtNumber": { "type": "integer", "description": "Current thought number (starts at 1)" },
                "totalThoughts": { "type": "integer", "description": "Estimated total thoughts needed" },
                "isRevision": { "type": "boolean", "description": "Whether this revises previous thinking" },
                "revisesThought": { "type": "integer", "description": "Which thought number is being revised" },
                "branchFromThought": { "type": "integer", "description": "Thought number this branch originates from" },
                "branchId": { "type": "string", "description": "Identifier for the current branch" },
                "needsMoreThoughts": { "type": "boolean", "description": "Request to add more thoughts" },
                "parentThoughts": { "type": "array", "items": { "type": "integer" }, "description": "Multiple parent thought numbers for GoT merging" },
                "assumptions": { "type": "array", "items": { "type": "string" }, "description": "Assumptions made in this step" },
                "verifiedAssumptions": { "type": "array", "items": { "type": "string" }, "description": "Assumptions verified or refuted" },
                "confidenceScore": { "type": "number", "description": "Confidence in this reasoning line (0.0 to 1.0)" },
                "criticism": { "type": "string", "description": "Self-criticism of previous thoughts" },
                "hypothesis": { "type": "string", "description": "Hypothesis to be tested" },
                "verificationMethod": { "type": "string", "description": "Method to verify the hypothesis" },
                "leftToBeDone": { "type": "array", "items": { "type": "string" }, "description": "Items/tasks left to be done" },
                "sessionId": { "type": "string", "description": "Session identifier for the thinking session" }
            },
            "required": ["thought", "nextThoughtNeeded", "thoughtNumber", "totalThoughts"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let thought_data: ThoughtData = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid arguments: {}", e))?;

        let engine = get_engine();
        let mut guard = engine.lock().await;
        let result = guard.process_thought(thought_data)
            .map_err(|e| anyhow!("{}", e))?;
        Ok(serde_json::to_value(result).unwrap_or(Value::Null))
    }
}

// ─── Tool 2: AnalyzeGraphTool ────────────────────────────────────

pub struct AnalyzeGraphTool;

#[async_trait::async_trait]
impl Tool for AnalyzeGraphTool {
    fn name(&self) -> &str { "analyze_graph" }

    fn description(&self) -> &str {
        "Query and analyze the thought graph of a thinking session. Supports low_confidence, contradictions, unverified_assumptions, dead_branches, summary_stats, and quality_report queries."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "enum": ["low_confidence", "contradictions", "unverified_assumptions", "dead_branches", "summary_stats", "quality_report"],
                    "description": "The type of analysis/query to run against the thought graph"
                },
                "confidenceThreshold": { "type": "number", "default": 0.5, "description": "Confidence threshold for low_confidence filter" },
                "sessionId": { "type": "string", "description": "Session identifier to analyze (defaults to active session)" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let engine = get_engine();
        let mut guard = engine.lock().await;

        let session_id = arguments["sessionId"].as_str().map(String::from).unwrap_or_else(|| guard.current_session_id.clone());
        if session_id.is_empty() {
            return Err(anyhow!("No active session and no sessionId provided"));
        }
        guard.load_session(&session_id).map_err(|e| anyhow!("{}", e))?;

        let query = arguments["query"].as_str().ok_or_else(|| anyhow!("Missing query parameter"))?;

        match query {
            "low_confidence" => {
                let threshold = arguments["confidenceThreshold"].as_f64().unwrap_or(0.5);
                let low: Vec<ThoughtData> = guard.thought_history.iter()
                    .filter(|t| t.confidence_score.map(|c| c <= threshold).unwrap_or(false))
                    .cloned().collect();
                Ok(json!(low))
            }
            "contradictions" => {
                let mut assumed = HashSet::new();
                let mut refuted = HashSet::new();
                for t in &guard.thought_history {
                    if let Some(ref ass) = t.assumptions { for a in ass { assumed.insert(a.trim().to_lowercase()); } }
                    if let Some(ref ver) = t.verified_assumptions {
                        for v in ver {
                            let vc = v.trim().to_lowercase();
                            if vc.contains("refuted") || vc.contains("false") || vc.contains("falsified") {
                                refuted.insert(vc.replace("refuted:", "").replace("refuted", "").replace("false:", "").replace("false", "").replace("falsified:", "").replace("falsified", "").trim().to_string());
                            }
                        }
                    }
                }
                let contradictions: Vec<String> = assumed.intersection(&refuted).map(|s| format!("Assumption '{}' is assumed but has been refuted/falsified.", s)).collect();
                Ok(json!(contradictions))
            }
            "unverified_assumptions" => {
                let mut assumed = HashSet::new();
                let mut verified = HashSet::new();
                for t in &guard.thought_history {
                    if let Some(ref ass) = t.assumptions { for a in ass { assumed.insert(a.clone()); } }
                    if let Some(ref ver) = t.verified_assumptions {
                        for v in ver {
                            let vc = v.replace("verified:", "").replace("refuted:", "").replace("false:", "").trim().to_string();
                            verified.insert(vc);
                            verified.insert(v.clone());
                        }
                    }
                }
                Ok(json!(assumed.into_iter().filter(|a| !verified.contains(a)).collect::<Vec<String>>()))
            }
            "dead_branches" => {
                if guard.thought_history.is_empty() { return Ok(json!([])); }
                let last = &guard.thought_history[guard.thought_history.len() - 1];
                let mut main_chain = HashSet::new();
                let mut queue = vec![last.thought_number];
                while let Some(tn) = queue.pop() {
                    if main_chain.insert(tn) {
                        if let Some(t) = guard.thought_history.iter().find(|x| x.thought_number == tn) {
                            if let Some(ref parents) = t.parent_thoughts { queue.extend(parents.iter().copied()); }
                            if let Some(bf) = t.branch_from_thought { queue.push(bf); }
                            if let Some(rev) = t.revises_thought { queue.push(rev); }
                            if t.parent_thoughts.is_none() && t.branch_from_thought.is_none() && !t.is_revision.unwrap_or(false) && t.thought_number > 1 {
                                queue.push(t.thought_number - 1);
                            }
                        }
                    }
                }
                let dead: Vec<ThoughtData> = guard.thought_history.iter().filter(|t| !main_chain.contains(&t.thought_number)).cloned().collect();
                Ok(json!(dead))
            }
            "summary_stats" => {
                let report = calculate_quality(&session_id, &guard.thought_history);
                Ok(json!({
                    "sessionId": session_id,
                    "totalThoughts": guard.thought_history.len(),
                    "averageConfidence": report.average_confidence,
                    "branchesCount": guard.branches.len(),
                    "totalAssumptions": report.assumptions_count,
                    "totalVerifiedAssumptions": report.verified_assumptions_count,
                    "qualityScore": report.quality_score,
                    "grade": report.grade,
                }))
            }
            "quality_report" => {
                let report = calculate_quality(&session_id, &guard.thought_history);
                Ok(json!(report))
            }
            _ => Err(anyhow!("Unknown query type: {}", query)),
        }
    }
}

// ─── Tool 3: ExportSessionTool ───────────────────────────────────

pub struct ExportSessionTool;

#[async_trait::async_trait]
impl Tool for ExportSessionTool {
    fn name(&self) -> &str { "export_session" }

    fn description(&self) -> &str {
        "Export the reasoning session in various formats: mermaid graph, JSON Graph, markdown report, or Graphviz DOT format."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string", "enum": ["mermaid", "json", "markdown", "dot"],
                    "description": "The target export format"
                },
                "sessionId": { "type": "string", "description": "Session to export (defaults to active session)" }
            },
            "required": ["format"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let engine = get_engine();
        let mut guard = engine.lock().await;

        let session_id = arguments["sessionId"].as_str().map(String::from).unwrap_or_else(|| guard.current_session_id.clone());
        if session_id.is_empty() { return Err(anyhow!("No active session and no sessionId provided")); }
        guard.load_session(&session_id).map_err(|e| anyhow!("{}", e))?;

        let format = arguments["format"].as_str().ok_or_else(|| anyhow!("Missing format parameter"))?;

        match format {
            "mermaid" => {
                let mermaid_graph = generate_mermaid(&guard.thought_history);
                Ok(json!({ "format": "mermaid", "sessionId": session_id, "data": mermaid_graph }))
            }
            "json" => {
                let mut nodes = Vec::new();
                let mut edges = Vec::new();
                for (i, t) in guard.thought_history.iter().enumerate() {
                    nodes.push(json!({
                        "id": format!("T{}", t.thought_number), "thoughtNumber": t.thought_number,
                        "thought": t.thought, "confidenceScore": t.confidence_score, "timestamp": t.timestamp,
                    }));
                    if let Some(ref parents) = t.parent_thoughts {
                        for p in parents { edges.push(json!({ "source": format!("T{}", p), "target": format!("T{}", t.thought_number), "type": "parent" })); }
                        continue;
                    }
                    if let Some(bf) = t.branch_from_thought {
                        edges.push(json!({ "source": format!("T{}", bf), "target": format!("T{}", t.thought_number), "type": "branch" }));
                    } else if t.is_revision.unwrap_or(false) {
                        if let Some(rev) = t.revises_thought { edges.push(json!({ "source": format!("T{}", rev), "target": format!("T{}", t.thought_number), "type": "revision" })); }
                    } else if i > 0 {
                        edges.push(json!({ "source": format!("T{}", guard.thought_history[i - 1].thought_number), "target": format!("T{}", t.thought_number), "type": "standard" }));
                    }
                }
                Ok(json!({ "format": "json", "sessionId": session_id, "data": { "nodes": nodes, "edges": edges } }))
            }
            "markdown" => {
                let mut md = String::new();
                md.push_str(&format!("# Reasoning Session History - Session `{}`\n\n", session_id));
                for t in &guard.thought_history {
                    let kind = if t.is_revision.unwrap_or(false) { "Revision" } else if t.branch_from_thought.is_some() { "Branch" } else { "Thought" };
                    md.push_str(&format!("## {} {}\n", kind, t.thought_number));
                    if let Some(ts) = t.timestamp { md.push_str(&format!("*Timestamp: {}*\n\n", ts.format("%Y-%m-%d %H:%M:%S UTC"))); }
                    md.push_str(&format!("{}\n\n", t.thought));
                    if let Some(ref ass) = t.assumptions { if !ass.is_empty() { md.push_str("### Assumptions\n"); for a in ass { md.push_str(&format!("- 🤔 {}\n", a)); } md.push_str("\n"); } }
                    if let Some(ref ver) = t.verified_assumptions { if !ver.is_empty() { md.push_str("### Verified Assumptions\n"); for v in ver { md.push_str(&format!("- ✅ {}\n", v)); } md.push_str("\n"); } }
                    if let Some(conf) = t.confidence_score { md.push_str(&format!("*Confidence Score: {}/5 ({:.0}%)*\n\n", (conf * 5.0).round(), conf * 100.0)); }
                    if let Some(ref c) = t.criticism { md.push_str(&format!("> **🧐 Self-Criticism:** {}\n\n", c)); }
                    if let Some(ref h) = t.hypothesis { md.push_str(&format!("> **🔬 Hypothesis:** {}\n\n", h)); }
                    if let Some(ref vm) = t.verification_method { md.push_str(&format!("> **🧪 Verification:** {}\n\n", vm)); }
                    md.push_str("---\n\n");
                }
                Ok(json!({ "format": "markdown", "sessionId": session_id, "data": md }))
            }
            "dot" => {
                let mut dot = String::from("digraph G {\n  node [shape=box, style=filled, fontname=\"Arial\"];\n");
                for (i, t) in guard.thought_history.iter().enumerate() {
                    let id = format!("T{}", t.thought_number);
                    let preview: String = t.thought.chars().take(20).collect();
                    let color = if t.is_revision.unwrap_or(false) { "\"#fafd7c\"" } else if t.branch_from_thought.is_some() { "\"#a1e887\"" } else { "\"#a5ccf7\"" };
                    dot.push_str(&format!("  {} [label=\"T{}: {}...\", fillcolor={}];\n", id, t.thought_number, preview, color));
                    if let Some(ref parents) = t.parent_thoughts { for p in parents { dot.push_str(&format!("  T{} -> {};\n", p, id)); } continue; }
                    if let Some(bf) = t.branch_from_thought { dot.push_str(&format!("  T{} -> {};\n", bf, id)); }
                    else if t.is_revision.unwrap_or(false) { if let Some(rev) = t.revises_thought { dot.push_str(&format!("  T{} -> {} [style=dotted, label=\"revises\"];\n", rev, id)); } }
                    else if i > 0 { dot.push_str(&format!("  T{} -> {};\n", guard.thought_history[i - 1].thought_number, id)); }
                }
                dot.push_str("}\n");
                Ok(json!({ "format": "dot", "sessionId": session_id, "data": dot }))
            }
            _ => Err(anyhow!("Unknown format: {}", format)),
        }
    }
}

// ─── Tool 4: SummarizeReasoningTool ──────────────────────────────

pub struct SummarizeReasoningTool;

#[async_trait::async_trait]
impl Tool for SummarizeReasoningTool {
    fn name(&self) -> &str { "summarize_reasoning" }

    fn description(&self) -> &str {
        "Retrieve a structured summary and timeline of the reasoning chain for the specified session."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "sessionId": { "type": "string", "description": "Session to summarize (defaults to active session)" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let engine = get_engine();
        let mut guard = engine.lock().await;

        let session_id = arguments["sessionId"].as_str().map(String::from).unwrap_or_else(|| guard.current_session_id.clone());
        if session_id.is_empty() { return Err(anyhow!("No active session and no sessionId provided")); }
        guard.load_session(&session_id).map_err(|e| anyhow!("{}", e))?;

        let total_thoughts = guard.thought_history.len();
        let total_branches = guard.branches.len();

        let confidences: Vec<f64> = guard.thought_history.iter().filter_map(|t| t.confidence_score).collect();
        let average_confidence = if confidences.is_empty() { 0.0 } else { confidences.iter().sum::<f64>() / confidences.len() as f64 };

        let mut merge_points = Vec::new();
        for t in &guard.thought_history {
            if let Some(ref parents) = t.parent_thoughts { if parents.len() > 1 { merge_points.push(t.thought_number); } }
        }

        let mut assumed = HashSet::new();
        let mut verified = HashSet::new();
        for t in &guard.thought_history {
            if let Some(ref ass) = t.assumptions { for a in ass { assumed.insert(a.clone()); } }
            if let Some(ref ver) = t.verified_assumptions {
                for v in ver {
                    let vc = v.replace("verified:", "").replace("refuted:", "").replace("false:", "").trim().to_string();
                    verified.insert(vc); verified.insert(v.clone());
                }
            }
        }
        let unverified_assumptions: Vec<String> = assumed.into_iter().filter(|a| !verified.contains(a)).collect();

        let open_todos = guard.thought_history.last().and_then(|t| t.left_to_be_done.clone()).unwrap_or_default();

        let mut parts = Vec::new();
        for t in &guard.thought_history {
            let mut part = format!("T{}", t.thought_number);
            if let Some(bf) = t.branch_from_thought {
                let bid = t.branch_id.as_deref().unwrap_or("unknown");
                part = format!("{}(branch:{}, from:T{})", part, bid, bf);
            } else if let Some(ref parents) = t.parent_thoughts {
                if parents.len() > 1 {
                    let p_str: Vec<String> = parents.iter().map(|p| format!("T{}", p)).collect();
                    part = format!("{}(merge:{})", part, p_str.join("+"));
                }
            } else if t.is_revision.unwrap_or(false) {
                if let Some(rev) = t.revises_thought { part = format!("{}(revises:T{})", part, rev); }
            }
            parts.push(part);
        }

        Ok(json!({
            "sessionId": session_id, "totalThoughts": total_thoughts, "totalBranches": total_branches,
            "mergePoints": merge_points, "averageConfidence": average_confidence,
            "unverifiedAssumptions": unverified_assumptions, "openTodos": open_todos,
            "timeline": parts.join(" → "),
        }))
    }
}

// ─── Tool 5: TemplatesTool ───────────────────────────────────────

pub struct TemplatesTool;

#[async_trait::async_trait]
impl Tool for TemplatesTool {
    fn name(&self) -> &str { "reasoning_templates" }

    fn description(&self) -> &str {
        "Retrieve pre-structured reasoning templates to guide complex thinking processes. Includes divide-and-conquer, hypothesis testing, and devil's advocate reasoning."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "template": {
                    "type": "string", "enum": ["divide-and-conquer", "hypothesis-test", "devils-advocate", "all"],
                    "default": "all", "description": "The reasoning template to retrieve"
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let template_name = arguments["template"].as_str().unwrap_or("all");

        let divide_and_conquer = json!({
            "name": "Divide and Conquer", "id": "divide-and-conquer",
            "description": "Decompose a large, complex problem into smaller, independent sub-problems.",
            "recommendedSteps": [
                { "step": 1, "title": "Problem Scope & Boundary Analysis", "description": "Define the problem, inputs, outputs, and constraints.", "propertiesToSet": ["assumptions"] },
                { "step": 2, "title": "Decomposition Strategy", "description": "Divide into smaller sub-problems. Formulate a hypothesis for combining results.", "propertiesToSet": ["hypothesis"] },
                { "step": 3, "title": "Sub-problem Exploration & Branching", "description": "Spawn branches for each sub-problem.", "propertiesToSet": ["branchId", "branchFromThought"] },
                { "step": 4, "title": "Synthesis & Solution Merge", "description": "Merge branches and synthesize results.", "propertiesToSet": ["parentThoughts", "verifiedAssumptions"] }
            ]
        });

        let hypothesis_test = json!({
            "name": "Hypothesis Testing", "id": "hypothesis-test",
            "description": "Establish a testable hypothesis, identify assumptions, design verification, and evaluate.",
            "recommendedSteps": [
                { "step": 1, "title": "Hypothesis Formulation", "description": "Define a testable, falsifiable hypothesis.", "propertiesToSet": ["hypothesis", "verificationMethod"] },
                { "step": 2, "title": "Assumption Mapping", "description": "List all assumptions required for the hypothesis.", "propertiesToSet": ["assumptions"] },
                { "step": 3, "title": "Evidence Gathering & Verification", "description": "Verify assumptions using the defined method.", "propertiesToSet": ["verifiedAssumptions", "confidenceScore"] },
                { "step": 4, "title": "Synthesis / Backtracking", "description": "Confirm or refute the hypothesis. Revise if needed.", "propertiesToSet": ["isRevision", "revisesThought", "criticism"] }
            ]
        });

        let devils_advocate = json!({
            "name": "Devil's Advocate", "id": "devils-advocate",
            "description": "Identify biases, challenge assumptions, find edge cases and failure modes.",
            "recommendedSteps": [
                { "step": 1, "title": "Proposed Solution", "description": "State the current preferred solution.", "propertiesToSet": ["thought"] },
                { "step": 2, "title": "Assumption Enumeration", "description": "List every supporting assumption.", "propertiesToSet": ["assumptions"] },
                { "step": 3, "title": "Adversarial Challenge", "description": "Challenge each assumption. Describe failure modes.", "propertiesToSet": ["criticism"] },
                { "step": 4, "title": "Solution Hardening", "description": "Revise to address criticisms.", "propertiesToSet": ["isRevision", "revisesThought", "leftToBeDone"] }
            ]
        });

        match template_name {
            "divide-and-conquer" => Ok(divide_and_conquer),
            "hypothesis-test" => Ok(hypothesis_test),
            "devils-advocate" => Ok(devils_advocate),
            "all" | _ => Ok(json!({ "templates": [divide_and_conquer, hypothesis_test, devils_advocate] })),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    /// Serialize tests that touch the shared ENGINE static.
    static TEST_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    fn test_lock() -> &'static tokio::sync::Mutex<()> {
        TEST_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    async fn seed_engine(session_id: &str) {
        let engine = get_engine();
        let mut guard = engine.lock().await;
        guard.store = Box::new(MemoryThoughtStore::new());
        guard.current_session_id = String::new();
        guard.thought_history.clear();
        guard.branches.clear();

        let _ = guard.process_thought(ThoughtData {
            thought: "Initial thought".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: None,
            assumptions: Some(vec!["A1".to_string()]), verified_assumptions: None,
            confidence_score: Some(0.8), criticism: None, hypothesis: Some("H1".to_string()),
            verification_method: Some("V1".to_string()), left_to_be_done: Some(vec!["Todo1".to_string()]),
            timestamp: None, session_id: Some(session_id.to_string()),
        });
        let _ = guard.process_thought(ThoughtData {
            thought: "Branching thought".to_string(), thought_number: 2, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: Some(1),
            branch_id: Some("branch-a".to_string()), needs_more_thoughts: None, parent_thoughts: None,
            assumptions: Some(vec!["A2".to_string()]), verified_assumptions: Some(vec!["refuted: A1".to_string()]),
            confidence_score: Some(0.3), criticism: None, hypothesis: None,
            verification_method: None, left_to_be_done: None,
            timestamp: None, session_id: Some(session_id.to_string()),
        });
        let _ = guard.process_thought(ThoughtData {
            thought: "Revising first thought".to_string(), thought_number: 3, total_thoughts: 3, next_thought_needed: false,
            is_revision: Some(true), revises_thought: Some(1), branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
            confidence_score: Some(0.9), criticism: None, hypothesis: None,
            verification_method: None, left_to_be_done: None,
            timestamp: None, session_id: Some(session_id.to_string()),
        });
    }

    #[tokio::test]
    async fn test_basic_thought() {
        let _l = test_lock().lock().await;
        let engine = get_engine();
        let mut guard = engine.lock().await;

        let input = ThoughtData {
            thought: "First thought".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
            confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
            left_to_be_done: None, timestamp: None, session_id: None,
        };
        let result = guard.process_thought(input).unwrap();
        assert_eq!(result.thought_number, 1);
        assert_eq!(result.total_thoughts, 3);
        assert_eq!(result.next_thought_needed, true);
        assert_eq!(result.thought_history_length, 1);
    }

    #[tokio::test]
    async fn test_auto_adjust_total_thoughts() {
        let _l = test_lock().lock().await;
        let engine = get_engine();
        let mut guard = engine.lock().await;
        guard.store = Box::new(MemoryThoughtStore::new());
        guard.current_session_id = String::new();
        guard.thought_history.clear();
        guard.branches.clear();

        let input = ThoughtData {
            thought: "Future thought".to_string(), thought_number: 5, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
            confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
            left_to_be_done: None, timestamp: None, session_id: None,
        };
        let result = guard.process_thought(input).unwrap();
        assert_eq!(result.total_thoughts, 5);
    }

    #[tokio::test]
    async fn test_branching() {
        let _l = test_lock().lock().await;
        let engine = get_engine();
        let mut guard = engine.lock().await;
        guard.store = Box::new(MemoryThoughtStore::new());
        guard.current_session_id = String::new();
        guard.thought_history.clear();
        guard.branches.clear();

        guard.process_thought(ThoughtData {
            thought: "Main line".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
            confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
            left_to_be_done: None, timestamp: None, session_id: None,
        }).unwrap();
        let result = guard.process_thought(ThoughtData {
            thought: "Branch line".to_string(), thought_number: 2, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: Some(1),
            branch_id: Some("branch-a".to_string()), needs_more_thoughts: None, parent_thoughts: None,
            assumptions: None, verified_assumptions: None, confidence_score: None, criticism: None,
            hypothesis: None, verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
        }).unwrap();
        assert_eq!(result.branches.len(), 1);
        assert!(result.branches.contains(&"branch-a".to_string()));
        assert!(result.thought_graph_mermaid.contains("T1 --> T2"));
    }

    #[tokio::test]
    async fn test_mermaid_got_parent() {
        let _l = test_lock().lock().await;
        let engine = get_engine();
        let mut guard = engine.lock().await;
        guard.store = Box::new(MemoryThoughtStore::new());
        guard.current_session_id = String::new();
        guard.thought_history.clear();
        guard.branches.clear();

        guard.process_thought(ThoughtData {
            thought: "Idea A".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
            confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
            left_to_be_done: None, timestamp: None, session_id: None,
        }).unwrap();
        guard.process_thought(ThoughtData {
            thought: "Idea B".to_string(), thought_number: 2, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: None, assumptions: None, verified_assumptions: None,
            confidence_score: None, criticism: None, hypothesis: None, verification_method: None,
            left_to_be_done: None, timestamp: None, session_id: None,
        }).unwrap();
        let result = guard.process_thought(ThoughtData {
            thought: "Merge A and B".to_string(), thought_number: 3, total_thoughts: 3, next_thought_needed: false,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: Some(vec![1, 2]), assumptions: None,
            verified_assumptions: None, confidence_score: None, criticism: None, hypothesis: None,
            verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
        }).unwrap();
        assert!(result.thought_graph_mermaid.contains("T1 --> T3"));
        assert!(result.thought_graph_mermaid.contains("T2 --> T3"));
    }

    #[tokio::test]
    async fn test_analyze_graph_tool() {
        let _l = test_lock().lock().await;
        seed_engine("test-session").await;
        let tool = AnalyzeGraphTool;

        // low_confidence
        let res = tool.call(&json!({"query": "low_confidence", "confidenceThreshold": 0.4, "sessionId": "test-session"})).await.unwrap();
        let list: Vec<Value> = serde_json::from_value(res).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["thoughtNumber"], 2);

        // contradictions
        let res = tool.call(&json!({"query": "contradictions", "sessionId": "test-session"})).await.unwrap();
        let list: Vec<String> = serde_json::from_value(res).unwrap();
        assert_eq!(list.len(), 1);

        // summary_stats
        let res = tool.call(&json!({"query": "summary_stats", "sessionId": "test-session"})).await.unwrap();
        assert_eq!(res["totalThoughts"], 3);
        assert!(res["qualityScore"].is_number());
    }

    #[tokio::test]
    async fn test_export_mermaid() {
        let _l = test_lock().lock().await;
        seed_engine("test-session").await;
        let tool = ExportSessionTool;

        let res = tool.call(&json!({"format": "mermaid", "sessionId": "test-session"})).await.unwrap();
        assert!(res["data"].as_str().unwrap().contains("graph TD"));
    }

    #[tokio::test]
    async fn test_export_markdown() {
        let _l = test_lock().lock().await;
        seed_engine("test-session").await;
        let tool = ExportSessionTool;

        let res = tool.call(&json!({"format": "markdown", "sessionId": "test-session"})).await.unwrap();
        assert!(res["data"].as_str().unwrap().contains("# Reasoning Session History"));
    }

    #[tokio::test]
    async fn test_summarize_reasoning() {
        let _l = test_lock().lock().await;
        seed_engine("test-session").await;
        let tool = SummarizeReasoningTool;

        let res = tool.call(&json!({"sessionId": "test-session"})).await.unwrap();
        assert_eq!(res["totalThoughts"], 3);
        assert_eq!(res["totalBranches"], 1);
    }

    #[tokio::test]
    async fn test_templates_tool() {
        let tool = TemplatesTool;
        let res = tool.call(&json!({"template": "all"})).await.unwrap();
        assert!(res["templates"].is_array());
        assert_eq!(res["templates"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_cycle_detection() {
        let t1 = ThoughtData {
            thought: "T1".to_string(), thought_number: 1, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: Some(vec![2]), assumptions: None,
            verified_assumptions: None, confidence_score: Some(0.8), criticism: None,
            hypothesis: None, verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
        };
        let t2 = ThoughtData {
            thought: "T2".to_string(), thought_number: 2, total_thoughts: 3, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: Some(vec![1]), assumptions: None,
            verified_assumptions: None, confidence_score: Some(0.9), criticism: None,
            hypothesis: None, verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
        };
        let thoughts = vec![t1, t2];
        assert!(detect_cycle(&thoughts).is_some());
    }

    #[test]
    fn test_quality_contradiction() {
        let t1 = ThoughtData {
            thought: "T1".to_string(), thought_number: 1, total_thoughts: 2, next_thought_needed: true,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: None,
            assumptions: Some(vec!["Gravity is constant".to_string()]), verified_assumptions: None,
            confidence_score: Some(0.8), criticism: None, hypothesis: None,
            verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
        };
        let t2 = ThoughtData {
            thought: "T2".to_string(), thought_number: 2, total_thoughts: 2, next_thought_needed: false,
            is_revision: None, revises_thought: None, branch_from_thought: None, branch_id: None,
            needs_more_thoughts: None, parent_thoughts: None,
            assumptions: None, verified_assumptions: Some(vec!["refuted: Gravity is constant".to_string()]),
            confidence_score: Some(0.7), criticism: None, hypothesis: None,
            verification_method: None, left_to_be_done: None, timestamp: None, session_id: None,
        };
        let thoughts = vec![t1, t2];
        let report = calculate_quality("test", &thoughts);
        assert_eq!(report.contradictions_count, 1);
    }
}
