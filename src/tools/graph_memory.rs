use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};

// ─── Constants ───────────────────────────────────────────────────

// ─── Constants ───────────────────────────────────────────────────

#[allow(dead_code)]
pub(crate) const DB_FILENAME: &str = "graph_memory.db";

// ─── Shared DB static ──────────────────────────────────────────

fn db_static() -> &'static OnceLock<Mutex<Connection>> {
    static DB: OnceLock<Mutex<Connection>> = OnceLock::new();
    &DB
}

fn init_db() -> Connection {
    let path = get_db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path).unwrap_or_else(|_| {
        Connection::open_in_memory().unwrap()
    });
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
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
        ",
    )
    .ok();
    conn
}

// ─── DB helpers ─────────────────────────────────────────────────

fn get_db_path() -> std::path::PathBuf {
    #[cfg(test)]
    {
        static TEST_DB_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
        TEST_DB_PATH.get_or_init(|| {
            std::env::temp_dir().join(format!("openz_test_graph_memory_{}.db", uuid::Uuid::new_v4()))
        }).clone()
    }
    #[cfg(not(test))]
    {
        if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
            std::path::PathBuf::from(override_dir).join(DB_FILENAME)
        } else {
            crate::config::resolve_path("~/.openz/graph_memory.db")
        }
    }
}

pub(crate) fn with_db<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&Connection) -> Result<T>,
{
    let mtx = db_static().get_or_init(|| {
        Mutex::new(init_db())
    });
    let guard = mtx.lock().map_err(|e| anyhow!("Graph memory lock error: {}", e))?;
    f(&guard)
}

// ─── Scope helpers ──────────────────────────────────────────────

pub(crate) fn scope_from_args(args: &Value) -> (String, String, String) {
    let user_id = args.get("userId").and_then(|v| v.as_str()).unwrap_or("*").to_string();
    let session_id = args.get("sessionId").and_then(|v| v.as_str()).unwrap_or("*").to_string();
    let agent_id = args.get("agentId").and_then(|v| v.as_str()).unwrap_or("*").to_string();
    (user_id, session_id, agent_id)
}

// ─── Tool 1: CreateEntitiesTool ─────────────────────────────────

pub struct CreateEntitiesTool;

#[async_trait::async_trait]
impl Tool for CreateEntitiesTool {
    fn name(&self) -> &str { "create_entities" }

    fn description(&self) -> &str {
        "Create multiple new entities in the knowledge graph. Each entity must have a name and entity_type."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entities": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "The name of the entity" },
                            "entityType": { "type": "string", "description": "The type of the entity" },
                            "observations": { "type": "array", "items": { "type": "string" }, "description": "An array of observation contents" }
                        },
                        "required": ["name", "entityType"]
                    },
                    "description": "Array of entities to create"
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["entities"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let entities = arguments["entities"]
            .as_array()
            .ok_or_else(|| anyhow!("Missing 'entities' array"))?
            .clone();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let mut created = Vec::new();
        with_db(|conn| {
            for entity in &entities {
                let name = entity["name"].as_str().ok_or_else(|| anyhow!("Entity missing 'name'"))?;
                let entity_type = entity["entityType"].as_str().ok_or_else(|| anyhow!("Entity missing 'entityType'"))?;
                let obs = entity["observations"].as_array().map(|a| {
                    a.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>()
                }).unwrap_or_default();
                let obs_json = serde_json::to_string(&obs)?;

                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4)",
                    params![name, user_id, session_id, agent_id],
                    |row| row.get(0),
                )?;

                if !exists {
                    conn.execute(
                        "INSERT INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![name, entity_type, obs_json, user_id, session_id, agent_id],
                    )?;
                    created.push(entity.clone());
                }
            }
            Ok(())
        })?;

        Ok(json!({ "result": created }))
    }
}

// ─── Tool 2: CreateRelationsTool ────────────────────────────────

pub struct CreateRelationsTool;

#[async_trait::async_trait]
impl Tool for CreateRelationsTool {
    fn name(&self) -> &str { "create_relations" }

