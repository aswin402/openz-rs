use super::cache::get_cache_connection;
use super::{estimate_tokens, CACHE_CAPACITY, MAX_INPUT_SIZE};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

// ═══════════════════════════════════════════════════════════════════
// Tool 4: PingTool
// ═══════════════════════════════════════════════════════════════════

pub struct PingTool;

#[async_trait::async_trait]
impl Tool for PingTool {
    fn name(&self) -> &str {
        "ping"
    }
    fn description(&self) -> &str {
        "Health check. Returns 'ok' if the tool is responsive."
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        Ok(json!({ "status": "ok" }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 5: ServerInfoTool
// ═══════════════════════════════════════════════════════════════════

pub struct ServerInfoTool;

#[async_trait::async_trait]
impl Tool for ServerInfoTool {
    fn name(&self) -> &str {
        "server_info"
    }
    fn description(&self) -> &str {
        "Returns information about the Headroom MCP server configuration and status."
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let count = get_cache_connection()
            .map(|conn| {
                conn.query_row("SELECT COUNT(*) FROM cache_entries", [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap_or(0)
            })
            .unwrap_or(0);
        let total_bytes = get_cache_connection()
            .map(|conn| {
                conn.query_row(
                    "SELECT COALESCE(SUM(size_bytes), 0) FROM cache_entries",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0)
            })
            .unwrap_or(0);
        Ok(json!({
            "version": "0.1.0",
            "cache_size": count,
            "total_bytes": total_bytes,
            "max_input_size": MAX_INPUT_SIZE,
            "cache_capacity": CACHE_CAPACITY,
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 6: CountTokensTool
// ═══════════════════════════════════════════════════════════════════

pub struct CountTokensTool;

#[async_trait::async_trait]
impl Tool for CountTokensTool {
    fn name(&self) -> &str {
        "count_tokens"
    }
    fn description(&self) -> &str {
        "Estimates the token count for a given text. Helps agents decide whether compression is needed."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The text to estimate tokens for." }
            },
            "required": ["text"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let text = arguments["text"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing text parameter"))?;
        let tokens = estimate_tokens(text);
        let chars = text.chars().count();
        Ok(
            json!({ "tokens": tokens, "characters": chars, "estimate": format!("~{} tokens ({} characters)", tokens, chars) }),
        )
    }
}
