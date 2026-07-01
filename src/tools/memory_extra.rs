use crate::tools::graph_memory::{scope_from_args, with_db};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use chrono::Utc;
use regex::Regex;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

// ─── Working Memory (in-memory HashMap with TTL) ─────────────────

fn working_memory_static() -> &'static Mutex<HashMap<String, WorkingEntry>> {
    static WM: OnceLock<Mutex<HashMap<String, WorkingEntry>>> = OnceLock::new();
    WM.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone)]
struct WorkingEntry {
    value: String,
    created_at: std::time::Instant,
    ttl_seconds: u64,
    access_count: usize,
}

fn working_scoped_key(key: &str, user_id: &str, session_id: &str, agent_id: &str) -> String {
    format!("{}:{}:{}:{}", user_id, session_id, agent_id, key)
}

// ─── Tool: SetWorkingMemoryTool ──────────────────────────────────

pub struct SetWorkingMemoryTool;

#[async_trait::async_trait]
impl Tool for SetWorkingMemoryTool {
    fn name(&self) -> &str { "set_working_memory" }

    fn description(&self) -> &str {
        "Set an ephemeral key-value pair in working memory, with an optional TTL (seconds)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The key to set" },
                "value": { "type": "string", "description": "The value to store" },
                "ttl": { "type": "integer", "description": "Time-to-live in seconds (default 300)", "minimum": 1 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["key", "value"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let key = arguments["key"].as_str().ok_or_else(|| anyhow!("Missing 'key'"))?;
        let value = arguments["value"].as_str().ok_or_else(|| anyhow!("Missing 'value'"))?;
        let ttl = arguments.get("ttl").and_then(|v| v.as_u64()).unwrap_or(300);
        let (uid, sid, aid) = scope_from_args(arguments);
        let scoped = working_scoped_key(key, &uid, &sid, &aid);

        let mut map = working_memory_static().lock().map_err(|e| anyhow!("Working memory lock error: {}", e))?;
        map.insert(scoped, WorkingEntry {
            value: value.to_string(),
            created_at: std::time::Instant::now(),
            ttl_seconds: ttl,
            access_count: 0,
        });

        Ok(json!({ "status": format!("Set working memory key '{}' (TTL: {}s)", key, ttl) }))
    }
}

// ─── Tool: GetWorkingMemoryTool ──────────────────────────────────

pub struct GetWorkingMemoryTool;

#[async_trait::async_trait]
impl Tool for GetWorkingMemoryTool {
    fn name(&self) -> &str { "get_working_memory" }

    fn description(&self) -> &str {
        "Retrieve an ephemeral value from working memory. Checks and handles TTL expiration."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The key to retrieve" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["key"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let key = arguments["key"].as_str().ok_or_else(|| anyhow!("Missing 'key'"))?;
        let (uid, sid, aid) = scope_from_args(arguments);
        let scoped = working_scoped_key(key, &uid, &sid, &aid);

        let mut map = working_memory_static().lock().map_err(|e| anyhow!("Working memory lock error: {}", e))?;

        if let Some(entry) = map.get(&scoped) {
            if entry.created_at.elapsed().as_secs() >= entry.ttl_seconds {
                map.remove(&scoped);
                return Ok(json!({ "status": format!("Key '{}' has expired", key) }));
            }
        } else {
            return Ok(json!({ "status": format!("Key '{}' not found", key) }));
        }

        if let Some(entry) = map.get_mut(&scoped) {
            entry.access_count += 1;
            return Ok(json!({ "key": key, "value": entry.value.clone() }));
        }

        Ok(json!({ "status": format!("Key '{}' not found", key) }))
    }
}

// ─── Tool: EvictExpiredWorkingMemoryTool ─────────────────────────

pub struct EvictExpiredWorkingMemoryTool;

#[async_trait::async_trait]
impl Tool for EvictExpiredWorkingMemoryTool {
    fn name(&self) -> &str { "evict_expired_working_memory" }

    fn description(&self) -> &str {
        "Evict expired keys from working memory, promoting important ones (accessed >= 3 times) to semantic memory."
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
        let (uid, sid, aid) = scope_from_args(arguments);
        let mut map = working_memory_static().lock().map_err(|e| anyhow!("Working memory lock error: {}", e))?;
        let mut evicted = 0usize;

        let to_remove: Vec<String> = map.iter()
            .filter(|(_, e)| e.created_at.elapsed().as_secs() >= e.ttl_seconds)
            .map(|(k, _)| k.clone())
            .collect();

        for scoped in &to_remove {
            if let Some(entry) = map.remove(scoped) {
                evicted += 1;
                if entry.access_count >= 3 {
                    // Promote to semantic memory
                    let fact_id = format!("working-promoted-{}", uuid::Uuid::new_v4());
                    let raw_text = format!("Ephemeral working memory was promoted: {}", entry.value);
                    let _ = store_semantic_fact(&fact_id, &raw_text, 0.8, &uid, &sid, &aid);
                }
            }
        }

        Ok(json!({ "status": format!("Evicted {} expired entries", evicted) }))
    }
}

// ─── Tool: PromoteWorkingMemoryTool ──────────────────────────────

pub struct PromoteWorkingMemoryTool;

#[async_trait::async_trait]
impl Tool for PromoteWorkingMemoryTool {
    fn name(&self) -> &str { "promote_working_memory" }

    fn description(&self) -> &str {
        "Manually promote a working memory entry to long-term semantic memory and remove it from working memory."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The key to promote" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["key"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let key = arguments["key"].as_str().ok_or_else(|| anyhow!("Missing 'key'"))?;
        let (uid, sid, aid) = scope_from_args(arguments);
        let scoped = working_scoped_key(key, &uid, &sid, &aid);

        let mut map = working_memory_static().lock().map_err(|e| anyhow!("Working memory lock error: {}", e))?;
        if let Some(entry) = map.remove(&scoped) {
            let fact_id = format!("working-promoted-{}", uuid::Uuid::new_v4());
            let raw_text = format!("Ephemeral working memory under key '{}' was promoted: {}", key, entry.value);
            store_semantic_fact(&fact_id, &raw_text, 0.8, &uid, &sid, &aid)?;
            Ok(json!({ "status": format!("Promoted '{}' to semantic memory", key) }))
        } else {
            Ok(json!({ "status": format!("Key '{}' not found", key) }))
        }
    }
}

// ─── Semantic fact helper ────────────────────────────────────────

fn store_semantic_fact(node_id: &str, text: &str, importance: f64, user_id: &str, session_id: &str, agent_id: &str) -> Result<()> {
    let timestamp = Utc::now().to_rfc3339();
    with_db(|conn| {
        conn.execute(
            "INSERT INTO semantic_metadata (node_id, raw_text, timestamp, importance, user_id, session_id, agent_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![node_id, text, timestamp, importance, user_id, session_id, agent_id],
        )?;
        // Also insert into FTS5 index
        let _ = conn.execute(
            "INSERT INTO semantic_fts (node_id, raw_text) VALUES (?1, ?2)",
            params![node_id, text],
        );
        Ok(())
    })
}

// ─── Tool: LogExecutionEpisodeTool ───────────────────────────────

pub struct LogExecutionEpisodeTool;

#[async_trait::async_trait]
impl Tool for LogExecutionEpisodeTool {
    fn name(&self) -> &str { "log_execution_episode" }

    fn description(&self) -> &str {
        "Log an execution episode: details tasks attempted, execution logs, status and reflections."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Optional episode ID (auto-generated if omitted)" },
                "taskDescription": { "type": "string", "description": "Description of the task" },
                "executionStatus": { "type": "string", "description": "Status of execution" },
                "stepsTaken": { "type": "string", "description": "Steps taken during execution" },
                "errorMessage": { "type": "string" },
                "reflection": { "type": "string" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["taskDescription", "executionStatus", "stepsTaken"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or_else(|| {
            Box::leak(uuid::Uuid::new_v4().to_string().into_boxed_str())
        }).to_string();
        let task_description = arguments["taskDescription"].as_str().ok_or_else(|| anyhow!("Missing 'taskDescription'"))?;
        let execution_status = arguments["executionStatus"].as_str().ok_or_else(|| anyhow!("Missing 'executionStatus'"))?;
        let steps_taken = arguments["stepsTaken"].as_str().ok_or_else(|| anyhow!("Missing 'stepsTaken'"))?;
        let error_message = arguments.get("errorMessage").and_then(|v| v.as_str());
        let reflection = arguments.get("reflection").and_then(|v| v.as_str());
        let (uid, sid, aid) = scope_from_args(arguments);
        let created_at = Utc::now().to_rfc3339();

        with_db(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO episodic_logs (id, task_description, execution_status, steps_taken, error_message, reflection, created_at, user_id, session_id, agent_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![id, task_description, execution_status, steps_taken, error_message, reflection, created_at, uid, sid, aid],
            )?;
            Ok(())
        })?;

        Ok(json!({ "status": "Episode logged successfully" }))
    }
}

// ─── Tool: LogReflectionTool ─────────────────────────────────────

pub struct LogReflectionTool;

#[async_trait::async_trait]
impl Tool for LogReflectionTool {
    fn name(&self) -> &str { "log_reflection" }

