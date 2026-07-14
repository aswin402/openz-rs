use crate::tools::graph_memory::{scope_from_args, with_db};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

// ─── Tool: StoreSharedTeamMemoryTool ─────────────────────────────

pub struct StoreSharedTeamMemoryTool;

#[async_trait::async_trait]
impl Tool for StoreSharedTeamMemoryTool {
    fn name(&self) -> &str {
        "store_shared_team_memory"
    }

    fn description(&self) -> &str {
        "Store a key-value memory shared across target agent IDs."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Memory key" },
                "value": { "type": "string", "description": "Memory value" },
                "sourceAgent": { "type": "string", "description": "Agent storing this memory" },
                "targetAgents": { "type": "array", "items": { "type": "string" }, "description": "Target agent IDs" },
                "importance": { "type": "number", "default": 1.0 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["key", "value", "sourceAgent", "targetAgents"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let key = arguments["key"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'key'"))?;
        let value = arguments["value"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'value'"))?;
        let source_agent = arguments["sourceAgent"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'sourceAgent'"))?;
        let target_agents: Vec<String> = arguments["targetAgents"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .ok_or_else(|| anyhow!("Missing 'targetAgents'"))?;
        let importance = arguments
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let (uid, sid, aid) = scope_from_args(arguments);
        let timestamp = Utc::now().to_rfc3339();
        let targets_json = serde_json::to_string(&target_agents)?;

        with_db(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO shared_agent_memory (memory_key, memory_value, source_agent, target_agents, importance, timestamp, user_id, session_id, agent_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![key, value, source_agent, targets_json, importance, timestamp, uid, sid, aid],
            )?;
            Ok(())
        })?;

        Ok(json!({ "status": "Shared team memory stored successfully" }))
    }
}

// ─── Tool: RetrieveSharedTeamMemoryTool ──────────────────────────

pub struct RetrieveSharedTeamMemoryTool;

#[async_trait::async_trait]
impl Tool for RetrieveSharedTeamMemoryTool {
    fn name(&self) -> &str {
        "retrieve_shared_team_memory"
    }

    fn description(&self) -> &str {
        "Retrieve shared team memories targeted at a specific agent ID (or wildcard '*')."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "agentId": { "type": "string", "description": "The agent ID to retrieve memories for" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentIdScope": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let agent_id = arguments
            .get("agentId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let uid = arguments
            .get("userId")
            .and_then(|v| v.as_str())
            .unwrap_or("*")
            .to_string();
        let sid = arguments
            .get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or("*")
            .to_string();
        let aid_scope = arguments
            .get("agentIdScope")
            .and_then(|v| v.as_str())
            .unwrap_or("*")
            .to_string();

        let results = with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT memory_key, memory_value, source_agent, target_agents, importance, timestamp
                 FROM shared_agent_memory
                 WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*')
                   AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
                   AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')"
            )?;
            let mut rows = stmt.query(params![uid, sid, aid_scope])?;
            let mut results = Vec::new();
            while let Some(row) = rows.next()? {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                let source_agent: String = row.get(2)?;
                let targets_json: String = row.get(3)?;
                let importance: f64 = row.get(4)?;
                let timestamp: String = row.get(5)?;
                let target_agents: Vec<String> =
                    serde_json::from_str(&targets_json).unwrap_or_default();

                if agent_id.is_empty()
                    || target_agents.contains(&agent_id.to_string())
                    || target_agents.contains(&"*".to_string())
                    || source_agent == agent_id
                {
                    results.push(json!({
                        "key": key,
                        "value": value,
                        "sourceAgent": source_agent,
                        "targetAgents": target_agents,
                        "importance": importance,
                        "timestamp": timestamp,
                    }));
                }
            }
            Ok(results)
        })?;

        Ok(json!(results))
    }
}

// ─── FTS5 semantic search helper ─────────────────────────────────

pub(crate) fn query_fts5(
    conn: &Connection,
    query: &str,
    limit: usize,
    uid: &str,
    sid: &str,
    aid: &str,
) -> Result<Vec<Value>> {
    let clean_query: String = query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    let words: Vec<&str> = clean_query.split_whitespace().collect();
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let match_query = words
        .iter()
        .map(|w| format!("{}*", w))
        .collect::<Vec<_>>()
        .join(" ");

    let mut stmt = conn.prepare(
        "SELECT m.node_id, m.raw_text, m.timestamp, m.importance
         FROM semantic_fts f
         JOIN semantic_metadata m ON f.node_id = m.node_id
         WHERE semantic_fts MATCH ?1
           AND m.valid_until IS NULL
           AND (?3 IS NULL OR m.user_id = ?3 OR m.user_id = '*')
           AND (?4 IS NULL OR m.session_id = ?4 OR m.session_id = '*')
           AND (?5 IS NULL OR m.agent_id = ?5 OR m.agent_id = '*')
         ORDER BY rank
         LIMIT ?2",
    )?;
    let mut rows = stmt.query(params![match_query, limit as i64, uid, sid, aid])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(json!({
            "nodeId": row.get::<_, String>(0)?,
            "rawText": row.get::<_, String>(1)?,
            "timestamp": row.get::<_, String>(2)?,
            "importance": row.get::<_, f64>(3)?,
        }));
    }
    Ok(results)
}

// ─── Tool: SearchTextTool (FTS5) ─────────────────────────────────

pub struct SearchTextTool;

#[async_trait::async_trait]
impl Tool for SearchTextTool {
    fn name(&self) -> &str {
        "search_text"
    }

    fn description(&self) -> &str {
        "Search for semantic facts using keyword-based SQLite FTS5 index. Matches prefix terms (e.g. 'rust*' or 'memory*')."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "limit": { "type": "integer", "default": 10 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments["query"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'query'"))?;
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;
        let (uid, sid, aid) = scope_from_args(arguments);

        let results = with_db(|conn| query_fts5(conn, query, limit, &uid, &sid, &aid))?;
        Ok(json!(results))
    }
}

// ─── Tool: HybridSearchTool (FTS5 + vector similarity) ───────────

pub struct HybridSearchTool;

#[async_trait::async_trait]
impl Tool for HybridSearchTool {
    fn name(&self) -> &str {
        "hybrid_search"
    }

    fn description(&self) -> &str {
        "Search for semantic facts using hybrid search (keyword FTS5 + simple text similarity) merged via Reciprocal Rank Fusion (RRF)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "limit": { "type": "integer", "default": 10 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments["query"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'query'"))?;
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;
        let (uid, sid, aid) = scope_from_args(arguments);
        let scope = crate::tools::memory_extra::coordinator::MemoryScope::new(uid, sid, aid);
        let results = crate::tools::memory_extra::coordinator::MemoryCoordinator::default()
            .recall_raw(query, limit, &scope)
            .await?;
        Ok(json!(results))
    }
}

// Simple text similarity via word overlap
pub(crate) fn text_similarity(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    let a_words: Vec<&str> = a_lower.split_whitespace().collect();
    let b_words: Vec<&str> = b_lower.split_whitespace().collect();
    if a_words.is_empty() && b_words.is_empty() {
        return 1.0;
    }
    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    let a_set: std::collections::HashSet<&&str> = a_words.iter().collect();
    let b_set: std::collections::HashSet<&&str> = b_words.iter().collect();

    let intersection = a_set.intersection(&b_set).count();
    let union = a_set.union(&b_set).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}