    fn description(&self) -> &str {
        "Create multiple new relations between entities in the knowledge graph. Relations should be in active voice."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "relations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "from": { "type": "string", "description": "The name of the source entity" },
                            "to": { "type": "string", "description": "The name of the target entity" },
                            "relationType": { "type": "string", "description": "The type of relation" }
                        },
                        "required": ["from", "to", "relationType"]
                    }
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["relations"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let relations = arguments["relations"]
            .as_array()
            .ok_or_else(|| anyhow!("Missing 'relations' array"))?
            .clone();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let mut created = Vec::new();
        with_db(|conn| {
            for rel in &relations {
                let from = rel["from"].as_str().ok_or_else(|| anyhow!("Relation missing 'from'"))?;
                let to = rel["to"].as_str().ok_or_else(|| anyhow!("Relation missing 'to'"))?;
                let rel_type = rel["relationType"].as_str().ok_or_else(|| anyhow!("Relation missing 'relationType'"))?;

                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM graph_edges WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6 AND valid_until IS NULL)",
                    params![from, to, rel_type, user_id, session_id, agent_id],
                    |row| row.get(0),
                )?;

                if !exists {
                    conn.execute(
                        "INSERT INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![from, to, rel_type, user_id, session_id, agent_id],
                    )?;
                    created.push(rel.clone());
                }
            }
            Ok(())
        })?;

        Ok(json!({ "result": created }))
    }
}

// ─── Tool 3: AddObservationsTool ────────────────────────────────

pub struct AddObservationsTool;

#[async_trait::async_trait]
impl Tool for AddObservationsTool {
    fn name(&self) -> &str { "add_observations" }

    fn description(&self) -> &str {
        "Add new observations to existing entities in the knowledge graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "observations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "entityName": { "type": "string" },
                            "contents": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["entityName", "contents"]
                    }
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["observations"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let observations = arguments["observations"]
            .as_array()
            .ok_or_else(|| anyhow!("Missing 'observations' array"))?
            .clone();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let mut results = Vec::new();
        with_db(|conn| {
            for obs in &observations {
                let entity_name = obs["entityName"].as_str().ok_or_else(|| anyhow!("Missing 'entityName'"))?;
                let contents: Vec<String> = obs["contents"].as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                let current_obs_str: Option<String> = conn
                    .query_row(
                        "SELECT observations FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
                        params![entity_name, user_id, session_id, agent_id],
                        |row| row.get(0),
                    )
                    .ok();

                match current_obs_str {
                    Some(obs_json) => {
                        let mut current_obs: Vec<String> = serde_json::from_str(&obs_json)?;
                        let mut added = Vec::new();
                        for content in &contents {
                            if !current_obs.contains(content) {
                                current_obs.push(content.clone());
                                added.push(content.clone());
                            }
                        }
                        let new_obs_json = serde_json::to_string(&current_obs)?;
                        conn.execute(
                            "UPDATE graph_nodes SET observations = ?1 WHERE name = ?2 AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5",
                            params![new_obs_json, entity_name, user_id, session_id, agent_id],
                        )?;
                        results.push(json!({ "entityName": entity_name, "addedObservations": added }));
                    }
                    None => {
                        return Err(anyhow!("Entity '{}' not found in scope", entity_name));
                    }
                }
            }
            Ok(())
        })?;

        Ok(json!({ "result": results }))
    }
}

// ─── Tool 4: DeleteEntitiesTool ─────────────────────────────────

pub struct DeleteEntitiesTool;

#[async_trait::async_trait]
impl Tool for DeleteEntitiesTool {
    fn name(&self) -> &str { "delete_entities" }

    fn description(&self) -> &str {
        "Delete multiple entities and their associated relations from the knowledge graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entityNames": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Array of entity names to delete"
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["entityNames"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let names: Vec<String> = arguments["entityNames"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .ok_or_else(|| anyhow!("Missing 'entityNames' array"))?;
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            for name in &names {
                conn.execute(
                    "DELETE FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
                    params![name, user_id, session_id, agent_id],
                )?;
                conn.execute(
                    "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE (from_name = ?1 OR to_name = ?1) AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4 AND valid_until IS NULL",
                    params![name, user_id, session_id, agent_id],
                )?;
            }
            Ok(())
        })?;

        Ok(json!({ "status": "deleted" }))
    }
}

// ─── Tool 5: DeleteObservationsTool ─────────────────────────────

pub struct DeleteObservationsTool;

#[async_trait::async_trait]
impl Tool for DeleteObservationsTool {
    fn name(&self) -> &str { "delete_observations" }

