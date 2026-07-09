use super::CACHE_CAPACITY;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::SystemTime;

// ─── Constants & Counter ─────────────────────────────────────────

static CCR_COUNTER: AtomicU64 = AtomicU64::new(0);

// ─── DB path & connection ───────────────────────────────────────

fn get_db_path() -> PathBuf {
    crate::config::loader::runtime_db_path("ccr_cache.db")
}

pub fn get_cache_connection() -> Result<std::sync::MutexGuard<'static, Connection>> {
    static DB: OnceLock<std::sync::Mutex<Connection>> = OnceLock::new();
    if let Some(mtx) = DB.get() {
        return mtx.lock().map_err(|e| anyhow!("Cache lock error: {}", e));
    }
    let path = get_db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path).unwrap_or_else(|_| Connection::open_in_memory().unwrap());
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
         CREATE TABLE IF NOT EXISTS cache_entries (
             ccr_id TEXT PRIMARY KEY,
             content TEXT NOT NULL,
             created_at TEXT NOT NULL,
             accessed_at TEXT NOT NULL,
             size_bytes INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS compression_log (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             tool_name TEXT NOT NULL,
             original_size INTEGER NOT NULL,
             compressed_size INTEGER NOT NULL,
             original_tokens INTEGER NOT NULL,
             compressed_tokens INTEGER NOT NULL,
             content_type TEXT NOT NULL,
             model_hint TEXT,
             created_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_cache_accessed ON cache_entries(accessed_at);",
    )?;
    let mtx = DB.get_or_init(|| std::sync::Mutex::new(conn));
    mtx.lock().map_err(|e| anyhow!("Cache lock error: {}", e))
}

// ─── Helper Functions ───────────────────────────────────────────

pub fn generate_ccr_id() -> String {
    let time_ns = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seq = CCR_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("ccr_{:x}_{:x}", time_ns & 0xFFFFFFFF, seq)
}

pub fn evict_lru_if_needed() {
    if let Ok(conn) = get_cache_connection() {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM cache_entries", [], |r| r.get(0))
            .unwrap_or(0);
        if count > CACHE_CAPACITY as i64 {
            let _ = conn.execute(
                "DELETE FROM cache_entries WHERE ccr_id IN (
                    SELECT ccr_id FROM cache_entries ORDER BY accessed_at ASC LIMIT ?
                )",
                params![count - CACHE_CAPACITY as i64],
            );
        }
    }
}

pub fn cache_content(content: &str) -> Result<String> {
    let id = generate_ccr_id();
    let now = chrono::Utc::now().to_rfc3339();
    {
        let conn = get_cache_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO cache_entries (ccr_id, content, created_at, accessed_at, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, content, now, now, content.len() as i64],
        )?;
    }
    evict_lru_if_needed();
    Ok(id)
}

// ═══════════════════════════════════════════════════════════════════
// Tool 7: CacheStatsTool
// ═══════════════════════════════════════════════════════════════════

pub struct CacheStatsTool;