    fn description(&self) -> &str {
        "Store a reflection memory summarizing what worked, what failed, why, and error analysis."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "taskDescription": { "type": "string" },
                "status": { "type": "string", "description": "Success or Failed" },
                "attemptNumber": { "type": "integer", "default": 1 },
                "stepsTaken": { "type": "string" },
                "errorEncountered": { "type": "string" },
                "rootCause": { "type": "string" },
                "solutionApplied": { "type": "string" },
                "reflection": { "type": "string" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["taskDescription", "status", "stepsTaken", "reflection"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let task_description = arguments["taskDescription"].as_str().ok_or_else(|| anyhow!("Missing 'taskDescription'"))?;
        let status = arguments["status"].as_str().ok_or_else(|| anyhow!("Missing 'status'"))?;
        let attempt_number = arguments.get("attemptNumber").and_then(|v| v.as_i64()).unwrap_or(1);
        let steps_taken = arguments["stepsTaken"].as_str().ok_or_else(|| anyhow!("Missing 'stepsTaken'"))?;
        let error_encountered = arguments.get("errorEncountered").and_then(|v| v.as_str());
        let root_cause = arguments.get("rootCause").and_then(|v| v.as_str());
        let solution_applied = arguments.get("solutionApplied").and_then(|v| v.as_str());
        let reflection = arguments["reflection"].as_str().ok_or_else(|| anyhow!("Missing 'reflection'"))?;
        let (uid, sid, aid) = scope_from_args(arguments);
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();

        with_db(|conn| {
            conn.execute(
                "INSERT INTO reflection_memory (id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at, user_id, session_id, agent_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at, uid, sid, aid],
            )?;
            Ok(())
        })?;

        Ok(json!({ "status": "Reflection logged successfully" }))
    }
}

// ─── Tool: RetrieveEpisodicReflectionsTool ───────────────────────

pub struct RetrieveEpisodicReflectionsTool;

#[async_trait::async_trait]
impl Tool for RetrieveEpisodicReflectionsTool {
    fn name(&self) -> &str { "retrieve_episodic_reflections" }

    fn description(&self) -> &str {
        "Retrieve reflections to guide current attempts based on query parameters."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query to filter reflections" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let (uid, sid, aid) = scope_from_args(arguments);

        let results = with_db(|conn| {
            let sql = if query.is_empty() {
                "SELECT id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at
                 FROM reflection_memory
                 WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*')
                   AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
                   AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')
                 ORDER BY created_at DESC"
            } else {
                "SELECT id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at
                 FROM reflection_memory
                 WHERE (task_description LIKE ?1 OR reflection LIKE ?1 OR root_cause LIKE ?1)
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')
                 ORDER BY created_at DESC"
            };
            let mut stmt = conn.prepare(sql)?;
            let mut rows = if query.is_empty() {
                stmt.query(params![uid, sid, aid])?
            } else {
                let pattern = format!("%{}%", query);
                stmt.query(params![pattern, uid, sid, aid])?
            };
            let mut results = Vec::new();
            while let Some(row) = rows.next()? {
                results.push(json!({
                    "id": row.get::<_, String>(0)?,
                    "taskDescription": row.get::<_, String>(1)?,
                    "status": row.get::<_, String>(2)?,
                    "attemptNumber": row.get::<_, i64>(3)?,
                    "stepsTaken": row.get::<_, String>(4)?,
                    "errorEncountered": row.get::<_, Option<String>>(5)?,
                    "rootCause": row.get::<_, Option<String>>(6)?,
                    "solutionApplied": row.get::<_, Option<String>>(7)?,
                    "reflection": row.get::<_, String>(8)?,
                    "createdAt": row.get::<_, String>(9)?,
                }));
            }
            Ok(results)
        })?;

        Ok(json!(results))
    }
}

// ─── Tool: RecordToolPerformanceTool ─────────────────────────────

pub struct RecordToolPerformanceTool;

#[async_trait::async_trait]
impl Tool for RecordToolPerformanceTool {
    fn name(&self) -> &str { "record_tool_performance" }

    fn description(&self) -> &str {
        "Record the success rates and latencies of an LLM or specific tool usage."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "toolName": { "type": "string", "description": "Name of the tool" },
                "modelName": { "type": "string", "description": "Name of the model" },
                "taskType": { "type": "string", "description": "Type of task" },
                "successCount": { "type": "integer", "description": "Number of successful calls" },
                "failureCount": { "type": "integer", "description": "Number of failed calls" },
                "averageLatency": { "type": "number", "description": "Average latency in seconds" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["toolName", "modelName", "taskType", "successCount", "failureCount", "averageLatency"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let tool_name = arguments["toolName"].as_str().ok_or_else(|| anyhow!("Missing 'toolName'"))?;
        let model_name = arguments["modelName"].as_str().ok_or_else(|| anyhow!("Missing 'modelName'"))?;
        let task_type = arguments["taskType"].as_str().ok_or_else(|| anyhow!("Missing 'taskType'"))?;
        let success_count: i64 = arguments["successCount"].as_i64().ok_or_else(|| anyhow!("Missing 'successCount'"))?;
        let failure_count: i64 = arguments["failureCount"].as_i64().ok_or_else(|| anyhow!("Missing 'failureCount'"))?;
        let average_latency: f64 = arguments["averageLatency"].as_f64().ok_or_else(|| anyhow!("Missing 'averageLatency'"))?;
        let (uid, sid, aid) = scope_from_args(arguments);
        let last_used = Utc::now().to_rfc3339();

        with_db(|conn| {
            // Check if record exists
            let existing: Option<(i64, i64, f64)> = conn
                .query_row(
                    "SELECT success_count, failure_count, average_latency FROM tool_performance
                     WHERE tool_name = ?1 AND model_name = ?2 AND task_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6",
                    params![tool_name, model_name, task_type, uid, sid, aid],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .ok();

            if let Some((s_count, f_count, avg_lat)) = existing {
                let new_s = s_count + success_count;
                let new_f = f_count + failure_count;
                let total_runs = new_s + new_f;
                let new_lat = if total_runs > 0 {
                    let current_total_lat = (s_count + f_count) as f64 * avg_lat;
                    (current_total_lat + average_latency) / total_runs as f64
                } else {
                    0.0
                };
                conn.execute(
                    "UPDATE tool_performance SET success_count = ?1, failure_count = ?2, average_latency = ?3, last_used = ?4
                     WHERE tool_name = ?5 AND model_name = ?6 AND task_type = ?7 AND user_id = ?8 AND session_id = ?9 AND agent_id = ?10",
                    params![new_s, new_f, new_lat, last_used, tool_name, model_name, task_type, uid, sid, aid],
                )?;
            } else {
                conn.execute(
                    "INSERT INTO tool_performance (tool_name, model_name, task_type, success_count, failure_count, average_latency, last_used, user_id, session_id, agent_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![tool_name, model_name, task_type, success_count, failure_count, average_latency, last_used, uid, sid, aid],
                )?;
            }
            Ok(())
        })?;

        Ok(json!({ "status": "Tool performance metrics recorded" }))
    }
}

// ─── Tool: QueryToolPerformanceTool ──────────────────────────────

pub struct QueryToolPerformanceTool;

#[async_trait::async_trait]
impl Tool for QueryToolPerformanceTool {
    fn name(&self) -> &str { "query_tool_performance" }

    fn description(&self) -> &str {
        "Query tool performance logs to recommend optimal tools/models for specific task types."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "taskType": { "type": "string", "description": "Filter by task type" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["taskType"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let task_type = arguments["taskType"].as_str().ok_or_else(|| anyhow!("Missing 'taskType'"))?;
        let (uid, sid, aid) = scope_from_args(arguments);

        let results = with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT tool_name, model_name, task_type, success_count, failure_count, average_latency, last_used
                 FROM tool_performance
                 WHERE task_type = ?1
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')
                 ORDER BY success_count DESC, average_latency ASC"
            )?;
            let mut rows = stmt.query(params![task_type, uid, sid, aid])?;
            let mut results = Vec::new();
            while let Some(row) = rows.next()? {
                results.push(json!({
                    "toolName": row.get::<_, String>(0)?,
                    "modelName": row.get::<_, String>(1)?,
                    "taskType": row.get::<_, String>(2)?,
                    "successCount": row.get::<_, i64>(3)?,
                    "failureCount": row.get::<_, i64>(4)?,
                    "averageLatency": row.get::<_, f64>(5)?,
                    "lastUsed": row.get::<_, String>(6)?,
                }));
            }
            Ok(results)
        })?;

        Ok(json!(results))
    }
}

// ─── Tool: StoreSharedTeamMemoryTool ─────────────────────────────

pub struct StoreSharedTeamMemoryTool;

#[async_trait::async_trait]
impl Tool for StoreSharedTeamMemoryTool {
    fn name(&self) -> &str { "store_shared_team_memory" }

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
        let key = arguments["key"].as_str().ok_or_else(|| anyhow!("Missing 'key'"))?;
        let value = arguments["value"].as_str().ok_or_else(|| anyhow!("Missing 'value'"))?;
        let source_agent = arguments["sourceAgent"].as_str().ok_or_else(|| anyhow!("Missing 'sourceAgent'"))?;
        let target_agents: Vec<String> = arguments["targetAgents"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .ok_or_else(|| anyhow!("Missing 'targetAgents'"))?;
        let importance = arguments.get("importance").and_then(|v| v.as_f64()).unwrap_or(1.0);
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
    fn name(&self) -> &str { "retrieve_shared_team_memory" }

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
        let agent_id = arguments.get("agentId").and_then(|v| v.as_str()).unwrap_or("");
        let uid = arguments.get("userId").and_then(|v| v.as_str()).unwrap_or("*").to_string();
        let sid = arguments.get("sessionId").and_then(|v| v.as_str()).unwrap_or("*").to_string();
        let aid_scope = arguments.get("agentIdScope").and_then(|v| v.as_str()).unwrap_or("*").to_string();

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
                let target_agents: Vec<String> = serde_json::from_str(&targets_json).unwrap_or_default();

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

fn query_fts5(conn: &Connection, query: &str, limit: usize, uid: &str, sid: &str, aid: &str) -> Result<Vec<Value>> {
    let clean_query: String = query.chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect();
    let words: Vec<&str> = clean_query.split_whitespace().collect();
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let match_query = words.iter()
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
         LIMIT ?2"
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
    fn name(&self) -> &str { "search_text" }

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
        let query = arguments["query"].as_str().ok_or_else(|| anyhow!("Missing 'query'"))?;
        let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let (uid, sid, aid) = scope_from_args(arguments);

        let results = with_db(|conn| query_fts5(conn, query, limit, &uid, &sid, &aid))?;
        Ok(json!(results))
    }
}

// ─── Tool: HybridSearchTool (FTS5 + vector similarity) ───────────

pub struct HybridSearchTool;

#[async_trait::async_trait]
impl Tool for HybridSearchTool {
    fn name(&self) -> &str { "hybrid_search" }

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
        let query = arguments["query"].as_str().ok_or_else(|| anyhow!("Missing 'query'"))?;
        let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let (uid, sid, aid) = scope_from_args(arguments);

        let fts_results = with_db(|conn| query_fts5(conn, query, limit * 2, &uid, &sid, &aid))?;

        // Simple keyword overlap scoring as a substitute for vector search
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let all_facts = with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, raw_text, timestamp, importance
                 FROM semantic_metadata
                 WHERE valid_until IS NULL
                   AND (?1 IS NULL OR user_id = ?1 OR user_id = '*')
                   AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
                   AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')"
            )?;
            let mut rows = stmt.query(params![uid, sid, aid])?;
            let mut facts = Vec::new();
            while let Some(row) = rows.next()? {
                facts.push(json!({
                    "nodeId": row.get::<_, String>(0)?,
                    "rawText": row.get::<_, String>(1)?,
                    "timestamp": row.get::<_, String>(2)?,
                    "importance": row.get::<_, f64>(3)?,
                }));
            }
            Ok(facts)
        })?;

        // Score each fact by keyword overlap
        let mut scored: Vec<(Value, f64)> = all_facts.into_iter()
            .map(|fact| {
                let text = fact["rawText"].as_str().unwrap_or("").to_lowercase();
                let overlap = query_words.iter().filter(|w| text.contains(*w)).count();
                let score = if query_words.is_empty() { 0.0 } else { overlap as f64 / query_words.len() as f64 };
                (fact, score)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit * 2);

        // RRF fusion with FTS5 results
        let mut doc_scores: HashMap<String, f64> = HashMap::new();
        let mut doc_map: HashMap<String, Value> = HashMap::new();

        for (i, fact) in fts_results.iter().enumerate() {
            let rank = (i + 1) as f64;
            let node_id = fact["nodeId"].as_str().unwrap_or("");
            *doc_scores.entry(node_id.to_string()).or_insert(0.0) += 1.0 / (60.0 + rank);
            doc_map.entry(node_id.to_string()).or_insert_with(|| fact.clone());
        }

        for (i, (fact, _)) in scored.iter().enumerate() {
            let rank = (i + 1) as f64;
            let node_id = fact["nodeId"].as_str().unwrap_or("");
            *doc_scores.entry(node_id.to_string()).or_insert(0.0) += 1.0 / (60.0 + rank);
            doc_map.entry(node_id.to_string()).or_insert_with(|| fact.clone());
        }

        let mut ranked: Vec<(String, f64)> = doc_scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(limit);

        let results: Vec<Value> = ranked.into_iter()
            .filter_map(|(node_id, _)| doc_map.remove(&node_id))
            .collect();

        Ok(json!(results))
    }
}

// ─── Tool: InvalidateFactTool ────────────────────────────────────

pub struct InvalidateFactTool;

#[async_trait::async_trait]
impl Tool for InvalidateFactTool {
    fn name(&self) -> &str { "invalidate_fact" }

    fn description(&self) -> &str {
        "Invalidate a graph relation (via from/to/relationType) or a semantic fact (via factId)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "factId": { "type": "string", "description": "Semantic fact ID to invalidate" },
                "from": { "type": "string", "description": "Source entity name (for graph relation)" },
                "to": { "type": "string", "description": "Target entity name (for graph relation)" },
                "relationType": { "type": "string", "description": "Relation type (for graph relation)" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (uid, sid, aid) = scope_from_args(arguments);
        let mut messages = Vec::new();
        let mut parameter_provided = false;

        if let Some(fact_id) = arguments.get("factId").and_then(|v| v.as_str()) {
            parameter_provided = true;
            let updated = with_db(|conn| {
                let rows = conn.execute(
                    "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                     WHERE node_id = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4 AND valid_until IS NULL",
                    params![fact_id, uid, sid, aid],
                )?;
                Ok(rows > 0)
            })?;
            if updated {
                messages.push(format!("Semantic fact '{}' invalidated successfully", fact_id));
            } else {
                messages.push(format!("Semantic fact '{}' not found or already invalidated", fact_id));
            }
        }

        if let (Some(from), Some(to), Some(rel_type)) = (
            arguments.get("from").and_then(|v| v.as_str()),
            arguments.get("to").and_then(|v| v.as_str()),
            arguments.get("relationType").and_then(|v| v.as_str()),
        ) {
            parameter_provided = true;
            let updated = with_db(|conn| {
                let rows = conn.execute(
                    "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                     WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND valid_until IS NULL
                       AND (user_id = ?4 OR user_id = '*')
                       AND (session_id = ?5 OR session_id = '*')
                       AND (agent_id = ?6 OR agent_id = '*')",
                    params![from, to, rel_type, uid, sid, aid],
                )?;
                Ok(rows > 0)
            })?;
            if updated {
                messages.push(format!("Graph relation '{}->{} ({})' invalidated", from, to, rel_type));
            } else {
                messages.push(format!("Graph relation not found or already invalidated"));
            }
        }

        if !parameter_provided {
            return Err(anyhow!("Either factId or all of (from, to, relationType) must be provided"));
        }

        Ok(json!({ "status": messages.join("\n") }))
    }
}

// ─── Tool: QueryFactHistoryTool ──────────────────────────────────

pub struct QueryFactHistoryTool;

#[async_trait::async_trait]
impl Tool for QueryFactHistoryTool {
    fn name(&self) -> &str { "query_fact_history" }

    fn description(&self) -> &str {
        "Query the chronological history of relations/facts involving a specific entity."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entityName": { "type": "string", "description": "Name of the entity" },
                "relationType": { "type": "string", "description": "Optional relation type filter" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["entityName"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let entity_name = arguments["entityName"].as_str().ok_or_else(|| anyhow!("Missing 'entityName'"))?;
        let relation_type = arguments.get("relationType").and_then(|v| v.as_str());
        let (uid, sid, aid) = scope_from_args(arguments);

        let results = with_db(|conn| {
            let sql = if let Some(rel) = relation_type {
                "SELECT from_name, to_name, relation_type, valid_from, valid_until
                 FROM graph_edges
                 WHERE (from_name = ?1 OR to_name = ?1)
                   AND relation_type = ?2
                   AND (user_id = ?3 OR user_id = '*')
                   AND (session_id = ?4 OR session_id = '*')
                   AND (agent_id = ?5 OR agent_id = '*')
                 ORDER BY valid_from DESC"
            } else {
                "SELECT from_name, to_name, relation_type, valid_from, valid_until
                 FROM graph_edges
                 WHERE (from_name = ?1 OR to_name = ?1)
                   AND (user_id = ?2 OR user_id = '*')
                   AND (session_id = ?3 OR session_id = '*')
                   AND (agent_id = ?4 OR agent_id = '*')
                 ORDER BY valid_from DESC"
            };
            let mut stmt = conn.prepare(sql)?;
            let mut rows = if let Some(_) = relation_type {
                stmt.query(params![entity_name, relation_type, uid, sid, aid])?
            } else {
                stmt.query(params![entity_name, uid, sid, aid])?
            };
            let mut results = Vec::new();
            while let Some(row) = rows.next()? {
                results.push(json!({
                    "from": row.get::<_, String>(0)?,
                    "to": row.get::<_, String>(1)?,
                    "relationType": row.get::<_, String>(2)?,
                    "validFrom": row.get::<_, String>(3)?,
                    "validUntil": row.get::<_, Option<String>>(4)?,
                }));
            }
            Ok(results)
        })?;

        Ok(json!(results))
    }
}

// ─── Tool: QueryAsOfTool ─────────────────────────────────────────

pub struct QueryAsOfTool;

#[async_trait::async_trait]
impl Tool for QueryAsOfTool {
    fn name(&self) -> &str { "query_as_of" }

    fn description(&self) -> &str {
        "Query both Graph and Semantic memory states as of a specific point in time (ISO 8601 datetime)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "asOf": { "type": "string", "description": "ISO 8601 datetime" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["asOf"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let as_of = arguments["asOf"].as_str().ok_or_else(|| anyhow!("Missing 'asOf'"))?;
        let (uid, sid, aid) = scope_from_args(arguments);

        // Normalize timestamp
        let normalized = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(as_of) {
            dt.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string()
        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(as_of, "%Y-%m-%dT%H:%M:%SZ") {
            dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(as_of, "%Y-%m-%d %H:%M:%S") {
            dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
        } else {
            return Err(anyhow!("Invalid datetime format: '{}'. Expected RFC3339.", as_of));
        };

        let graph_snapshot = with_db(|conn| {
            let mut entities = Vec::new();
            let mut stmt_nodes = conn.prepare(
                "SELECT name, entity_type, observations, created_at
                 FROM graph_nodes
                 WHERE created_at <= ?1
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
            )?;
            let mut node_rows = stmt_nodes.query(params![normalized, uid, sid, aid])?;
            while let Some(row) = node_rows.next()? {
                let obs_json: String = row.get(2)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                entities.push(json!({
                    "name": row.get::<_, String>(0)?,
                    "entityType": row.get::<_, String>(1)?,
                    "observations": observations,
                    "createdAt": row.get::<_, String>(3)?,
                }));
            }

            let mut relations = Vec::new();
            let mut stmt_edges = conn.prepare(
                "SELECT from_name, to_name, relation_type, valid_from, valid_until
                 FROM graph_edges
                 WHERE valid_from <= ?1
                   AND (valid_until IS NULL OR valid_until > ?1)
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
            )?;
            let mut edge_rows = stmt_edges.query(params![normalized, uid, sid, aid])?;
            while let Some(row) = edge_rows.next()? {
                relations.push(json!({
                    "from": row.get::<_, String>(0)?,
                    "to": row.get::<_, String>(1)?,
                    "relationType": row.get::<_, String>(2)?,
                    "validFrom": row.get::<_, String>(3)?,
                    "validUntil": row.get::<_, Option<String>>(4)?,
                }));
            }
            Ok(json!({ "entities": entities, "relations": relations }))
        })?;

        let semantic_snapshot = with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, raw_text, timestamp, importance
                 FROM semantic_metadata
                 WHERE valid_from <= ?1
                   AND (valid_until IS NULL OR valid_until > ?1)
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
            )?;
            let mut rows = stmt.query(params![normalized, uid, sid, aid])?;
            let mut facts = Vec::new();
            while let Some(row) = rows.next()? {
                facts.push(json!({
                    "nodeId": row.get::<_, String>(0)?,
                    "rawText": row.get::<_, String>(1)?,
                    "timestamp": row.get::<_, String>(2)?,
                    "importance": row.get::<_, f64>(3)?,
                }));
            }
            Ok(facts)
        })?;

        Ok(json!({
            "graph": graph_snapshot,
            "semantic": semantic_snapshot,
        }))
    }
}

// ─── SmartStoreTool (dedup + merge aware store) ──────────────────

pub struct SmartStoreTool;

#[async_trait::async_trait]
impl Tool for SmartStoreTool {
    fn name(&self) -> &str { "smart_store" }

    fn description(&self) -> &str {
        "Intelligently store or merge memories in Semantic and Graph layers using deduplication and decision logic."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text statement to store in semantic memory" },
                "relation": {
                    "type": "object",
                    "properties": {
                        "from": { "type": "string" },
                        "to": { "type": "string" },
                        "relationType": { "type": "string" }
                    },
                    "description": "Graph relation to store"
                },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let text = arguments.get("text").and_then(|v| v.as_str());
        let relation = arguments.get("relation");
        let (uid, sid, aid) = scope_from_args(arguments);

        // Handle relation input (Graph Layer)
        if let Some(rel) = relation {
            let from = rel["from"].as_str().ok_or_else(|| anyhow!("Relation missing 'from'"))?;
            let to = rel["to"].as_str().ok_or_else(|| anyhow!("Relation missing 'to'"))?;
            let rel_type = rel["relationType"].as_str().ok_or_else(|| anyhow!("Relation missing 'relationType'"))?;

            let exclusive_relations = ["lives_in", "current_job", "spouse", "has_status", "is_born_in", "located_in"];

            if exclusive_relations.contains(&rel_type) {
                // Check for existing relation and supersede if needed
                let existing = with_db(|conn| -> Result<Option<String>> {
                    let mut stmt = conn.prepare(
                        "SELECT to_name FROM graph_edges
                         WHERE from_name = ?1 AND relation_type = ?2 AND valid_until IS NULL
                           AND (user_id = ?3 OR user_id = '*')
                           AND (session_id = ?4 OR session_id = '*')
                           AND (agent_id = ?5 OR agent_id = '*')"
                    )?;
                    let mut rows = stmt.query(params![from, rel_type, uid, sid, aid])?;
                    if let Some(row) = rows.next()? {
                        Ok(Some(row.get(0)?))
                    } else {
                        Ok(None)
                    }
                })?;

                if let Some(old_to) = existing {
                    if old_to != to {
                        with_db(|conn| {
                            conn.execute(
                                "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                                 WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND valid_until IS NULL
                                   AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6",
                                params![from, old_to, rel_type, uid, sid, aid],
                            )?;
                            Ok(())
                        })?;

                        // Insert new edge via graph_memory tables directly
                        with_db(|conn| {
                            conn.execute(
                                "INSERT INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                params![from, to, rel_type, uid, sid, aid],
                            )?;
                            Ok(())
                        })?;

                        return Ok(json!({
                            "action": "superseded",
                            "layer": "graph",
                            "message": format!("Superseded '{} {} {}' -> '{} {} {}'", from, rel_type, old_to, to, rel_type, from),
                        }));
                    } else {
                        return Ok(json!({
                            "action": "no-op",
                            "layer": "graph",
                            "message": format!("Relation already exists: '{} {} {}'", from, rel_type, to),
                        }));
                    }
                }
            }

            // Insert directly
            with_db(|conn| {
                conn.execute(
                    "INSERT INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![from, to, rel_type, uid, sid, aid],
                )?;
                Ok(())
            })?;

            return Ok(json!({
                "action": "add",
                "layer": "graph",
                "message": format!("Created relation '{}->{} ({})'", from, to, rel_type),
            }));
        }

        // Handle text input (Semantic Layer)
        if let Some(t) = text {
            // Check for duplicates via FTS5
            let fts_matches = with_db(|conn| query_fts5(conn, t, 5, &uid, &sid, &aid))?;
            let best_match = fts_matches.into_iter().max_by(|a, b| {
                let sim_a = text_similarity(t, a["rawText"].as_str().unwrap_or(""));
                let sim_b = text_similarity(t, b["rawText"].as_str().unwrap_or(""));
                sim_a.partial_cmp(&sim_b).unwrap_or(std::cmp::Ordering::Equal)
            });

            if let Some(matched) = best_match {
                let node_id = matched["nodeId"].as_str().unwrap_or("");
                let existing_text = matched["rawText"].as_str().unwrap_or("");
                let similarity = text_similarity(t, existing_text);

                if similarity >= 0.98 {
                    return Ok(json!({
                        "action": "no-op",
                        "layer": "semantic",
                        "message": format!("Duplicate found (sim: {:.3}). No action needed.", similarity),
                        "winnerId": node_id,
                    }));
                } else if similarity >= 0.85 {
                    // Merge: keep the longer text
                    let merged = if t.len() >= existing_text.len() { t.to_string() } else { existing_text.to_string() };
                    with_db(|conn| {
                        conn.execute(
                            "UPDATE semantic_metadata SET raw_text = ?1 WHERE node_id = ?2 AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5",
                            params![merged, node_id, uid, sid, aid],
                        )?;
                        let _ = conn.execute(
                            "UPDATE semantic_fts SET raw_text = ?1 WHERE node_id = ?2",
                            params![merged, node_id],
                        );
                        Ok(())
                    })?;

                    return Ok(json!({
                        "action": "merge",
                        "layer": "semantic",
                        "message": format!("Merged with existing fact '{}'", node_id),
                        "winnerId": node_id,
                    }));
                }
            }

            // Add as new fact
            let fact_id = format!("fact-{}", uuid::Uuid::new_v4());
            store_semantic_fact(&fact_id, t, 0.8, &uid, &sid, &aid)?;

            return Ok(json!({
                "action": "add",
                "layer": "semantic",
                "message": format!("Added new fact '{}'", fact_id),
                "winnerId": fact_id,
            }));
        }

        Err(anyhow!("Either text or relation must be provided"))
    }
}

// Simple text similarity via word overlap
fn text_similarity(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    let a_words: Vec<&str> = a_lower.split_whitespace().collect();
    let b_words: Vec<&str> = b_lower.split_whitespace().collect();
    if a_words.is_empty() && b_words.is_empty() { return 1.0; }
    if a_words.is_empty() || b_words.is_empty() { return 0.0; }

    let a_set: std::collections::HashSet<&&str> = a_words.iter().collect();
    let b_set: std::collections::HashSet<&&str> = b_words.iter().collect();

    let intersection = a_set.intersection(&b_set).count();
    let union = a_set.union(&b_set).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

// ─── Tool: ExtractAndStoreFactsTool ──────────────────────────────

pub struct ExtractAndStoreFactsTool;

#[async_trait::async_trait]
impl Tool for ExtractAndStoreFactsTool {
    fn name(&self) -> &str { "extract_and_store_facts" }

    fn description(&self) -> &str {
        "Extract facts from text using regular expressions and store them in the graph memory."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to extract facts from" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["text"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let text = arguments["text"].as_str().ok_or_else(|| anyhow!("Missing 'text'"))?;
        let (uid, sid, aid) = scope_from_args(arguments);

        let facts = extract_facts(text);

        let mut entities_created = 0;
        let mut relations_created = 0;

        for fact in facts {
            // Ensure entities exist
            for name in [&fact.from, &fact.to] {
                let exists = with_db(|conn| -> Result<bool> {
                    let exists: bool = conn.query_row(
                        "SELECT EXISTS(SELECT 1 FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4)",
                        params![name, uid, sid, aid],
                        |row| row.get(0),
                    )?;
                    Ok(exists)
                })?;
                if !exists {
                    with_db(|conn| {
                        conn.execute(
                            "INSERT INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id)
                             VALUES (?1, 'Concept', '[]', ?2, ?3, ?4)",
                            params![name, uid, sid, aid],
                        )?;
                        Ok(())
                    })?;
                    entities_created += 1;
                }
            }

            // Create relation
            let exists = with_db(|conn| -> Result<bool> {
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM graph_edges WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6 AND valid_until IS NULL)",
                    params![fact.from, fact.to, fact.relation, uid, sid, aid],
                    |row| row.get(0),
                )?;
                Ok(exists)
            })?;

            if !exists {
                with_db(|conn| {
                    conn.execute(
                        "INSERT INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![fact.from, fact.to, fact.relation, uid, sid, aid],
                    )?;
                    Ok(())
                })?;
                relations_created += 1;
            }
        }

        Ok(json!({
            "factsExtracted": 0, // Count from extractor (approximate)
            "entitiesCreated": entities_created,
            "relationsCreated": relations_created,
            "status": format!("Extracted facts from text, created {} entities and {} relations", entities_created, relations_created),
        }))
    }
}

struct ExtractedFact {
    from: String,
    relation: String,
    to: String,
}

fn extract_facts(text: &str) -> Vec<ExtractedFact> {
    let patterns: Vec<(regex::Regex, &str)> = vec![
        (regex::Regex::new(r"(\w+)\s+(?:(?:is|are|was|were|am)\s+)?(?:uses?|using)\s+(\w+)").unwrap(), "uses"),
        (regex::Regex::new(r"(\w+)\s+(?:depends\s+on|requires?)\s+(\w+)").unwrap(), "depends_on"),
        (regex::Regex::new(r"(\w+)\s+(?:prefers?|likes?|favou?rs?)\s+(\w+)").unwrap(), "prefers"),
        (regex::Regex::new(r"(\w+)\s+(?:is\s+a|is\s+an|is\s+the)\s+(\w+)").unwrap(), "is_a"),
        (regex::Regex::new(r"(\w+)\s+(?:(?:is|are|was|were|am)\s+)?(?:works?|working)\s+(?:on|with|at)\s+(\w+)").unwrap(), "works_with"),
        (regex::Regex::new(r"(\w+)\s+(?:created?|built?|wrote?)\s+(\w+)").unwrap(), "created"),
    ];

    let mut facts = Vec::new();
    // Simple sentence splitting by punctuation
    for sentence in text.split(|c: char| c == '.' || c == '!' || c == '?') {
        let sentence = sentence.trim();
        if sentence.is_empty() { continue; }
        for (pattern, rel_type) in &patterns {
            for caps in pattern.captures_iter(sentence) {
                let from = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let to = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                if !from.is_empty() && !to.is_empty() && is_valid_entity(from) && is_valid_entity(to) {
                    facts.push(ExtractedFact {
                        from: from.to_string(),
                        relation: rel_type.to_string(),
                        to: to.to_string(),
                    });
                }
            }
        }
    }
    facts
}

const STOP_WORDS: &[&str] = &[
    "a", "about", "after", "again", "all", "am", "an", "and", "any", "are", "as", "at",
    "be", "been", "before", "being", "below", "between", "both", "but", "by",
    "can", "did", "do", "does", "doing", "down", "during",
    "each", "few", "for", "from", "further",
    "had", "has", "have", "having", "he", "her", "here", "hers", "herself", "him", "himself", "his", "how",
    "i", "if", "in", "into", "is", "it", "its", "itself",
    "just", "me", "more", "most", "my", "myself",
    "no", "nor", "not", "now",
    "of", "off", "on", "once", "only", "or", "other", "our", "ours", "ourselves", "out", "over", "own",
    "same", "she", "should", "so", "some", "someone", "something", "than", "that", "the", "their", "theirs", "them", "themselves", "then", "there", "these", "they", "this", "those", "through", "to", "too",
    "under", "until", "up",
    "very",
    "was", "we", "were", "what", "when", "where", "which", "who", "whom", "why", "will", "with",
    "you", "your", "yours", "yourself", "yourselves",
];

const COMMON_NOUNS: &[&str] = &[
    "app", "application", "code", "compiler", "computer", "database", "db", "developer", "engine", "engineer", "file", "framework", "hardware", "interpreter", "job", "language", "library", "machine", "program", "programmer", "project", "server", "software", "system", "thing", "things", "tool", "user", "work",
];

fn is_valid_entity(word: &str) -> bool {
    let lower = word.to_lowercase();
    if lower.is_empty() { return false; }
    if STOP_WORDS.binary_search(&lower.as_str()).is_ok() { return false; }
    if word == lower && COMMON_NOUNS.binary_search(&lower.as_str()).is_ok() { return false; }
    true
}

// ─── Tool: ProactiveRecallTool ───────────────────────────────────

pub struct ProactiveRecallTool;

#[async_trait::async_trait]
impl Tool for ProactiveRecallTool {
    fn name(&self) -> &str { "proactive_recall" }

    fn description(&self) -> &str {
        "Recall contextually relevant memories across semantic, graph, and episodic layers given a query context."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search context" },
                "maxResults": { "type": "integer", "default": 10 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments["query"].as_str().ok_or_else(|| anyhow!("Missing 'query'"))?;
        let max_results = arguments.get("maxResults").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let (uid, sid, aid) = scope_from_args(arguments);

        // Extract keywords
        let keywords: Vec<String> = query.chars()
            .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() >= 3 && !STOP_WORDS.binary_search(&w.as_str()).is_ok())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let mut items: Vec<Value> = Vec::new();

        // 1. Semantic memory via FTS5
        if !query.trim().is_empty() {
            if let Ok(fts_results) = with_db(|conn| query_fts5(conn, query, max_results, &uid, &sid, &aid)) {
                for fact in fts_results {
                    let mut item = json!({
                        "layer": "semantic",
                        "content": fact["rawText"],
                        "confidence": 0.85,
                        "metadata": {
                            "nodeId": fact["nodeId"],
                            "timestamp": fact["timestamp"],
                            "importance": fact["importance"],
                        }
                    });
                    if let Some(c) = item["confidence"].as_f64() {
                        item["confidence"] = json!(c.min(1.0));
                    }
                    items.push(item);
                }
            }
        }

        // 2. Graph memory via LIKE search
        let query_pattern = format!("%{}%", query.to_lowercase());
        let graph_results = with_db(|conn| -> Result<Vec<Value>> {
            let mut entities = Vec::new();
            let mut stmt = conn.prepare(
                "SELECT name, entity_type, observations FROM graph_nodes
                 WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1)
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
            )?;
            let mut rows = stmt.query(params![query_pattern, uid, sid, aid])?;
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let entity_type: String = row.get(1)?;
                let obs_json: String = row.get(2)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json).unwrap_or_default();
                let obs_str = if observations.is_empty() { String::new() } else { format!(" - Observations: {}", observations.join(", ")) };
                entities.push(json!({
                    "name": name,
                    "entityType": entity_type,
                    "observations": observations,
                    "content": format!("Entity: {} ({}){}", name, entity_type, obs_str),
                }));
            }
            Ok(entities)
        })?;

        for entity in graph_results {
            let confidence = if keywords.is_empty() { 0.6 } else {
                let lower_name = entity["name"].as_str().unwrap_or("").to_lowercase();
                let lower_type = entity["entityType"].as_str().unwrap_or("").to_lowercase();
                let obs_text: String = entity["observations"].as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" "))
                    .unwrap_or_default()
                    .to_lowercase();
                let match_count = keywords.iter().filter(|kw| {
                    lower_name.contains(kw.as_str()) || lower_type.contains(kw.as_str()) || obs_text.contains(kw.as_str())
                }).count();
                (0.6 + 0.1 * match_count as f64).min(1.0)
            };
            items.push(json!({
                "layer": "graph",
                "content": entity["content"],
                "confidence": confidence,
                "metadata": {
                    "name": entity["name"],
                    "entityType": entity["entityType"],
                    "observations": entity["observations"],
                }
            }));
        }