    fn description(&self) -> &str {
        "Delete specific observations from entities in the knowledge graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "deletions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "entityName": { "type": "string" },
                            "observations": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["entityName", "observations"]
                    }
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["deletions"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let deletions = arguments["deletions"]
            .as_array()
            .ok_or_else(|| anyhow!("Missing 'deletions' array"))?
            .clone();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            for del in &deletions {
                let entity_name = del["entityName"].as_str().ok_or_else(|| anyhow!("Missing 'entityName'"))?;
                let to_remove: Vec<String> = del["observations"].as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                let current_obs_str: Option<String> = conn
                    .query_row(
                        "SELECT observations FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
                        params![entity_name, user_id, session_id, agent_id],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(obs_json) = current_obs_str {
                    let current_obs: Vec<String> = serde_json::from_str(&obs_json)?;
                    let filtered: Vec<String> = current_obs.into_iter()
                        .filter(|o| !to_remove.contains(o))
                        .collect();
                    let new_obs_json = serde_json::to_string(&filtered)?;
                    conn.execute(
                        "UPDATE graph_nodes SET observations = ?1 WHERE name = ?2 AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5",
                        params![new_obs_json, entity_name, user_id, session_id, agent_id],
                    )?;
                }
            }
            Ok(())
        })?;

        Ok(json!({ "status": "observations deleted" }))
    }
}

// ─── Tool 6: DeleteRelationsTool ────────────────────────────────

pub struct DeleteRelationsTool;

#[async_trait::async_trait]
impl Tool for DeleteRelationsTool {
    fn name(&self) -> &str { "delete_relations" }

    fn description(&self) -> &str {
        "Delete multiple relations from the knowledge graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "relations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "from": { "type": "string" },
                            "to": { "type": "string" },
                            "relationType": { "type": "string" }
                        },
                        "required": ["from", "to", "relationType"]
                    }
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["relations"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let relations = arguments["relations"]
            .as_array()
            .ok_or_else(|| anyhow!("Missing 'relations' array"))?
            .clone();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            for rel in &relations {
                let from = rel["from"].as_str().ok_or_else(|| anyhow!("Missing 'from'"))?;
                let to = rel["to"].as_str().ok_or_else(|| anyhow!("Missing 'to'"))?;
                let rel_type = rel["relationType"].as_str().ok_or_else(|| anyhow!("Missing 'relationType'"))?;
                conn.execute(
                    "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6 AND valid_until IS NULL",
                    params![from, to, rel_type, user_id, session_id, agent_id],
                )?;
            }
            Ok(())
        })?;

        Ok(json!({ "status": "relations deleted" }))
    }
}

// ─── Tool 7: ReadGraphTool ──────────────────────────────────────

pub struct ReadGraphTool;

#[async_trait::async_trait]
impl Tool for ReadGraphTool {
    fn name(&self) -> &str { "read_graph" }

    fn description(&self) -> &str {
        "Read the entire knowledge graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            let mut stmt_nodes = conn.prepare(
                "SELECT name, entity_type, observations FROM graph_nodes WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')"
            )?;
            let mut node_rows = stmt_nodes.query(params![user_id, session_id, agent_id])?;
            let mut entities = Vec::new();
            while let Some(row) = node_rows.next()? {
                let name: String = row.get(0)?;
                let entity_type: String = row.get(1)?;
                let obs_json: String = row.get(2)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                entities.push(json!({ "name": name, "entityType": entity_type, "observations": observations }));
            }

            let mut stmt_edges = conn.prepare(
                "SELECT from_name, to_name, relation_type FROM graph_edges WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*') AND valid_until IS NULL"
            )?;
            let mut edge_rows = stmt_edges.query(params![user_id, session_id, agent_id])?;
            let mut relations = Vec::new();
            while let Some(row) = edge_rows.next()? {
                relations.push(json!({ "from": row.get::<_, String>(0)?, "to": row.get::<_, String>(1)?, "relationType": row.get::<_, String>(2)? }));
            }

            Ok(json!({ "entities": entities, "relations": relations }))
        })
    }
}

// ─── Tool 8: SearchNodesTool ────────────────────────────────────

pub struct SearchNodesTool;

#[async_trait::async_trait]
impl Tool for SearchNodesTool {
    fn name(&self) -> &str { "search_nodes" }

