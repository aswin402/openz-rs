use crate::tools::graph_memory::{scope_from_args, with_db};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::params;
use serde_json::{json, Value};
use std::collections::HashMap;
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
        
        // Eviction limit to prevent unbounded in-memory growth
        if !map.contains_key(&scoped) && map.len() >= 1000 {
            let oldest_key = map.iter()
                .min_by_key(|&(_, entry)| entry.created_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest_key {
                map.remove(&k);
            }
        }

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

pub(crate) fn store_semantic_fact(node_id: &str, text: &str, importance: f64, user_id: &str, session_id: &str, agent_id: &str) -> Result<()> {
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