        // 3. Episodic reflections
        if !query.trim().is_empty() {
            let pattern = format!("%{}%", query);
            let reflections = with_db(|conn| -> Result<Vec<Value>> {
                let mut stmt = conn.prepare(
                    "SELECT id, task_description, status, attempt_number, reflection, root_cause, solution_applied, created_at
                     FROM reflection_memory
                     WHERE (task_description LIKE ?1 OR reflection LIKE ?1 OR root_cause LIKE ?1)
                       AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                       AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                       AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')
                     ORDER BY created_at DESC"
                )?;
                let mut rows = stmt.query(params![pattern, uid, sid, aid])?;
                let mut results = Vec::new();
                while let Some(row) = rows.next()? {
                    results.push(json!({
                        "id": row.get::<_, String>(0)?,
                        "taskDescription": row.get::<_, String>(1)?,
                        "status": row.get::<_, String>(2)?,
                        "attemptNumber": row.get::<_, i64>(3)?,
                        "reflection": row.get::<_, String>(4)?,
                        "rootCause": row.get::<_, Option<String>>(5)?,
                        "solutionApplied": row.get::<_, Option<String>>(6)?,
                        "createdAt": row.get::<_, String>(7)?,
                    }));
                }
                Ok(results)
            })?;

            for r in reflections {
                let confidence = if keywords.is_empty() { 0.6 } else {
                    let text_to_check = format!(
                        "{} {} {} {}",
                        r["taskDescription"].as_str().unwrap_or(""),
                        r["reflection"].as_str().unwrap_or(""),
                        r["rootCause"].as_str().unwrap_or(""),
                        r["solutionApplied"].as_str().unwrap_or(""),
                    ).to_lowercase();
                    let match_count = keywords.iter().filter(|kw| text_to_check.contains(kw.as_str())).count();
                    (0.6 + 0.1 * match_count as f64).min(1.0)
                };
                items.push(json!({
                    "layer": "episodic",
                    "content": format!(
                        "Reflection on '{}' (Status: {}) | {} | Root Cause: {} | Solution: {}",
                        r["taskDescription"],
                        r["status"],
                        r["reflection"],
                        r["rootCause"].as_str().unwrap_or("None"),
                        r["solutionApplied"].as_str().unwrap_or("None"),
                    ),
                    "confidence": confidence,
                    "metadata": {
                        "id": r["id"],
                        "taskDescription": r["taskDescription"],
                        "status": r["status"],
                        "createdAt": r["createdAt"],
                    }
                }));
            }
        }