    fn description(&self) -> &str {
        "Search for nodes in the knowledge graph based on a query."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query to match against entity names, types, and observation content" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments["query"].as_str().ok_or_else(|| anyhow!("Missing 'query'"))?;
        let (user_id, session_id, agent_id) = scope_from_args(arguments);
        let query_pattern = format!("%{}%", query.to_lowercase());

        with_db(|conn| {
            let mut stmt_nodes = conn.prepare(
                "SELECT name, entity_type, observations FROM graph_nodes WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1) AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
            )?;
            let mut node_rows = stmt_nodes.query(params![query_pattern, user_id, session_id, agent_id])?;
            let mut entities = Vec::new();
            while let Some(row) = node_rows.next()? {
                let name: String = row.get(0)?;
                let entity_type: String = row.get(1)?;
                let obs_json: String = row.get(2)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                entities.push(json!({ "name": name, "entityType": entity_type, "observations": observations }));
            }

            let mut stmt_edges = conn.prepare(
                "SELECT DISTINCT from_name, to_name, relation_type FROM graph_edges WHERE (from_name IN (SELECT name FROM graph_nodes WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1)) OR to_name IN (SELECT name FROM graph_nodes WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1))) AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*') AND valid_until IS NULL"
            )?;
            let mut edge_rows = stmt_edges.query(params![query_pattern, user_id, session_id, agent_id])?;
            let mut relations = Vec::new();
            while let Some(row) = edge_rows.next()? {
                relations.push(json!({ "from": row.get::<_, String>(0)?, "to": row.get::<_, String>(1)?, "relationType": row.get::<_, String>(2)? }));
            }

            Ok(json!({ "entities": entities, "relations": relations }))
        })
    }
}

// ─── Tool 9: OpenNodesTool ──────────────────────────────────────

pub struct OpenNodesTool;

#[async_trait::async_trait]
impl Tool for OpenNodesTool {
    fn name(&self) -> &str { "open_nodes" }

    fn description(&self) -> &str {
        "Open specific nodes in the knowledge graph by their names."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "names": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "An array of entity names to retrieve"
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["names"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let names: Vec<String> = arguments["names"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .ok_or_else(|| anyhow!("Missing 'names' array"))?;
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            let mut entities = Vec::new();
            for name in &names {
                let mut stmt = conn.prepare(
                    "SELECT entity_type, observations FROM graph_nodes WHERE name = ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
                )?;
                let mut rows = stmt.query(params![name, user_id, session_id, agent_id])?;
                if let Some(row) = rows.next()? {
                    let entity_type: String = row.get(0)?;
                    let obs_json: String = row.get(1)?;
                    let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                    entities.push(json!({ "name": name, "entityType": entity_type, "observations": observations }));
                }
            }

            let mut relations = Vec::new();
            if !names.is_empty() {
                let placeholders: Vec<String> = (0..names.len()).map(|i| format!("?{}", i + 5)).collect();
                let placeholders_str = placeholders.join(", ");
                let sql = format!(
                    "SELECT DISTINCT from_name, to_name, relation_type FROM graph_edges WHERE (from_name IN ({0}) OR to_name IN ({0})) AND (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*') AND valid_until IS NULL",
                    placeholders_str
                );
                let mut stmt_edges = conn.prepare(&sql)?;
                let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
                    Box::new(user_id),
                    Box::new(session_id),
                    Box::new(agent_id),
                ];
                for name in &names {
                    param_values.push(Box::new(name.clone()));
                }
                for name in &names {
                    param_values.push(Box::new(name.clone()));
                }
                let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
                let mut edge_rows = stmt_edges.query(params_refs.as_slice())?;
                while let Some(row) = edge_rows.next()? {
                    relations.push(json!({ "from": row.get::<_, String>(0)?, "to": row.get::<_, String>(1)?, "relationType": row.get::<_, String>(2)? }));
                }
            }

            Ok(json!({ "entities": entities, "relations": relations }))
        })
    }
}

// ─── Branching tools ────────────────────────────────────────────

static BRANCH_MUTEX: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn get_active_branch() -> &'static Mutex<Option<String>> {
    BRANCH_MUTEX.get_or_init(|| Mutex::new(None))
}

fn branch_db_path(branch_id: &str) -> std::path::PathBuf {
    let base = get_db_path();
    let base_str = base.to_string_lossy().to_string();
    std::path::PathBuf::from(format!("{}.branch_{}", base_str, branch_id))
}

// ─── Tool 10: CreateDatabaseBranchTool ──────────────────────────

pub struct CreateDatabaseBranchTool;

