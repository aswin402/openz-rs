use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use rusqlite::{Connection, params};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CognitiveMemoryEntry {
    pub id: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub timestamp: String,
    pub workspace: String,
    pub tags: Vec<String>,
    pub importance: f32,
    pub last_accessed: String,
    pub access_count: i64,
    pub decay_rate: f32,
}

pub fn get_db_mutex() -> &'static tokio::sync::Mutex<()> {
    static DB_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    DB_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

pub fn get_sqlite_db_path() -> PathBuf {
    if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
        PathBuf::from(override_dir).join("memory.db")
    } else {
        crate::config::resolve_path("~/.openz/memory.db")
    }
}

pub fn get_sqlite_connection() -> Result<Connection> {
    let path = get_sqlite_db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cognitive_memory (
            id TEXT PRIMARY KEY,
            text TEXT NOT NULL,
            embedding TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            workspace TEXT NOT NULL,
            tags TEXT NOT NULL,
            importance REAL NOT NULL,
            last_accessed TEXT NOT NULL,
            access_count INTEGER DEFAULT 1,
            decay_rate REAL DEFAULT 0.05
        )",
        [],
    )?;
    Ok(conn)
}

pub fn get_current_workspace() -> String {
    if let Some(dir) = crate::config::loader::ACTIVE_WORKSPACE.try_with(|w| w.clone()).ok() {
        if let Ok(abs_path) = std::fs::canonicalize(&dir) {
            return abs_path.to_string_lossy().to_string();
        }
        return dir.to_string_lossy().to_string();
    }
    if let Ok(curr_dir) = std::env::current_dir() {
        if let Ok(abs_path) = std::fs::canonicalize(&curr_dir) {
            return abs_path.to_string_lossy().to_string();
        }
        return curr_dir.to_string_lossy().to_string();
    }
    ".".to_string()
}

pub async fn get_embedding(text: &str, is_query: bool) -> Result<Vec<f32>> {
    let text_owned = text.to_string();
    tokio::task::spawn_blocking(move || -> Result<Vec<f32>> {
        let mut model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2))?;
        let formatted = if is_query {
            format!("query: {}", text_owned)
        } else {
            format!("passage: {}", text_owned)
        };
        let embeds = model.embed(vec![&formatted], None)?;
        Ok(embeds[0].clone())
    }).await?
}

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for i in 0..std::cmp::min(v1.len(), v2.len()) {
        dot_product += v1[i] * v2[i];
        norm_a += v1[i] * v1[i];
        norm_b += v2[i] * v2[i];
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a.sqrt() * norm_b.sqrt())
    }
}

pub fn prune_decayed_memories(conn: &Connection) -> Result<usize> {
    let mut stmt = conn.prepare("SELECT id, importance, last_accessed, decay_rate FROM cognitive_memory")?;
    let now = chrono::Utc::now();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, f64>(3)?,
        ))
    })?;

    let mut to_delete = Vec::new();
    for r in rows.flatten() {
        let (id, importance, last_acc_str, decay_rate) = r;
        let last_acc_date = chrono::DateTime::parse_from_rfc3339(&last_acc_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        let duration = now.signed_duration_since(last_acc_date);
        let days_elapsed = duration.num_seconds() as f64 / 86400.0;
        
        let decay_factor = (-decay_rate * days_elapsed).exp();
        let decayed_importance = importance * decay_factor;
        
        if decayed_importance < 0.15 {
            to_delete.push(id);
        }
    }

    let count = to_delete.len();
    for id in to_delete {
        let _ = conn.execute("DELETE FROM cognitive_memory WHERE id = ?", [id]);
    }
    Ok(count)
}

// 1. StoreMemoryTool
pub struct StoreMemoryTool;

#[async_trait::async_trait]
impl Tool for StoreMemoryTool {
    fn name(&self) -> &str {
        "store_memory"
    }