        // Sort by confidence descending
        items.sort_by(|a, b| {
            let ca = a["confidence"].as_f64().unwrap_or(0.0);
            let cb = b["confidence"].as_f64().unwrap_or(0.0);
            cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
        });
        items.truncate(max_results);

        Ok(json!(items))
    }
}

// ─── Tool: CompressContextTool (TF-IDF sentence scoring) ─────────

pub struct CompressContextTool;

#[async_trait::async_trait]
impl Tool for CompressContextTool {
    fn name(&self) -> &str { "compress_context" }

    fn description(&self) -> &str {
        "Compress text context by scoring sentences using TF-IDF and keeping a specified ratio."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to compress" },
                "ratio": { "type": "number", "default": 0.5, "description": "Ratio of sentences to keep (0.0-1.0)" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["text"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let text = arguments["text"].as_str().ok_or_else(|| anyhow!("Missing 'text'"))?;
        let ratio = arguments.get("ratio").and_then(|v| v.as_f64()).unwrap_or(0.5).clamp(0.0, 1.0);

        // Simple sentence splitting
        let sentences: Vec<&str> = text.split(|c: char| c == '.' || c == '!' || c == '?')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if sentences.is_empty() {
            return Ok(json!({
                "originalLength": text.len(),
                "compressedLength": 0,
                "ratio": ratio,
                "compressedText": "",
            }));
        }

        // TF-IDF scoring (simplified)
        let mut sentence_terms: Vec<Vec<String>> = Vec::new();
        let mut term_dfs: HashMap<String, usize> = HashMap::new();

        for &sentence in &sentences {
            let words: Vec<String> = sentence.chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>()
                .split_whitespace()
                .map(|w| w.to_lowercase())
                .filter(|w| w.len() >= 3 && !STOP_WORDS.binary_search(&w.as_str()).is_ok())
                .collect();
            let unique_terms: std::collections::HashSet<String> = words.iter().cloned().collect();
            sentence_terms.push(words);
            for term in unique_terms {
                *term_dfs.entry(term.clone()).or_insert(0) += 1;
            }
        }

        let n_sentences = sentences.len() as f64;
        let mut scored: Vec<(usize, f64)> = Vec::new();

        for (i, terms) in sentence_terms.iter().enumerate() {
            let mut tfidf = 0.0f64;
            let mut seen = std::collections::HashSet::new();
            for term in terms {
                if !seen.insert(term) { continue; }
                let tf = terms.iter().filter(|t| *t == term).count() as f64 / terms.len() as f64;
                let df = *term_dfs.get(term).unwrap_or(&1) as f64;
                let idf = (n_sentences / df).ln() + 1.0;
                tfidf += tf * idf;
            }
            scored.push((i, tfidf));
        }

        // Sort by score descending, keep top ratio
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let keep_count = ((n_sentences * ratio).ceil() as usize).max(1);
        let top_indices: std::collections::HashSet<usize> = scored.iter().take(keep_count).map(|(i, _)| *i).collect();

        // Reconstruct in original order
        let compressed: String = sentences.iter().enumerate()
            .filter(|(i, _)| top_indices.contains(i))
            .map(|(_, s)| *s)
            .collect::<Vec<&str>>()
            .join(". ");

        Ok(json!({
            "originalLength": text.len(),
            "compressedLength": compressed.len(),
            "ratio": ratio,
            "compressedText": if compressed.is_empty() { sentences[0] } else { &compressed },
        }))
    }
}

// ─── Tool: MemoryStatsTool ──────────────────────────────────────

pub struct MemoryStatsTool;

#[async_trait::async_trait]
impl Tool for MemoryStatsTool {
    fn name(&self) -> &str { "memory_stats" }

    fn description(&self) -> &str {
        "Get memory access statistics and record counts for all layers."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        with_db(|conn| {
            let graph_nodes: i64 = conn.query_row("SELECT COUNT(*) FROM graph_nodes", [], |r| r.get(0))?;
            let graph_edges: i64 = conn.query_row("SELECT COUNT(*) FROM graph_edges WHERE valid_until IS NULL", [], |r| r.get(0))?;
            let episodic: i64 = conn.query_row("SELECT COUNT(*) FROM episodic_logs", [], |r| r.get(0))?;
            let reflections: i64 = conn.query_row("SELECT COUNT(*) FROM reflection_memory", [], |r| r.get(0))?;
            let tool_perf: i64 = conn.query_row("SELECT COUNT(*) FROM tool_performance", [], |r| r.get(0))?;
            let shared: i64 = conn.query_row("SELECT COUNT(*) FROM shared_agent_memory", [], |r| r.get(0))?;
            let semantic: i64 = conn.query_row("SELECT COUNT(*) FROM semantic_metadata WHERE valid_until IS NULL", [], |r| r.get(0))?;
            let working: i64 = conn.query_row("SELECT COUNT(*) FROM working_memory WHERE expired = 0", [], |r| r.get(0)).unwrap_or(0);
            Ok(json!({
                "graphNodes": graph_nodes, "graphEdges": graph_edges, "episodicLogs": episodic,
                "reflections": reflections, "toolPerformance": tool_perf, "sharedMemory": shared,
                "semanticFacts": semantic, "workingMemory": working,
            }))
        })
    }
}

// ─── Tool: LogRepositoryEvolutionTool ────────────────────────────

pub struct LogRepositoryEvolutionTool;

#[async_trait::async_trait]
impl Tool for LogRepositoryEvolutionTool {
    fn name(&self) -> &str { "log_repository_evolution" }

    fn description(&self) -> &str {
        "Log file changes, refactoring records, commits, versions, and bug status metrics."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": { "type": "string" },
                "version": { "type": "string" },
                "commitHash": { "type": "string" },
                "author": { "type": "string" },
                "changeType": { "type": "string", "description": "e.g. added, modified, refactored, deleted" },
                "summary": { "type": "string" },
                "bugIntroduced": { "type": "boolean" },
                "bugFixed": { "type": "boolean" }
            },
            "required": ["filePath", "version", "changeType", "summary"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments["filePath"].as_str().ok_or_else(|| anyhow!("Missing filePath"))?;
        let version = arguments["version"].as_str().ok_or_else(|| anyhow!("Missing version"))?;
        let commit_hash = arguments["commitHash"].as_str().unwrap_or("");
        let author = arguments["author"].as_str().unwrap_or("");
        let change_type = arguments["changeType"].as_str().ok_or_else(|| anyhow!("Missing changeType"))?;
        let summary = arguments["summary"].as_str().ok_or_else(|| anyhow!("Missing summary"))?;
        let bug_introduced = arguments["bugIntroduced"].as_bool().unwrap_or(false) as i32;
        let bug_fixed = arguments["bugFixed"].as_bool().unwrap_or(false) as i32;

        with_db(|conn| {
            conn.execute(
                "INSERT INTO repo_evolution (file_path, version, commit_hash, author, change_type, summary, bug_introduced, bug_fixed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![file_path, version, commit_hash, author, change_type, summary, bug_introduced, bug_fixed],
            )?;
            Ok(json!({ "status": "logged" }))
        })
    }
}

// ─── Tool: QueryRepositoryEvolutionTool ──────────────────────────

pub struct QueryRepositoryEvolutionTool;

#[async_trait::async_trait]
impl Tool for QueryRepositoryEvolutionTool {
    fn name(&self) -> &str { "query_repository_evolution" }

    fn description(&self) -> &str {
        "Query repository file history and change statistics."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": { "type": "string", "description": "Optional filter by file path" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments["filePath"].as_str();

        with_db(|conn| {
            if let Some(fp) = file_path {
                let mut stmt = conn.prepare("SELECT * FROM repo_evolution WHERE file_path = ?1 ORDER BY created_at DESC")?;
                let rows = stmt.query_map(params![fp], map_repo_row)?;
                let mut entries = Vec::new();
                for r in rows { entries.push(r?); }
                Ok(json!({ "entries": entries, "totalCount": entries.len() }))
            } else {
                let mut stmt = conn.prepare(
                    "SELECT file_path, COUNT(*) as changes, SUM(bug_introduced) as bugs_introduced, SUM(bug_fixed) as bugs_fixed FROM repo_evolution GROUP BY file_path ORDER BY changes DESC"
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(json!({
                        "filePath": row.get::<_, String>(0)?,
                        "changes": row.get::<_, i64>(1)?,
                        "bugsIntroduced": row.get::<_, i64>(2)?,
                        "bugsFixed": row.get::<_, i64>(3)?,
                    }))
                })?;
                let mut stats = Vec::new();
                for r in rows { stats.push(r?); }
                Ok(json!({ "statistics": stats }))
            }
        })
    }
}

fn map_repo_row(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "filePath": row.get::<_, String>(1)?,
        "version": row.get::<_, String>(2)?,
        "commitHash": row.get::<_, String>(3)?,
        "author": row.get::<_, String>(4)?,
        "changeType": row.get::<_, String>(5)?,
        "summary": row.get::<_, String>(6)?,
        "bugIntroduced": row.get::<_, bool>(7)?,
        "bugFixed": row.get::<_, bool>(8)?,
        "createdAt": row.get::<_, String>(9)?,
    }))
}