#[async_trait::async_trait]
impl Tool for CreateDatabaseBranchTool {
    fn name(&self) -> &str { "create_database_branch" }

    fn description(&self) -> &str {
        "Create an isolated database branch for subagent/task execution. Branch ID must be unique."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "branchId": { "type": "string", "description": "Unique identifier for the branch" }
            },
            "required": ["branchId"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        if arguments["branchId"].as_str().is_none() {
            return Err(anyhow!("Missing 'branchId'"));
        }
        let branch_id = arguments["branchId"].as_str().unwrap();

        let active = get_active_branch().lock().map_err(|e| anyhow!("Branch lock error: {}", e))?;
        if active.is_some() {
            return Err(anyhow!("A database branch is already active. Commit or rollback first."));
        }

        let src = get_db_path();
        let dst = branch_db_path(branch_id);

        // Flush WAL before copying
        if let Ok(mtx) = db_static().get_or_init(|| Mutex::new(Connection::open_in_memory().unwrap())).lock() {
            let _ = mtx.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        drop(active);
        std::thread::sleep(std::time::Duration::from_millis(10));

        // If src doesn't exist yet (e.g. in tests), create a fresh DB with schema
        if !src.exists() {
            let fresh = init_db();
            drop(fresh);
        }

        std::fs::copy(&src, &dst)?;

        let mut active = get_active_branch().lock().map_err(|e| anyhow!("Branch lock error: {}", e))?;
        // Reset the static DB connection to point to branch file
        let branch_conn = Connection::open(&dst)?;
        branch_conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
             CREATE TABLE IF NOT EXISTS graph_nodes (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 entity_type TEXT NOT NULL,
                 observations TEXT DEFAULT '[]',
                 user_id TEXT NOT NULL DEFAULT '*',
                 session_id TEXT NOT NULL DEFAULT '*',
                 agent_id TEXT NOT NULL DEFAULT '*',
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 UNIQUE(name, user_id, session_id, agent_id)
             );
             CREATE TABLE IF NOT EXISTS graph_edges (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 from_name TEXT NOT NULL,
                 to_name TEXT NOT NULL,
                 relation_type TEXT NOT NULL,
                 user_id TEXT NOT NULL DEFAULT '*',
                 session_id TEXT NOT NULL DEFAULT '*',
                 agent_id TEXT NOT NULL DEFAULT '*',
                 valid_until TEXT DEFAULT NULL,
                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE INDEX IF NOT EXISTS idx_graph_nodes_scope ON graph_nodes(user_id, session_id, agent_id);
             CREATE INDEX IF NOT EXISTS idx_graph_edges_scope ON graph_edges(user_id, session_id, agent_id);
             CREATE INDEX IF NOT EXISTS idx_graph_edges_active ON graph_edges(valid_until);"
        ).ok();
        *db_static().get_or_init(|| Mutex::new(Connection::open_in_memory().unwrap()))
            .lock().map_err(|e| anyhow!("Lock error: {}", e))? = branch_conn;

        *active = Some(branch_id.to_string());
        Ok(json!({ "status": format!("Created branch: {}", branch_id) }))
    }
}

// ─── Tool 11: CommitDatabaseBranchTool ──────────────────────────

pub struct CommitDatabaseBranchTool;

#[async_trait::async_trait]
impl Tool for CommitDatabaseBranchTool {
    fn name(&self) -> &str { "commit_database_branch" }