    fn description(&self) -> &str {
        "Store a fact, guideline, key decision, or code pattern in the cognitive memory bus with support for importance weighting and temporal decay."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "The description of the fact, key solution, API usage, or decision to remember."
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional category tags (e.g. ['auth', 'cargo', 'setup', 'sqlite'])."
                },
                "importance": {
                    "type": "number",
                    "description": "Optional importance score from 0.0 (low) to 1.0 (high) (default 0.8)."
                },
                "decay_rate": {
                    "type": "number",
                    "description": "Optional decay rate for time-based forgetting (default 0.05)."
                }
            },
            "required": ["text"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let text = arguments.get("text").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'text' parameter"))?;
        
        let tags = if let Some(arr) = arguments.get("tags").and_then(|v| v.as_array()) {
            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
        } else {
            Vec::new()
        };

        let importance = arguments.get("importance").and_then(|v| v.as_f64()).unwrap_or(0.8) as f32;
        let decay_rate = arguments.get("decay_rate").and_then(|v| v.as_f64()).unwrap_or(0.05) as f32;

        let embedding = get_embedding(text, false).await?;
        let workspace = get_current_workspace();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        let embedding_json = serde_json::to_string(&embedding)?;
        let tags_json = serde_json::to_string(&tags)?;

        let _lock = get_db_mutex().lock().await;
        let conn = get_sqlite_connection()?;
        
        conn.execute(
            "INSERT INTO cognitive_memory (id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9)",
            params![id, text, embedding_json, timestamp, workspace, tags_json, importance, timestamp, decay_rate],
        )?;

        let _ = prune_decayed_memories(&conn);

        Ok(json!({
            "status": "success",
            "message": "Fact stored successfully in the cognitive memory bus."
        }))
    }
}

// 2. RecallMemoryTool
pub struct RecallMemoryTool;

#[async_trait::async_trait]
impl Tool for RecallMemoryTool {
    fn name(&self) -> &str {
        "recall_memory"
    }

    fn description(&self) -> &str {
        "Recall relevant facts, guidelines, or code patterns from the cognitive memory bus using semantic similarity combined with temporal memory decay."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query to search for in memory (e.g. 'cargo check workflow issue')."
                },
                "top_k": {
                    "type": "integer",
                    "description": "Optional number of top matches to return (default 5)."
                },
                "scope": {
                    "type": "string",
                    "enum": ["workspace", "global"],
                    "description": "Optional search scope. 'workspace' limits results to the current active project/directory. 'global' searches all workspaces (default 'workspace')."
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional list of tags to filter by. Matches entries containing at least one tag."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments.get("query").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;
        
        let top_k = arguments.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
        let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("workspace");
        
        let filter_tags: Vec<String> = if let Some(arr) = arguments.get("tags").and_then(|v| v.as_array()) {
            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
        } else {
            Vec::new()
        };

        let query_embed = get_embedding(query, true).await?;
        let current_ws = get_current_workspace();

        let _lock = get_db_mutex().lock().await;
        let conn = get_sqlite_connection()?;

        let mut stmt = conn.prepare("SELECT id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate FROM cognitive_memory")?;
        let rows = stmt.query_map([], |row| {
            let embedding_str: String = row.get(2)?;
            let tags_str: String = row.get(5)?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_str).unwrap_or_default();
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            
            Ok(CognitiveMemoryEntry {
                id: row.get(0)?,
                text: row.get(1)?,
                embedding,
                timestamp: row.get(3)?,
                workspace: row.get(4)?,
                tags,
                importance: row.get(6)?,
                last_accessed: row.get(7)?,
                access_count: row.get(8)?,
                decay_rate: row.get(9)?,
            })
        })?;

        let now = chrono::Utc::now();
        let mut scored_results = Vec::new();

        for entry in rows.flatten() {
            // Scope filter
            if scope == "workspace" && entry.workspace != current_ws {
                continue;
            }

            // Tag filter
            if !filter_tags.is_empty() {
                let matches_tag = entry.tags.iter().any(|t| filter_tags.contains(t));
                if !matches_tag {
                    continue;
                }
            }

            // Calculate temporal decay factor
            let last_acc_date = chrono::DateTime::parse_from_rfc3339(&entry.last_accessed)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or(now);
            let duration = now.signed_duration_since(last_acc_date);
            let days_elapsed = duration.num_seconds() as f32 / 86400.0;
            
            let decay_factor = (-entry.decay_rate * days_elapsed).exp();
            let decayed_importance = entry.importance * decay_factor;

            let sim = cosine_similarity(&query_embed, &entry.embedding);
            
            // Combine similarity (70% weight) and decayed importance (30% weight)
            let cognitive_score = sim * 0.7 + decayed_importance * 0.3;

            scored_results.push((cognitive_score, entry));
        }

        // Sort descending by cognitive score
        scored_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let selected_matches: Vec<(f32, CognitiveMemoryEntry)> = scored_results.into_iter()
            .take(top_k)
            .collect();