// ─── Tool: TraverseGraphTool ─────────────────────────────────────

pub struct TraverseGraphTool;

#[async_trait::async_trait]
impl Tool for TraverseGraphTool {
    fn name(&self) -> &str { "traverse_graph" }

    fn description(&self) -> &str {
        "Traverse nodes and edges from a start entity using BFS up to a maximum depth."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "startEntity": { "type": "string" },
                "maxDepth": { "type": "integer", "default": 2 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["startEntity"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let start = arguments["startEntity"].as_str().ok_or_else(|| anyhow!("Missing startEntity"))?;
        let max_depth = arguments["maxDepth"].as_i64().unwrap_or(2) as usize;
        let (uid, sid, aid) = scope_from_args(arguments);

        with_db(|conn| {
            let mut visited = std::collections::HashSet::new();
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            let mut queue = std::collections::VecDeque::new();
            queue.push_back((start.to_string(), 0usize));

            while let Some((current, depth)) = queue.pop_front() {
                if !visited.insert(current.clone()) || depth > max_depth { continue; }

                // Get node info
                if let Ok(node) = conn.query_row(
                    "SELECT name, entity_type, observations FROM graph_nodes WHERE name = ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
                    params![current, uid, sid, aid],
                    |row| {
                        let name: String = row.get(0)?;
                        let etype: String = row.get(1)?;
                        let obs: String = row.get(2)?;
                        Ok(json!({ "name": name, "entityType": etype, "observations": serde_json::from_str::<Vec<String>>(&obs).unwrap_or_default() }))
                    }
                ) {
                    nodes.push(node);
                }

                if depth >= max_depth { continue; }

                // Get neighbors
                let mut stmt = conn.prepare(
                    "SELECT from_name, to_name, relation_type FROM graph_edges WHERE (from_name = ?1 OR to_name = ?1) AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*') AND valid_until IS NULL"
                )?;
                let rows = stmt.query_map(params![current, uid, sid, aid], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                })?;
                for r in rows {
                    let (from, to, rel_type) = r?;
                    edges.push(json!({ "from": from.clone(), "to": to.clone(), "relationType": rel_type }));
                    let neighbor = if from == current { &to } else { &from };
                    if !visited.contains(neighbor) {
                        queue.push_back((neighbor.clone(), depth + 1));
                    }
                }
            }

            Ok(json!({ "nodes": nodes, "edges": edges, "maxDepth": max_depth }))
        })
    }
}

// ─── Tool: FindPathTool ──────────────────────────────────────────

pub struct FindPathTool;

#[async_trait::async_trait]
impl Tool for FindPathTool {
    fn name(&self) -> &str { "find_path" }

