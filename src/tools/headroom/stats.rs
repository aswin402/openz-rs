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

// ═══════════════════════════════════════════════════════════════════
// Tool 20: HeadroomStatsTool
// ═══════════════════════════════════════════════════════════════════

pub struct HeadroomStatsTool;

#[async_trait::async_trait]
impl Tool for HeadroomStatsTool {
    fn name(&self) -> &str {
        "headroom_stats"
    }
    fn description(&self) -> &str {
        "Returns Headroom compression history totals: bytes, tokens, cache entries, and DB size."
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let conn = get_cache_connection()?;
        let (total_compressions, total_original_bytes, total_compressed_bytes, total_original_tokens, total_compressed_tokens): (i64, i64, i64, i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(original_size), 0), COALESCE(SUM(compressed_size), 0), COALESCE(SUM(original_tokens), 0), COALESCE(SUM(compressed_tokens), 0) FROM compression_log",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap_or((0, 0, 0, 0, 0));
        let cache_entries: i64 = conn
            .query_row("SELECT COUNT(*) FROM cache_entries", [], |row| row.get(0))
            .unwrap_or(0);
        let db_size_bytes: i64 = conn
            .query_row(
                "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let saved_bytes = total_original_bytes.saturating_sub(total_compressed_bytes);
        let saved_tokens = total_original_tokens.saturating_sub(total_compressed_tokens);
        let saving_pct = if total_original_tokens > 0 {
            (saved_tokens as f64 / total_original_tokens as f64) * 100.0
        } else {
            0.0
        };
        Ok(json!({
            "total_compressions": total_compressions,
            "total_original_bytes": total_original_bytes,
            "total_compressed_bytes": total_compressed_bytes,
            "total_saved_bytes": saved_bytes,
            "total_original_tokens": total_original_tokens,
            "total_compressed_tokens": total_compressed_tokens,
            "total_saved_tokens": saved_tokens,
            "saving_pct": saving_pct,
            "cache_entries": cache_entries,
            "db_size_bytes": db_size_bytes,
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 21: HeadroomUsageTool
// ═══════════════════════════════════════════════════════════════════

pub struct HeadroomUsageTool;

#[async_trait::async_trait]
impl Tool for HeadroomUsageTool {
    fn name(&self) -> &str {
        "headroom_usage"
    }
    fn description(&self) -> &str {
        "Groups Headroom compression token savings by model_hint for cost and context-budget analysis."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "model": { "type": "string", "description": "Optional exact model_hint filter." }
            }
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let model_filter = arguments.get("model").and_then(|v| v.as_str());
        let conn = get_cache_connection()?;
        let mut rows_json = Vec::new();
        if let Some(model) = model_filter {
            let mut stmt = conn.prepare(
                "SELECT COALESCE(NULLIF(model_hint, ''), 'default') AS model, COALESCE(SUM(original_tokens), 0), COALESCE(SUM(original_tokens - compressed_tokens), 0) FROM compression_log WHERE model_hint = ?1 GROUP BY model ORDER BY SUM(original_tokens - compressed_tokens) DESC",
            )?;
            let rows = stmt.query_map([model], usage_row_from_sql)?;
            for row in rows {
                rows_json.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT COALESCE(NULLIF(model_hint, ''), 'default') AS model, COALESCE(SUM(original_tokens), 0), COALESCE(SUM(original_tokens - compressed_tokens), 0) FROM compression_log GROUP BY model ORDER BY SUM(original_tokens - compressed_tokens) DESC",
            )?;
            let rows = stmt.query_map([], usage_row_from_sql)?;
            for row in rows {
                rows_json.push(row?);
            }
        }
        Ok(json!({ "rows": rows_json, "count": rows_json.len() }))
    }
}

fn usage_row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let model: String = row.get(0)?;
    let total_original_tokens: i64 = row.get(1)?;
    let total_saved_tokens: i64 = row.get(2)?;
    let saving_pct = if total_original_tokens > 0 {
        (total_saved_tokens as f64 / total_original_tokens as f64) * 100.0
    } else {
        0.0
    };
    Ok(json!({
        "model": model,
        "total_original_tokens": total_original_tokens.max(0) as u64,
        "total_saved_tokens": total_saved_tokens.max(0) as u64,
        "saving_pct": saving_pct,
        "estimated_usd_saved": 0.0,
    }))
}