        // Update last_accessed and access_count for returned matches
        let now_str = now.to_rfc3339();
        for (_, entry) in &selected_matches {
            let _ = conn.execute(
                "UPDATE cognitive_memory SET last_accessed = ?1, access_count = access_count + 1 WHERE id = ?2",
                params![now_str, entry.id],
            );
        }

        let matches_val: Vec<Value> = selected_matches.into_iter()
            .map(|(score, entry)| {
                json!({
                    "id": entry.id,
                    "text": entry.text,
                    "score": score,
                    "timestamp": entry.timestamp,
                    "workspace": entry.workspace,
                    "tags": entry.tags,
                })
            })
            .collect();

        Ok(json!({
            "status": "success",
            "matches": matches_val
        }))
    }
}

// 3. ClearMemoryTool
pub struct ClearMemoryTool;

#[async_trait::async_trait]
impl Tool for ClearMemoryTool {
    fn name(&self) -> &str {
        "clear_memory"
    }

    fn description(&self) -> &str {
        "Clear stored memories or force-compact the cognitive memory database."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["workspace", "all", "prune"],
                    "description": "The scope of memory deletion. 'workspace' deletes memories for the current project. 'all' deletes all stored memories. 'prune' deletes deeply decayed memories (default 'workspace')."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("workspace");
        
        let _lock = get_db_mutex().lock().await;
        let conn = get_sqlite_connection()?;

        if scope == "all" {
            conn.execute("DELETE FROM cognitive_memory", [])?;
            Ok(json!({
                "status": "success",
                "message": "All memories cleared successfully."
            }))
        } else if scope == "prune" {
            let count = prune_decayed_memories(&conn)?;
            Ok(json!({
                "status": "success",
                "message": format!("Pruned {} deeply decayed memories from the cognitive database.", count)
            }))
        } else {
            let current_ws = get_current_workspace();
            let count = conn.execute("DELETE FROM cognitive_memory WHERE workspace = ?", params![current_ws])?;
            Ok(json!({
                "status": "success",
                "message": format!("Cleared {} memories associated with the current workspace.", count)
            }))
        }
    }
}