    fn description(&self) -> &str {
        "Find the shortest path and relations between two entity nodes using BFS."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "startEntity": { "type": "string" },
                "targetEntity": { "type": "string" },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            },
            "required": ["startEntity", "targetEntity"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let start = arguments["startEntity"].as_str().ok_or_else(|| anyhow!("Missing startEntity"))?;
        let target = arguments["targetEntity"].as_str().ok_or_else(|| anyhow!("Missing targetEntity"))?;
        let (uid, sid, aid) = scope_from_args(arguments);

        with_db(|conn| {
            // BFS tracking parents to reconstruct path
            let mut parent: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new(); // child -> (parent, relation_type)
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(start.to_string());
            parent.insert(start.to_string(), (String::new(), String::new()));

            let mut found = false;
            while let Some(current) = queue.pop_front() {
                if current == target { found = true; break; }

                let mut stmt = conn.prepare(
                    "SELECT from_name, to_name, relation_type FROM graph_edges WHERE (from_name = ?1 OR to_name = ?1) AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*') AND valid_until IS NULL"
                )?;
                let rows = stmt.query_map(params![current, uid, sid, aid], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                })?;
                for r in rows {
                    let (from, to, rel_type) = r?;
                    let neighbor = if from == current { &to } else { &from };
                    if !parent.contains_key(neighbor) {
                        parent.insert(neighbor.clone(), (current.clone(), rel_type));
                        queue.push_back(neighbor.clone());
                    }
                }
            }

            if !found {
                return Ok(json!({ "found": false, "path": [] }));
            }

            // Reconstruct path
            let mut path = Vec::new();
            let mut current = target.to_string();
            while current != start {
                if let Some((p, rel)) = parent.get(&current) {
                    path.push(json!({ "from": p, "to": current, "relationType": rel }));
                    current = p.clone();
                } else { break; }
            }
            path.reverse();

            Ok(json!({ "found": true, "pathLength": path.len(), "path": path }))
        })
    }
}

// ─── Tool: AnalyzeGraphCommunitiesTool ──────────────────────────

pub struct AnalyzeGraphCommunitiesTool;

#[async_trait::async_trait]
impl Tool for AnalyzeGraphCommunitiesTool {
    fn name(&self) -> &str { "analyze_graph_communities" }

    fn description(&self) -> &str {
        "Cluster the entity-relation graph into weakly connected communities with summaries."
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
        let (uid, sid, aid) = scope_from_args(arguments);

        with_db(|conn| {
            // Collect all node names
            let mut stmt = conn.prepare(
                "SELECT name FROM graph_nodes WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')"
            )?;
            let names: Vec<String> = stmt.query_map(params![uid, sid, aid], |r| r.get(0))?
                .filter_map(|r| r.ok()).collect();

            // Union-Find
            let mut parent: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            for n in &names { parent.insert(n.clone(), n.clone()); }

            fn find(p: &mut std::collections::HashMap<String, String>, x: &str) -> String {
                let px = p.get(x).cloned().unwrap_or_default();
                if px != x {
                    let root = find(p, &px);
                    p.insert(x.to_string(), root.clone());
                    root
                } else { x.to_string() }
            }

            let mut edge_stmt = conn.prepare(
                "SELECT from_name, to_name FROM graph_edges WHERE valid_until IS NULL AND (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')"
            )?;
            let edges = edge_stmt.query_map(params![uid, sid, aid], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for e in edges {
                if let Ok((from, to)) = e {
                    let rf = find(&mut parent, &from);
                    let rt = find(&mut parent, &to);
                    if rf != rt { parent.insert(rf, rt); }
                }
            }

            // Group by root
            let mut communities: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
            for n in &names {
                let root = find(&mut parent, n);
                communities.entry(root).or_default().push(n.clone());
            }

            let mut result = Vec::new();
            for (_, members) in communities {
                if members.len() < 2 { continue; }
                let summary = format!("{} entities: {}", members.len(), members.join(", "));
                result.push(json!({ "size": members.len(), "members": members, "summary": summary }));
            }
            result.sort_by(|a, b| b["size"].as_i64().cmp(&a["size"].as_i64()));

            Ok(json!({ "totalCommunities": result.len(), "communities": result }))
        })
    }
}

// ─── Tool: DetectAndResolveConflictsTool ─────────────────────────

pub struct DetectAndResolveConflictsTool;

const EXCLUSIVE_RELATIONS: &[&str] = &["lives_in", "current_job", "spouse", "has_status", "is_born_in", "located_in"];

#[async_trait::async_trait]
impl Tool for DetectAndResolveConflictsTool {
    fn name(&self) -> &str { "detect_and_resolve_conflicts" }

    fn description(&self) -> &str {
        "Detect and resolve contradictions or conflicts in graph relations and semantic memories."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "strategy": { "type": "string", "default": "recency", "enum": ["recency"] },
                "dryRun": { "type": "boolean", "default": true },
                "semanticThreshold": { "type": "number", "default": 0.85 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let strategy = arguments["strategy"].as_str().unwrap_or("recency");
        let dry_run = arguments["dryRun"].as_bool().unwrap_or(true);
        let (uid, sid, aid) = scope_from_args(arguments);

        with_db(|conn| {
            let mut conflicts = Vec::new();
            let mut resolved = 0i64;

            for &rel_type in EXCLUSIVE_RELATIONS {
                // Find entities with multiple current edges of the same exclusive type
                let mut stmt = conn.prepare(
                    "SELECT from_name, COUNT(*) as cnt FROM graph_edges WHERE relation_type = ?1 AND valid_until IS NULL AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*') GROUP BY from_name HAVING cnt > 1"
                )?;
                let rows = stmt.query_map(params![rel_type, uid, sid, aid], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                })?;
                for r in rows {
                    let (entity, count) = r?;
                    let description = format!("Entity '{}' has {} '{}' relations (exclusive type allows 1)", entity, count, rel_type);

                    if !dry_run && strategy == "recency" {
                        // Keep the most recent, expire others
                        conn.execute(
                            "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE rowid NOT IN (SELECT rowid FROM graph_edges WHERE from_name = ?1 AND relation_type = ?2 AND valid_until IS NULL ORDER BY created_at DESC LIMIT 1) AND from_name = ?3 AND relation_type = ?4 AND valid_until IS NULL",
                            params![entity, rel_type, entity, rel_type],
                        )?;
                        resolved += count - 1;
                    }

                    conflicts.push(json!({ "entity": entity, "relationType": rel_type, "count": count, "description": description }));
                }
            }

            Ok(json!({
                "conflictsFound": conflicts.len() as i64,
                "conflicts": conflicts,
                "resolved": if dry_run { 0 } else { resolved },
                "dryRun": dry_run,
                "strategy": strategy,
            }))
        })
    }
}

// ─── Tool: CompactMemoriesTool ───────────────────────────────────

pub struct CompactMemoriesTool;

#[async_trait::async_trait]
impl Tool for CompactMemoriesTool {
    fn name(&self) -> &str { "compact_memories" }

    fn description(&self) -> &str {
        "Compact memories using decay-based archival and cluster consolidation."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "strategy": { "type": "string", "default": "both", "enum": ["decay", "cluster", "both"] },
                "dryRun": { "type": "boolean", "default": false },
                "minImportance": { "type": "number", "default": 0.15 },
                "maxAgeHours": { "type": "number", "default": 24.0 },
                "userId": { "type": "string" },
                "sessionId": { "type": "string" },
                "agentId": { "type": "string" }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let strategy = arguments["strategy"].as_str().unwrap_or("both");
        let dry_run = arguments["dryRun"].as_bool().unwrap_or(false);
        let min_importance = arguments["minImportance"].as_f64().unwrap_or(0.15);
        let max_age_hours = arguments["maxAgeHours"].as_f64().unwrap_or(24.0);
        let (uid, sid, aid) = scope_from_args(arguments);

        let cutoff = chrono::Utc::now() - chrono::Duration::hours(max_age_hours as i64);
        let cutoff_str = cutoff.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let mut archived = 0i64;
        let mut merged = 0i64;

        with_db(|conn| {
            if strategy == "decay" || strategy == "both" {
                // Archive low-importance semantic facts older than max_age
                let rows: i64 = if dry_run {
                    conn.query_row(
                        "SELECT COUNT(*) FROM semantic_metadata WHERE importance < ?1 AND timestamp < ?2 AND valid_until IS NULL AND (?3 IS NULL OR user_id = ?3 OR user_id = '*') AND (?4 IS NULL OR session_id = ?4 OR session_id = '*') AND (?5 IS NULL OR agent_id = ?5 OR agent_id = '*')",
                        params![min_importance, cutoff_str, uid, sid, aid],
                        |r| r.get::<_, i64>(0),
                    )?
                } else {
                    conn.execute(
                        "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE importance < ?1 AND timestamp < ?2 AND valid_until IS NULL AND (?3 IS NULL OR user_id = ?3 OR user_id = '*') AND (?4 IS NULL OR session_id = ?4 OR session_id = '*') AND (?5 IS NULL OR agent_id = ?5 OR agent_id = '*')",
                        params![min_importance, cutoff_str, uid, sid, aid],
                    )? as i64
                };
                archived += rows;
            }

            if strategy == "cluster" || strategy == "both" {
                // Simple consolidation: expire very old episodic logs
                let rows: i64 = if dry_run {
                    conn.query_row(
                        "SELECT COUNT(*) FROM episodic_logs WHERE created_at < ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
                        params![cutoff_str, uid, sid, aid],
                        |r| r.get::<_, i64>(0),
                    )?
                } else {
                    conn.execute(
                        "DELETE FROM episodic_logs WHERE created_at < ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')",
                        params![cutoff_str, uid, sid, aid],
                    )? as i64
                };
                merged += rows;
            }

            Ok(json!({
                "strategy": strategy, "dryRun": dry_run,
                "archivedFacts": archived, "consolidatedLogs": merged,
                "cutoffTimestamp": cutoff_str,
            }))
        })
    }
}

// ─── IndexCodebaseTool ──────────────────────────────────────────

pub struct IndexCodebaseTool;

#[async_trait::async_trait]
impl Tool for IndexCodebaseTool {
    fn name(&self) -> &str {
        "index_codebase"
    }
    fn description(&self) -> &str {
        "Index functions, structs, enums and types in the codebase by scanning source files. Stores results for query_code_graph and analyze_code_impact."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to scan (defaults to current directory '.')"
                },
                "user_id": { "type": "string", "description": "Optional scope identifier" },
                "session_id": { "type": "string", "description": "Optional scope identifier" },
                "agent_id": { "type": "string", "description": "Optional scope identifier" }
            }
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let scan_path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or(".").to_string();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let start = std::time::Instant::now();
        let count = with_db(|conn| {
            // Clear existing data for this scope first
            conn.execute(
                "DELETE FROM code_calls WHERE caller_id IN (SELECT element_id FROM code_elements WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*'))",
                params![user_id, session_id, agent_id],
            )?;
            conn.execute(
                "DELETE FROM code_elements WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*')",
                params![user_id, session_id, agent_id],
            )?;
            scan_and_index(Path::new(&scan_path), &user_id, &session_id, &agent_id, conn)
        })?;

