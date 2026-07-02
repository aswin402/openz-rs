use crate::tools::graph_memory::{scope_from_args, with_db};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::params;
use serde_json::{json, Value};

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