#[async_trait::async_trait]
impl Tool for CacheStatsTool {
    fn name(&self) -> &str {
        "cache_stats"
    }
    fn description(&self) -> &str {
        "Returns statistics about the context cache."
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let conn = get_cache_connection()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM cache_entries", [], |r| r.get(0))
            .unwrap_or(0);
        let total_bytes: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(size_bytes), 0) FROM cache_entries",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let mut stmt = conn.prepare(
            "SELECT ccr_id, size_bytes FROM cache_entries ORDER BY accessed_at DESC LIMIT 50",
        )?;
        let items: Vec<Value> = stmt
            .query_map([], |row| {
                Ok(json!({
                    "ccr_id": row.get::<_, String>(0)?,
                    "size_bytes": row.get::<_, i64>(1)?,
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(json!({
            "total_items": count,
            "total_bytes": total_bytes,
            "items": items,
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 8: ClearCacheTool
// ═══════════════════════════════════════════════════════════════════

pub struct ClearCacheTool;

#[async_trait::async_trait]
impl Tool for ClearCacheTool {
    fn name(&self) -> &str {
        "clear_cache"
    }
    fn description(&self) -> &str {
        "Clears all cached context entries to free memory."
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let conn = get_cache_connection()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM cache_entries", [], |r| r.get(0))
            .unwrap_or(0);
        let total_bytes: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(size_bytes), 0) FROM cache_entries",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        conn.execute("DELETE FROM cache_entries", [])?;
        Ok(json!({
            "evicted": count,
            "freed_bytes": total_bytes,
            "message": format!("Successfully cleared cache. Evicted {} items (freed {} bytes).", count, total_bytes),
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 9: SearchCacheTool
// ═══════════════════════════════════════════════════════════════════

pub struct SearchCacheTool;

#[async_trait::async_trait]
impl Tool for SearchCacheTool {
    fn name(&self) -> &str {
        "search_cache"
    }
    fn description(&self) -> &str {
        "Searches cached content by keyword. Returns matching CCR IDs and content snippets."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Keyword to search for in cached content." },
                "max_results": { "type": "integer", "description": "Maximum number of results (default 10)." }
            },
            "required": ["query"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments["query"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing query parameter"))?;
        let max_results = arguments["max_results"].as_u64().unwrap_or(10) as usize;
        let conn = get_cache_connection()?;

        let search_pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        let mut stmt = conn.prepare(
            "SELECT ccr_id, content FROM cache_entries WHERE content LIKE ?1 ESCAPE '\\' ORDER BY accessed_at DESC LIMIT ?2"
        )?;

        let results: Vec<Value> = stmt
            .query_map(params![search_pattern, max_results as i64], |row| {
                let id: String = row.get(0)?;
                let content: String = row.get(1)?;
                let snippet = if let Some(idx) = content.to_lowercase().find(&query.to_lowercase())
                {
                    let start = idx.saturating_sub(30);
                    let end = (idx + query.len() + 50).min(content.len());
                    let sub = &content[start..end];
                    format!("...{}...", sub.replace('\n', " "))
                } else {
                    content.chars().take(80).collect::<String>()
                };
                Ok(json!({ "ccr_id": id, "snippet": snippet }))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(json!({
            "query": query,
            "count": results.len(),
            "results": results,
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 10: CacheAlignTool
// ═══════════════════════════════════════════════════════════════════

pub struct CacheAlignTool;

#[async_trait::async_trait]
impl Tool for CacheAlignTool {
    fn name(&self) -> &str {
        "cache_align"
    }
    fn description(&self) -> &str {
        "Aligns context chunks deterministically, padding and wrapping them to optimize KV cache hits for LLM providers."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "chunks": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of text chunks to align."
                },
                "padding_size": {
                    "type": "integer",
                    "description": "Alignment modulus in bytes (default 1024)."
                }
            },
            "required": ["chunks"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let chunks: Vec<String> = serde_json::from_value(arguments["chunks"].clone())
            .map_err(|_| anyhow!("Invalid chunks: expected array of strings"))?;
        let size = arguments["padding_size"].as_u64().unwrap_or(1024) as usize;
        if size == 0 {
            return Err(anyhow!("Padding size must be greater than 0"));
        }

        let mut sorted_chunks = chunks;
        let total_chunks = sorted_chunks.len();
        sorted_chunks.sort();

        let mut aligned_output = String::new();
        for chunk in sorted_chunks {
            let trimmed = chunk.trim_end();
            let mut hasher = DefaultHasher::new();
            trimmed.hash(&mut hasher);
            let hash = format!("{:016x}", hasher.finish());

            let len = trimmed.len();
            let rem = len % size;
            let pad = if rem == 0 { 0 } else { size - rem };
            let padded = format!("{}{}", trimmed, " ".repeat(pad));

            aligned_output.push_str(&format!(
                "<!-- chunk: {} -->\n{}\n<!-- endchunk -->\n",
                hash, padded
            ));
        }

        Ok(json!({ "aligned": aligned_output, "chunks": total_chunks, "padding_size": size }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 14: ExportCacheTool
// ═══════════════════════════════════════════════════════════════════

pub struct ExportCacheTool;

#[async_trait::async_trait]
impl Tool for ExportCacheTool {
    fn name(&self) -> &str {
        "export_cache"
    }
    fn description(&self) -> &str {
        "Exports the entire cache to a JSON file for session portability."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "Path for the export JSON file." }
            },
            "required": ["file_path"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let path_str = arguments["file_path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing file_path parameter"))?;
        let path = std::path::Path::new(path_str);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| anyhow!("{}", e))?
                .join(path)
        };

        let conn = get_cache_connection()?;
        let mut stmt = conn.prepare("SELECT ccr_id, content, created_at FROM cache_entries")?;
        let entries: Vec<Value> = stmt
            .query_map([], |row| {
                Ok(json!({
                    "ccr_id": row.get::<_, String>(0)?,
                    "content": row.get::<_, String>(1)?,
                    "created_at": row.get::<_, String>(2)?,
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let json_str = serde_json::to_string_pretty(&entries)?;
        std::fs::write(&absolute, json_str)
            .map_err(|e| anyhow!("Failed to write export file: {}", e))?;

        Ok(json!({ "count": entries.len(), "file_path": absolute.to_string_lossy().to_string() }))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tool 15: ImportCacheTool
// ═══════════════════════════════════════════════════════════════════

pub struct ImportCacheTool;

#[async_trait::async_trait]
impl Tool for ImportCacheTool {
    fn name(&self) -> &str {
        "import_cache"
    }
    fn description(&self) -> &str {
        "Imports cached entries from a previously exported JSON file."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "Path to the JSON export file." }
            },
            "required": ["file_path"]
        })
    }
    async fn call(&self, arguments: &Value) -> Result<Value> {
        let path_str = arguments["file_path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing file_path parameter"))?;
        let path = std::path::Path::new(path_str);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| anyhow!("{}", e))?
                .join(path)
        };

        let json_str = std::fs::read_to_string(&absolute)
            .map_err(|e| anyhow!("Failed to read import file: {}", e))?;

        let entries: Vec<Value> =
            serde_json::from_str(&json_str).map_err(|e| anyhow!("Invalid JSON format: {}", e))?;

        let conn = get_cache_connection()?;
        let mut count = 0i64;
        for entry in &entries {
            let id = entry["ccr_id"].as_str().unwrap_or("");
            let content = entry["content"].as_str().unwrap_or("");
            let created_at = entry["created_at"].as_str().unwrap_or("");
            if !id.is_empty() && !content.is_empty() {
                let now = chrono::Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO cache_entries (ccr_id, content, created_at, accessed_at, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, content, created_at, now, content.len() as i64],
                );
                count += 1;
            }
        }

        Ok(json!({ "imported": count, "file_path": absolute.to_string_lossy().to_string() }))
    }
}