        Ok(json!({
            "status": format!("Indexed {} source files in {:?}", count, start.elapsed()),
            "path": scan_path,
            "filesIndexed": count,
        }))
    }
}

fn scan_and_index(dir: &Path, user_id: &str, session_id: &str, agent_id: &str, conn: &Connection) -> Result<i64> {
    let mut count = 0;
    if !dir.is_dir() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name != "target" && name != ".git" && name != "external" && name != "node_modules" && name != ".venv" {
                count += scan_and_index(&path, user_id, session_id, agent_id, conn)?;
            }
        } else {
            let ext = path.extension().unwrap_or_default().to_string_lossy().to_string();
            if matches!(ext.as_str(), "rs" | "py" | "js" | "jsx" | "ts" | "tsx" | "go" | "rb" | "java" | "swift" | "kt" | "c" | "h" | "cpp" | "hpp") {
                if let Ok(_) = index_file(&path, user_id, session_id, agent_id, conn) {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

fn index_file(path: &Path, user_id: &str, session_id: &str, agent_id: &str, conn: &Connection) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    let relative_path = path.to_string_lossy().to_string();
    let lines: Vec<&str> = content.lines().collect();
    let mut elements: Vec<(String, String, String, String, i64, i64)> = Vec::new(); // id, type, name, signature, start, end

    // Patterns for function/struct/enum/class definitions
    let fn_re = Regex::new(r"^\s*(public\s+|pub\s+)?(async\s+)?fn\s+([a-zA-Z_]\w*)").unwrap();
    let struct_re = Regex::new(r"^\s*(public\s+|pub\s+)?struct\s+([a-zA-Z_]\w*)").unwrap();
    let enum_re = Regex::new(r"^\s*(public\s+|pub\s+)?enum\s+([a-zA-Z_]\w*)").unwrap();
    let impl_re = Regex::new(r"^\s*(public\s+|pub\s+)?impl\s+([a-zA-Z_]\w*)").unwrap();
    let def_re = Regex::new(r"^\s*def\s+([a-zA-Z_]\w*)").unwrap();
    let class_re = Regex::new(r"^\s*class\s+([a-zA-Z_]\w*)").unwrap();
    let func_re = Regex::new(r"^\s*func\s+([a-zA-Z_]\w*)").unwrap();
    let type_re = Regex::new(r"^\s*type\s+([a-zA-Z_]\w*)").unwrap();
    let trait_re = Regex::new(r"^\s*(public\s+|pub\s+)?trait\s+([a-zA-Z_]\w*)").unwrap();
    let interface_re = Regex::new(r"^\s*interface\s+([a-zA-Z_]\w*)").unwrap();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as i64;
        let trimmed = line.trim();
        let mut element_type: Option<&str> = None;
        let mut name: Option<String> = None;
        let mut signature = String::new();

        if let Some(caps) = fn_re.captures(trimmed) {
            element_type = Some("Function");
            name = caps.get(3).or(caps.get(2)).map(|m| m.as_str().to_string());
            signature = caps.get(0).map(|m| m.as_str().to_string() + "(...)").unwrap_or_default();
        } else if let Some(caps) = def_re.captures(trimmed) {
            element_type = Some("Function");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps.get(0).map(|m| m.as_str().to_string() + "(...)").unwrap_or_default();
        } else if let Some(caps) = func_re.captures(trimmed) {
            element_type = Some("Function");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps.get(0).map(|m| m.as_str().to_string() + "(...)").unwrap_or_default();
        } else if let Some(caps) = struct_re.captures(trimmed) {
            element_type = Some("Struct");
            name = caps.get(2).map(|m| m.as_str().to_string());
            signature = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
        } else if let Some(caps) = enum_re.captures(trimmed) {
            element_type = Some("Enum");
            name = caps.get(2).map(|m| m.as_str().to_string());
            signature = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
        } else if let Some(caps) = impl_re.captures(trimmed) {
            element_type = Some("ImplBlock");
            name = caps.get(2).map(|m| format!("impl_{}", m.as_str()));
            signature = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
        } else if let Some(caps) = class_re.captures(trimmed) {
            element_type = Some("Class");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
        } else if let Some(caps) = trait_re.captures(trimmed) {
            element_type = Some("Trait");
            name = caps.get(2).map(|m| m.as_str().to_string());
            signature = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
        } else if let Some(caps) = interface_re.captures(trimmed) {
            element_type = Some("Interface");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
        } else if let Some(caps) = type_re.captures(trimmed) {
            element_type = Some("TypeAlias");
            name = caps.get(1).map(|m| m.as_str().to_string());
            signature = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
        }

        if let (Some(el_type), Some(el_name)) = (element_type, name) {
            let el_id = format!("{}:{}:{}", relative_path, el_name, line_num);
            let end_line = (lines.len() as i64).min(line_num + 10);
            conn.execute(
                "INSERT OR IGNORE INTO code_elements (element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line, user_id, session_id, agent_id) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7, ?8, ?9, ?10)",
                params![el_id, relative_path, el_type, el_name, signature, line_num, end_line, user_id, session_id, agent_id],
            )?;
            elements.push((el_id, el_name, relative_path.clone(), el_type.to_string(), line_num, end_line));
        }
    }

    // Call detection: find `name(` patterns that match known element names
    let call_re = Regex::new(r"([a-zA-Z_]\w*)\s*\(").unwrap();
    let known_names: HashSet<String> = elements.iter().map(|e| e.1.clone()).collect();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Skip definition lines themselves
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") || trimmed.starts_with("def ") || trimmed.starts_with("func ") {
            continue;
        }
        for cap in call_re.captures_iter(trimmed) {
            let callee_name = cap.get(1).unwrap().as_str().to_string();
            // Skip keywords
            if matches!(callee_name.as_str(), "if" | "for" | "while" | "match" | "return" | "let" | "mut" | "Some" | "None" | "Ok" | "Err" | "self" | "Self" | "super" | "crate") {
                continue;
            }
            if known_names.contains(&callee_name) {
                // Find the callee element_id
                if let Some(callee) = elements.iter().find(|e| e.1 == callee_name) {
                    // Find nearest caller (the enclosing function/element on this line)
                    let line_num = (idx + 1) as i64;
                    if let Some(caller) = elements.iter().filter(|e| e.4 <= line_num && line_num <= e.5).last() {
                        conn.execute(
                            "INSERT OR IGNORE INTO code_calls (caller_id, callee_id, call_site) VALUES (?1, ?2, ?3)",
                            params![caller.0.clone(), callee.0.clone(), format!("{}:{}", relative_path, line_num)],
                        )?;
                    }
                }
            }
        }
    }

    Ok(())
}

// ─── QueryCodeGraphTool ─────────────────────────────────────────

pub struct QueryCodeGraphTool;

#[async_trait::async_trait]
impl Tool for QueryCodeGraphTool {
    fn name(&self) -> &str {
        "query_code_graph"
    }
    fn description(&self) -> &str {
        "Query structural elements (structs, functions, impls) and calling patterns indexed in the codebase"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Filter by file path (substring match)"
                },
                "query": {
                    "type": "string",
                    "description": "Search by name or element type (e.g. 'Struct', 'Function')"
                },
                "user_id": { "type": "string", "description": "Optional scope identifier" },
                "session_id": { "type": "string", "description": "Optional scope identifier" },
                "agent_id": { "type": "string", "description": "Optional scope identifier" }
            }
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments.get("file_path").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let query = arguments.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        let results = with_db(|conn| {
            let mut conditions = vec![
                "(user_id = ?1 OR user_id = '*')".to_string(),
                "(session_id = ?2 OR session_id = '*')".to_string(),
                "(agent_id = ?3 OR agent_id = '*')".to_string(),
            ];
            let mut param_values: Vec<String> = vec![user_id, session_id, agent_id];

            if !file_path.is_empty() {
                conditions.push(format!("file_path LIKE ?{}", param_values.len() + 1));
                param_values.push(format!("%{}%", file_path));
            }
            if !query.is_empty() {
                conditions.push(format!("(name LIKE ?{} OR element_type LIKE ?{})", param_values.len() + 1, param_values.len() + 1));
                param_values.push(format!("%{}%", query));
            }

            let sql = format!(
                "SELECT element_id, file_path, element_type, name, signature, start_line, end_line FROM code_elements WHERE {} ORDER BY file_path, start_line LIMIT 200",
                conditions.join(" AND ")
            );

            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(rusqlite::params_from_iter(param_values.iter().map(|s| s.as_str())))?;
            let mut items = Vec::new();
            while let Some(row) = rows.next()? {
                items.push(json!({
                    "id": row.get::<_, String>(0)?,
                    "filePath": row.get::<_, String>(1)?,
                    "elementType": row.get::<_, String>(2)?,
                    "name": row.get::<_, String>(3)?,
                    "signature": row.get::<_, String>(4)?,
                    "startLine": row.get::<_, i64>(5)?,
                    "endLine": row.get::<_, i64>(6)?,
                }));
            }
            Ok(json!(items))
        })?;

        Ok(results)
    }
}

// ─── AnalyzeCodeImpactTool ──────────────────────────────────────

pub struct AnalyzeCodeImpactTool;