    fn description(&self) -> &str {
        "Commit changes from the active database branch to the main database and delete the branch."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let mut active = get_active_branch().lock().map_err(|e| anyhow!("Branch lock error: {}", e))?;
        let branch_id = active.as_ref()
            .ok_or_else(|| anyhow!("No active branch to commit."))?
            .clone();

        let branch_path = branch_db_path(&branch_id);
        let main_path = get_db_path();

        // Switch DB to in-memory to release file locks
        // Flush WAL first
        if let Ok(mtx) = db_static().get_or_init(|| Mutex::new(Connection::open_in_memory().unwrap())).lock() {
            let _ = mtx.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        let mem_conn = Connection::open_in_memory()?;
        *db_static().get_or_init(|| Mutex::new(Connection::open_in_memory().unwrap()))
            .lock().map_err(|e| anyhow!("Lock error: {}", e))? = mem_conn;

        // Copy branch file over main db
        std::fs::copy(&branch_path, &main_path)?;
        std::fs::remove_file(&branch_path)?;
        // Clean up WAL/SHM files
        let _ = std::fs::remove_file(&main_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(&main_path.with_extension("db-shm"));

        // Restore connection to updated main
        let main_conn = Connection::open(&main_path)?;
        main_conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;").ok();
        *db_static().get_or_init(|| Mutex::new(Connection::open_in_memory().unwrap()))
            .lock().map_err(|e| anyhow!("Lock error: {}", e))? = main_conn;

        *active = None;
        Ok(json!({ "status": format!("Committed branch: {}", branch_id) }))
    }
}

// ─── Tool 12: RollbackDatabaseBranchTool ────────────────────────

pub struct RollbackDatabaseBranchTool;

#[async_trait::async_trait]
impl Tool for RollbackDatabaseBranchTool {
    fn name(&self) -> &str { "rollback_database_branch" }

    fn description(&self) -> &str {
        "Roll back changes from the active database branch, restoring the main database state and deleting the branch."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let mut active = get_active_branch().lock().map_err(|e| anyhow!("Branch lock error: {}", e))?;
        let branch_id = active.as_ref()
            .ok_or_else(|| anyhow!("No active branch to rollback."))?
            .clone();

        let branch_path = branch_db_path(&branch_id);

        // Restore connection back to main database
        let main_path = get_db_path();
        let main_conn = Connection::open(&main_path)?;
        main_conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;").ok();
        *db_static().get_or_init(|| Mutex::new(Connection::open_in_memory().unwrap()))
            .lock().map_err(|e| anyhow!("Lock error: {}", e))? = main_conn;

        // Remove branch file
        if branch_path.exists() {
            std::fs::remove_file(&branch_path)?;
        }

        *active = None;
        Ok(json!({ "status": format!("Rolled back branch: {}", branch_id) }))
    }
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) fn test_lock() -> &'static tokio::sync::Mutex<()> {
    static TEST_MUTEX: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    TEST_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn unique_scope(prefix: &str) -> String {
        format!("{}_{}", prefix, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())
    }

    #[tokio::test]
    async fn test_create_and_read_entities() {
        let _l = test_lock().lock().await;
        let scope_id = unique_scope("test");

        let tool_c = CreateEntitiesTool;
        let res = tool_c.call(&json!({
            "entities": [
                { "name": "Alice", "entityType": "Person", "observations": ["Loves coffee"] },
                { "name": "Bob", "entityType": "Person", "observations": ["Loves tea"] }
            ],
            "sessionId": scope_id
        })).await.unwrap();
        assert!(res["result"].is_array());

        let tool_r = ReadGraphTool;
        let res2 = tool_r.call(&json!({ "sessionId": scope_id })).await.unwrap();
        let entities = res2["entities"].as_array().unwrap();
        assert!(entities.len() >= 2);
    }

    #[tokio::test]
    async fn test_create_relations() {
        let _l = test_lock().lock().await;
        let scope_id = unique_scope("test_rel");

        let tool_c = CreateEntitiesTool;
        tool_c.call(&json!({
            "entities": [
                { "name": "X", "entityType": "Item", "observations": [] },
                { "name": "Y", "entityType": "Item", "observations": [] }
            ],
            "sessionId": scope_id
        })).await.unwrap();

        let tool_r = CreateRelationsTool;
        let res = tool_r.call(&json!({
            "relations": [{ "from": "X", "to": "Y", "relationType": "connects_to" }],
            "sessionId": scope_id
        })).await.unwrap();
        assert!(res["result"].is_array());
    }

    #[tokio::test]
    async fn test_add_observations() {
        let _l = test_lock().lock().await;
        let scope_id = unique_scope("test_obs");

        CreateEntitiesTool.call(&json!({
            "entities": [{ "name": "Node1", "entityType": "Test", "observations": [] }],
            "sessionId": scope_id
        })).await.unwrap();

        let tool = AddObservationsTool;
        let res = tool.call(&json!({
            "observations": [{ "entityName": "Node1", "contents": ["New observation"] }],
            "sessionId": scope_id
        })).await.unwrap();
        let added = res["result"][0]["addedObservations"].as_array().unwrap();
        assert_eq!(added.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_entities() {
        let _l = test_lock().lock().await;
        let scope_id = unique_scope("test_del");

        CreateEntitiesTool.call(&json!({
            "entities": [{ "name": "ToDelete", "entityType": "Temp", "observations": [] }],
            "sessionId": scope_id
        })).await.unwrap();

        DeleteEntitiesTool.call(&json!({
            "entityNames": ["ToDelete"],
            "sessionId": scope_id
        })).await.unwrap();

        let res = ReadGraphTool.call(&json!({ "sessionId": scope_id })).await.unwrap();
        let found = res["entities"].as_array().unwrap().iter().any(|e| e["name"] == "ToDelete");
        assert!(!found);
    }

    #[tokio::test]
    async fn test_search_nodes() {
        let _l = test_lock().lock().await;
        let scope_id = unique_scope("test_srch");

        CreateEntitiesTool.call(&json!({
            "entities": [{ "name": "UniqueSearchTarget", "entityType": "Searchable", "observations": ["special keyword"] }],
            "sessionId": scope_id
        })).await.unwrap();

        let res = SearchNodesTool.call(&json!({ "query": "UniqueSearch", "sessionId": scope_id })).await.unwrap();
        assert!(!res["entities"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_delete_relations() {
        let _l = test_lock().lock().await;
        let scope_id = unique_scope("test_delrel");

        CreateEntitiesTool.call(&json!({
            "entities": [
                { "name": "A", "entityType": "Node", "observations": [] },
                { "name": "B", "entityType": "Node", "observations": [] }
            ],
            "sessionId": scope_id
        })).await.unwrap();

        CreateRelationsTool.call(&json!({
            "relations": [{ "from": "A", "to": "B", "relationType": "connected" }],
            "sessionId": scope_id
        })).await.unwrap();

        DeleteRelationsTool.call(&json!({
            "relations": [{ "from": "A", "to": "B", "relationType": "connected" }],
            "sessionId": scope_id
        })).await.unwrap();

        let res = ReadGraphTool.call(&json!({ "sessionId": scope_id })).await.unwrap();
        assert!(res["relations"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_open_nodes() {
        let _l = test_lock().lock().await;
        let scope_id = unique_scope("test_open");

        CreateEntitiesTool.call(&json!({
            "entities": [{ "name": "OpenMe", "entityType": "Test", "observations": ["visible"] }],
            "sessionId": scope_id
        })).await.unwrap();

        let res = OpenNodesTool.call(&json!({ "names": ["OpenMe"], "sessionId": scope_id })).await.unwrap();
        assert_eq!(res["entities"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_delete_observations() {
        let _l = test_lock().lock().await;
        let scope_id = unique_scope("test_delobs");

        CreateEntitiesTool.call(&json!({
            "entities": [{ "name": "ObsTarget", "entityType": "Test", "observations": ["keep me", "delete me"] }],
            "sessionId": scope_id
        })).await.unwrap();

        DeleteObservationsTool.call(&json!({
            "deletions": [{ "entityName": "ObsTarget", "observations": ["delete me"] }],
            "sessionId": scope_id
        })).await.unwrap();

        let res = OpenNodesTool.call(&json!({ "names": ["ObsTarget"], "sessionId": scope_id })).await.unwrap();
        let obs = res["entities"][0]["observations"].as_array().unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0], "keep me");
    }

    #[tokio::test]
    async fn test_branch_commit_rollback() {
        let _l = test_lock().lock().await;
        let scope_id = unique_scope("test_branch");
        let branch_id = format!("br_{}", &uuid::Uuid::new_v4().to_string()[..8]);

        // Create entity in main
        CreateEntitiesTool.call(&json!({
            "entities": [{ "name": "MainEntity", "entityType": "Test", "observations": [] }],
            "sessionId": scope_id
        })).await.unwrap();

        // Create branch
        CreateDatabaseBranchTool.call(&json!({ "branchId": branch_id })).await.unwrap();

        // Add entity in branch
        CreateEntitiesTool.call(&json!({
            "entities": [{ "name": "BranchEntity", "entityType": "Test", "observations": [] }],
            "sessionId": scope_id
        })).await.unwrap();

        // Verify branch has both
        let res = ReadGraphTool.call(&json!({ "sessionId": scope_id })).await.unwrap();
        assert_eq!(res["entities"].as_array().unwrap().len(), 2);

        // Rollback
        RollbackDatabaseBranchTool.call(&json!({})).await.unwrap();

        // Verify only main entity remains
        let res2 = ReadGraphTool.call(&json!({ "sessionId": scope_id })).await.unwrap();
        assert_eq!(res2["entities"].as_array().unwrap().len(), 1);
    }
}