pub async fn consolidate_shared_memory(provider: &std::sync::Arc<dyn crate::providers::LLMProvider>) -> Result<()> {
    let _lock = get_db_mutex().lock().await;
    
    let mut entries: Vec<CognitiveMemoryEntry> = {
        let conn = get_sqlite_connection()?;
        let mut stmt = conn.prepare("SELECT id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate FROM cognitive_memory")?;
        let rows = stmt.query_map([], |row| {
            let embedding_str: String = row.get(2)?;
            let tags_str: String = row.get(5)?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_str).unwrap_or_default();
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            
            Ok(CognitiveMemoryEntry {
                id: row.get(0)?,
                text: row.get(1)?,
                embedding,
                timestamp: row.get(3)?,
                workspace: row.get(4)?,
                tags,
                importance: row.get(6)?,
                last_accessed: row.get(7)?,
                access_count: row.get(8)?,
                decay_rate: row.get(9)?,
            })
        })?;
        rows.flatten().collect()
    };

    if entries.len() < 5 {
        return Ok(());
    }

    let mut consolidated_count = 0;
    
    for _ in 0..3 {
        let n = entries.len();
        if n < 2 {
            break;
        }

        let mut max_sim = 0.0;
        let mut best_pair = None;

        for i in 0..n {
            for j in (i + 1)..n {
                let sim = cosine_similarity(&entries[i].embedding, &entries[j].embedding);
                if sim > max_sim {
                    max_sim = sim;
                    best_pair = Some((i, j));
                }
            }
        }

        if let Some((i, j)) = best_pair {
            if max_sim >= 0.82 {
                let entry_a = &entries[i];
                let entry_b = &entries[j];
                
                let merge_prompt = format!(
                    "Fact A: {}\nFact B: {}\n\nPlease consolidate these two facts into a single, concise, and complete statement. Do not lose any technical guidelines, details, or specific values. Return ONLY the consolidated statement, with no conversational filler.",
                    entry_a.text, entry_b.text
                );

                let system_prompt = "You are a Shared Memory Curator. Consolidate similar facts and remove redundancy, preserving all technical details.";
                let messages = vec![crate::session::Message {
                    role: "user".to_string(),
                    content: merge_prompt,
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra: serde_json::Map::new(),
                }];

                let settings = crate::providers::GenerationSettings {
                    temperature: 0.1,
                    max_tokens: 512,
                    reasoning_effort: None,
                };

                let resp = provider.chat(system_prompt, &messages, &[], &settings).await?;
                if let Some(merged_text) = resp.content {
                    let clean_text = merged_text.trim().to_string();
                    if !clean_text.is_empty() {
                        let new_embed = get_embedding(&clean_text, false).await?;
                        
                        let mut merged_tags = entry_a.tags.clone();
                        for t in &entry_b.tags {
                            if !merged_tags.contains(t) {
                                merged_tags.push(t.clone());
                            }
                        }

                        let workspace = entry_a.workspace.clone();
                        let new_id = uuid::Uuid::new_v4().to_string();
                        let now_str = chrono::Utc::now().to_rfc3339();
                        
                        // Calculate merged importance (average of two, scaled up slightly for consolidation reinforcement)
                        let new_importance = ((entry_a.importance + entry_b.importance) / 2.0 + 0.1).min(1.0);

                        let new_entry = CognitiveMemoryEntry {
                            id: new_id.clone(),
                            text: clean_text.clone(),
                            embedding: new_embed,
                            timestamp: now_str.clone(),
                            workspace,
                            tags: merged_tags.clone(),
                            importance: new_importance,
                            last_accessed: now_str.clone(),
                            access_count: entry_a.access_count + entry_b.access_count,
                            decay_rate: (entry_a.decay_rate + entry_b.decay_rate) / 2.0,
                        };

                        // Remove from database and insert new merged entry inside a short-lived block
                        {
                            let conn = get_sqlite_connection()?;
                            let _ = conn.execute("DELETE FROM cognitive_memory WHERE id IN (?1, ?2)", params![entry_a.id, entry_b.id]);
                            
                            let embedding_json = serde_json::to_string(&new_entry.embedding)?;
                            let tags_json = serde_json::to_string(&new_entry.tags)?;
                            let _ = conn.execute(
                                "INSERT INTO cognitive_memory (id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                                params![new_id, clean_text, embedding_json, now_str, new_entry.workspace, tags_json, new_importance, now_str, new_entry.access_count, new_entry.decay_rate],
                            );
                        }

                        entries.remove(j);
                        entries.remove(i);
                        entries.push(new_entry);
                        consolidated_count += 1;
                        continue;
                    }
                }
            }
        }
        break;
    }

    if consolidated_count > 0 {
        let aura_blue = "\x1b[38;2;96;165;250m";
        let color_reset = "\x1b[0m";
        crate::channels::cli::send_notification(&format!(
            "{}◇ [Memory-Curator] Consolidated {} duplicate/redundant shared memories.{}",
            aura_blue, consolidated_count, color_reset
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct TestLock;

    impl TestLock {
        fn acquire() -> Self {
            let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
            loop {
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&lock_path)
                {
                    Ok(_) => break,
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }
            TestLock
        }
    }

    impl Drop for TestLock {
        fn drop(&mut self) {
            let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
            let _ = std::fs::remove_file(lock_path);
        }
    }

    #[tokio::test]
    async fn test_shared_memory_workflow() -> Result<()> {
        let _lock = TestLock::acquire();
        let temp_dir = std::env::temp_dir().join(format!("openz_mem_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        
        std::env::set_var("OPENZ_CONFIG_DIR", &temp_dir);

        let store_tool = StoreMemoryTool;
        let recall_tool = RecallMemoryTool;
        let clear_tool = ClearMemoryTool;

        // 1. Initial recall should return empty list
        let res = recall_tool.call(&json!({
            "query": "something",
            "scope": "global"
        })).await?;
        assert_eq!(res["status"], "success");
        assert_eq!(res["matches"].as_array().unwrap().len(), 0);

        // 2. Store memory
        let res = store_tool.call(&json!({
            "text": "Cargo check can sometimes fail due to lock conflicts. Run cargo clean or check for running processes.",
            "tags": ["cargo", "debug"],
            "importance": 0.9
        })).await?;
        assert_eq!(res["status"], "success");

        // Store another memory
        let res = store_tool.call(&json!({
            "text": "Docker builds should utilize multi-stage builds to reduce final image size.",
            "tags": ["devops", "docker"],
            "importance": 0.8
        })).await?;
        assert_eq!(res["status"], "success");

        // 3. Recall memory (semantic search)
        let res = recall_tool.call(&json!({
            "query": "How do I fix cargo lock or compilation errors?",
            "top_k": 1,
            "scope": "global"
        })).await?;
        assert_eq!(res["status"], "success");
        let matches = res["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert!(matches[0]["text"].as_str().unwrap().contains("Cargo check"));
        assert!(matches[0]["score"].as_f64().unwrap() > 0.1);

        // 4. Recall with tag filter
        let res = recall_tool.call(&json!({
            "query": "deployment",
            "tags": ["docker"],
            "scope": "global"
        })).await?;
        assert_eq!(res["status"], "success");
        let matches = res["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert!(matches[0]["text"].as_str().unwrap().contains("Docker builds"));

        // Test decay pruning directly
        {
            let conn = get_sqlite_connection()?;
            let count = prune_decayed_memories(&conn)?;
            assert_eq!(count, 0); // No memory should decay yet
        }

        // 5. Clear memories
        let res = clear_tool.call(&json!({
            "scope": "all"
        })).await?;
        assert_eq!(res["status"], "success");

        // Verify cleared
        let res = recall_tool.call(&json!({
            "query": "cargo",
            "scope": "global"
        })).await?;
        assert_eq!(res["matches"].as_array().unwrap().len(), 0);

        // Cleanup
        std::env::remove_var("OPENZ_CONFIG_DIR");
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    struct MockProvider {
        response_content: String,
    }

    #[async_trait::async_trait]
    impl crate::providers::LLMProvider for MockProvider {
        async fn chat(
            &self,
            _system_prompt: &str,
            _messages: &[crate::session::Message],
            _tools: &[serde_json::Value],
            _settings: &crate::providers::GenerationSettings,
        ) -> Result<crate::providers::LLMResponse> {
            Ok(crate::providers::LLMResponse {
                content: Some(self.response_content.clone()),
                tool_calls: Vec::new(),
                finish_reason: "stop".to_string(),
                reasoning_content: None,
            })
        }
    }

    #[tokio::test]
    async fn test_shared_memory_consolidation() -> Result<()> {
        let _lock = TestLock::acquire();
        let temp_dir = std::env::temp_dir().join(format!("openz_mem_con_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        std::env::set_var("OPENZ_CONFIG_DIR", &temp_dir);

        let store_tool = StoreMemoryTool;

        // 1. Store five entries (at least 5 required for consolidation to run)
        let _ = store_tool.call(&json!({
            "text": "Cargo check can sometimes fail due to lock conflicts. Run cargo clean to fix the compiler error.",
            "tags": ["cargo"],
            "importance": 0.8
        })).await?;
        let _ = store_tool.call(&json!({
            "text": "Cargo check can sometimes fail due to lock conflicts. Run cargo clean to fix this compiler error.",
            "tags": ["debug"],
            "importance": 0.8
        })).await?;
        let _ = store_tool.call(&json!({
            "text": "Docker builds use multi-stage recipes to reduce image size.",
            "tags": ["docker"],
            "importance": 0.7
        })).await?;
        let _ = store_tool.call(&json!({
            "text": "Git worktrees allow isolated branching for concurrent agent tasks.",
            "tags": ["git"],
            "importance": 0.7
        })).await?;
        let _ = store_tool.call(&json!({
            "text": "Rust async traits require the async-trait crate macro.",
            "tags": ["rust"],
            "importance": 0.9
        })).await?;

        // 2. Setup mock provider for the merge result
        let mock_provider = Arc::new(MockProvider {
            response_content: "Consolidated Fact: Resolve cargo check lock conflicts by killing locks, cleaning target, or clean cargo registry.".to_string(),
        }) as Arc<dyn crate::providers::LLMProvider>;

        // 3. Consolidate memory
        let res = consolidate_shared_memory(&mock_provider).await;
        assert!(res.is_ok());

        // 4. Verify that SQLite database has 4 entries (5 - 2 + 1)
        let conn = get_sqlite_connection()?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM cognitive_memory")?;
        let count: i64 = stmt.query_row([], |r| r.get(0))?;
        assert_eq!(count, 4);

        // Verify that the consolidated text is present
        let mut stmt_check = conn.prepare("SELECT text FROM cognitive_memory WHERE text LIKE '%Consolidated Fact%'")?;
        let text_found: String = stmt_check.query_row([], |r| r.get(0))?;
        assert!(text_found.contains("Consolidated Fact: Resolve cargo check"));

        // Cleanup
        std::env::remove_var("OPENZ_CONFIG_DIR");
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