#[async_trait::async_trait]
impl Tool for AnalyzeCodeImpactTool {
    fn name(&self) -> &str {
        "analyze_code_impact"
    }
    fn description(&self) -> &str {
        "Calculate downstream callers and change risk for a code symbol"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target_symbol": {
                    "type": "string",
                    "description": "Element ID or name of the symbol to analyze (e.g. 'my_function')"
                },
                "user_id": { "type": "string", "description": "Optional scope identifier" },
                "session_id": { "type": "string", "description": "Optional scope identifier" },
                "agent_id": { "type": "string", "description": "Optional scope identifier" }
            }
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let target_symbol = arguments.get("target_symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if target_symbol.is_empty() {
            return Ok(json!({"error": "target_symbol is required"}));
        }
        let (user_id, session_id, agent_id) = scope_from_args(arguments);

        with_db(|conn| {
            // Find the element - try direct ID match first, then name match
            let element = conn.query_row(
                "SELECT element_id, name, element_type, file_path FROM code_elements WHERE (element_id = ?1 OR name = ?1) AND (user_id = ?2 OR user_id = '*') AND (session_id = ?3 OR session_id = '*') AND (agent_id = ?4 OR agent_id = '*') LIMIT 1",
                params![target_symbol, user_id, session_id, agent_id],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                )),
            ).map_err(|_| anyhow!("Symbol '{}' not found in indexed codebase. Run index_codebase first.", target_symbol))?;

            let (element_id, element_name, element_type, file_path) = element;

            // Build reverse call graph: for each callee, collect all callers
            let mut stmt = conn.prepare(
                "SELECT caller_id, callee_id FROM code_calls WHERE caller_id IN (SELECT element_id FROM code_elements WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*'))"
            )?;
            let rows = stmt.query_map(params![user_id, session_id, agent_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;

            // Build adjacency list (callee -> callers)
            let mut reverse_graph: HashMap<String, Vec<String>> = HashMap::new();
            for r in rows {
                let (caller, callee) = r?;
                reverse_graph.entry(callee).or_default().push(caller);
            }

            // BFS from target symbol through reverse graph
            let mut visited: HashSet<String> = HashSet::new();
            let mut queue: Vec<(String, u32)> = Vec::new();
            let mut affected: Vec<String> = Vec::new();
            let mut max_depth: u32 = 0;

            queue.push((element_id.clone(), 0));
            visited.insert(element_id.clone());

            while let Some((current, depth)) = queue.pop() {
                if depth > 0 {
                    affected.push(current.clone());
                    if depth > max_depth {
                        max_depth = depth;
                    }
                }
                if let Some(callers) = reverse_graph.get(&current) {
                    for caller in callers {
                        if !visited.contains(caller) {
                            visited.insert(caller.clone());
                            queue.push((caller.clone(), depth + 1));
                        }
                    }
                }
            }

            // Risk heuristic
            let direct_callers = reverse_graph.get(&element_id).map(|v| v.len()).unwrap_or(0);
            let transitive_callers = affected.len().saturating_sub(direct_callers);
            let raw_score = 0.1 * (direct_callers as f64) + 0.05 * (transitive_callers as f64) + 0.1 * (max_depth as f64);
            let risk_score = raw_score.min(1.0);

            let details = format!(
                "Symbol '{}' ({}) in {} has {} direct callers and {} transitive callers. Maximum propagation depth: {}.",
                element_name, element_type, file_path, direct_callers, transitive_callers, max_depth
            );

            Ok(json!({
                "targetSymbol": element_name,
                "elementType": element_type,
                "filePath": file_path,
                "affectedSymbols": affected,
                "maxDepth": max_depth,
                "riskScore": (risk_score * 100.0).round() / 100.0,
                "details": details,
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::graph_memory::test_lock;

    #[tokio::test]
    async fn test_set_get_working_memory() {
        let _l = test_lock().lock().await;
        let scope = format!("test_wm_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        let set_tool = SetWorkingMemoryTool;
        let res = set_tool.call(&json!({
            "key": "test_key", "value": "test_value", "ttl": 60, "sessionId": scope
        })).await.unwrap();
        assert!(res["status"].as_str().unwrap().contains("test_key"));

        let get_tool = GetWorkingMemoryTool;
        let res2 = get_tool.call(&json!({
            "key": "test_key", "sessionId": scope
        })).await.unwrap();
        assert_eq!(res2["value"], "test_value");
    }

    #[tokio::test]
    async fn test_get_working_memory_expired() {
        let _l = test_lock().lock().await;
        let scope = format!("test_wm_exp_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        let set_tool = SetWorkingMemoryTool;
        set_tool.call(&json!({
            "key": "exp_key", "value": "exp_value", "ttl": 0, "sessionId": scope
        })).await.unwrap();

        // Wait a tiny bit to ensure expiration
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let get_tool = GetWorkingMemoryTool;
        let res = get_tool.call(&json!({
            "key": "exp_key", "sessionId": scope
        })).await.unwrap();
        assert!(res["status"].as_str().unwrap().contains("expired"));
    }

    #[tokio::test]
    async fn test_log_and_retrieve_reflections() {
        let scope = format!("test_ref_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        let log_tool = LogReflectionTool;
        log_tool.call(&json!({
            "taskDescription": "Test task",
            "status": "Success",
            "attemptNumber": 1,
            "stepsTaken": "Step 1",
            "reflection": "It worked",
            "sessionId": scope
        })).await.unwrap();

        let retrieve_tool = RetrieveEpisodicReflectionsTool;
        let res = retrieve_tool.call(&json!({
            "query": "Test task",
            "sessionId": scope
        })).await.unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
        assert_eq!(arr[0]["taskDescription"], "Test task");
    }

    #[tokio::test]
    async fn test_log_execution_episode() {
        let scope = format!("test_ep_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        let tool = LogExecutionEpisodeTool;
        let res = tool.call(&json!({
            "taskDescription": "Test episode",
            "executionStatus": "Completed",
            "stepsTaken": "Did something",
            "sessionId": scope
        })).await.unwrap();
        assert_eq!(res["status"], "Episode logged successfully");
    }

    #[tokio::test]
    async fn test_store_retrieve_shared_memory() {
        let scope = format!("test_shr_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        let store = StoreSharedTeamMemoryTool;
        store.call(&json!({
            "key": "shared_key",
            "value": "shared_value",
            "sourceAgent": "agent_a",
            "targetAgents": ["agent_b"],
            "sessionId": scope
        })).await.unwrap();

        let retrieve = RetrieveSharedTeamMemoryTool;
        let res = retrieve.call(&json!({
            "agentId": "agent_b",
            "sessionId": scope
        })).await.unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
        assert_eq!(arr[0]["key"], "shared_key");
    }

    #[tokio::test]
    async fn test_record_query_tool_performance() {
        let scope = format!("test_perf_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        let record = RecordToolPerformanceTool;
        record.call(&json!({
            "toolName": "test_tool",
            "modelName": "test_model",
            "taskType": "coding",
            "successCount": 5,
            "failureCount": 1,
            "averageLatency": 0.5,
            "sessionId": scope
        })).await.unwrap();

        let query = QueryToolPerformanceTool;
        let res = query.call(&json!({
            "taskType": "coding",
            "sessionId": scope
        })).await.unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
        assert_eq!(arr[0]["toolName"], "test_tool");
    }

    #[tokio::test]
    async fn test_text_similarity() {
        assert!((text_similarity("hello world", "hello world") - 1.0).abs() < 0.01);
        assert!((text_similarity("hello world", "hello there") - 0.333).abs() < 0.01);
        assert_eq!(text_similarity("hello", "world"), 0.0);
    }

    #[tokio::test]
    async fn test_extract_facts() {
        let facts = extract_facts("Alice uses Rust. Bob created Python.");
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].from, "Alice");
        assert_eq!(facts[0].to, "Rust");
        assert_eq!(facts[0].relation, "uses");
    }

    #[tokio::test]
    async fn test_promote_working_memory() {
        let _l = test_lock().lock().await;
        let scope = format!("test_prom_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        let set_tool = SetWorkingMemoryTool;
        set_tool.call(&json!({
            "key": "prom_key", "value": "prom_value", "ttl": 60, "sessionId": scope
        })).await.unwrap();

        let promote = PromoteWorkingMemoryTool;
        let res = promote.call(&json!({
            "key": "prom_key", "sessionId": scope
        })).await.unwrap();
        assert!(res["status"].as_str().unwrap().contains("Promoted"));

        // Should be gone from working memory
        let get_tool = GetWorkingMemoryTool;
        let res2 = get_tool.call(&json!({
            "key": "prom_key", "sessionId": scope
        })).await.unwrap();
        assert!(res2["status"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_static_semantic_fact_store() {
        let _l = test_lock().lock().await;
        let scope = format!("test_sf_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let (uid, sid, aid) = ("*", &scope, "*");

        store_semantic_fact("test-fact-1", "Rust is a systems language.", 0.8, uid, sid, aid).unwrap();

        let results = with_db(|conn| query_fts5(conn, "systems language", 10, uid, sid, aid)).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0]["nodeId"], "test-fact-1");
    }

    #[tokio::test]
    async fn test_search_text_fts5() {
        let _l = test_lock().lock().await;
        let scope = format!("test_fts_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let (uid, sid, aid) = ("*", &scope, "*");

        store_semantic_fact("fts-fact-1", "MCP defines a standard protocol for context-aware AI tools.", 0.9, uid, sid, aid).unwrap();
        store_semantic_fact("fts-fact-2", "SQLite is a self-contained SQL database engine.", 0.7, uid, sid, aid).unwrap();

        let results = with_db(|conn| query_fts5(conn, "context-aware", 10, uid, sid, aid)).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["nodeId"], "fts-fact-1");
    }

    #[tokio::test]
    async fn test_query_fact_history() {
        let _l = test_lock().lock().await;
        let scope = format!("test_qfh_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        // Insert test entities and relations in proper tables
        with_db(|conn| {
            conn.execute("INSERT OR IGNORE INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id)
                          VALUES ('EntityA', 'Test', '[]', '*', ?1, '*')", params![scope]).ok();
            conn.execute("INSERT OR IGNORE INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id)
                          VALUES ('EntityB', 'Test', '[]', '*', ?1, '*')", params![scope]).ok();
            conn.execute("INSERT INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id)
                          VALUES ('EntityA', 'EntityB', 'knows', '*', ?1, '*')", params![scope]).ok();
            Ok(())
        }).unwrap();

        let tool = QueryFactHistoryTool;
        let res = tool.call(&json!({
            "entityName": "EntityA",
            "sessionId": scope
        })).await.unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
        assert_eq!(arr[0]["from"], "EntityA");
    }

    #[tokio::test]
    async fn test_invalidate_semantic_fact() {
        let _l = test_lock().lock().await;
        let scope = format!("test_inv_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let (uid, sid, aid) = ("*", &scope, "*");

        store_semantic_fact("inv-fact-1", "Temporary data.", 0.5, uid, sid, aid).unwrap();

        let tool = InvalidateFactTool;
        let res = tool.call(&json!({
            "factId": "inv-fact-1",
            "sessionId": scope
        })).await.unwrap();
        assert!(res["status"].as_str().unwrap().contains("invalidated"));
    }

    #[tokio::test]
    async fn test_compress_context() {
        let tool = CompressContextTool;
        let res = tool.call(&json!({
            "text": "This is the first important sentence. This is the second one. Third sentence is here. Fourth and final one.",
            "ratio": 0.5
        })).await.unwrap();
        assert!(res["compressedLength"].as_u64().unwrap() > 0);
        assert!(res["originalLength"].as_u64().unwrap() > res["compressedLength"].as_u64().unwrap());
    }

    #[tokio::test]
    async fn test_proactive_recall() {
        let scope = format!("test_pr_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let (uid, sid, aid) = ("*", &scope, "*");

        store_semantic_fact("pr-fact-1", "Rust compiler optimizations improve performance.", 0.9, uid, sid, aid).unwrap();

        let tool = ProactiveRecallTool;
        let res = tool.call(&json!({
            "query": "rust compiler",
            "sessionId": scope
        })).await.unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
    }

    #[tokio::test]
    async fn test_smart_store_text() {
        let scope = format!("test_sst_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        let tool = SmartStoreTool;
        let res = tool.call(&json!({
            "text": "Smart store test fact.",
            "sessionId": scope
        })).await.unwrap();
        assert_eq!(res["action"], "add");
    }

    #[tokio::test]
    async fn test_evict_expired_working_memory() {
        let _l = test_lock().lock().await;
        let scope = format!("test_ev_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        let set_tool = SetWorkingMemoryTool;
        set_tool.call(&json!({
            "key": "evict_key", "value": "evict_value", "ttl": 0, "sessionId": scope
        })).await.unwrap();

        // Wait a moment for expiration
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let evict = EvictExpiredWorkingMemoryTool;
        let res = evict.call(&json!({ "sessionId": scope })).await.unwrap();
        assert!(res["status"].as_str().unwrap().contains("Evicted"));
    }
}
