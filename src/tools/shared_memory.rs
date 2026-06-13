use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub timestamp: String,
    pub workspace: String,
    pub tags: Vec<String>,
}

pub fn get_db_mutex() -> &'static tokio::sync::Mutex<()> {
    static DB_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    DB_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

pub fn get_db_path() -> PathBuf {
    if let Ok(override_dir) = std::env::var("OPENZ_CONFIG_DIR") {
        PathBuf::from(override_dir).join("shared_memory.json")
    } else {
        crate::config::resolve_path("~/.openz/shared_memory.json")
    }
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

// 1. StoreMemoryTool
pub struct StoreMemoryTool;

#[async_trait::async_trait]
impl Tool for StoreMemoryTool {
    fn name(&self) -> &str {
        "store_memory"
    }

    fn description(&self) -> &str {
        "Store a fact, guideline, key decision, or code pattern in the shared memory bus to share with other agents."
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

        let embedding = get_embedding(text, false).await?;
        let workspace = get_current_workspace();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        let new_entry = MemoryEntry {
            id,
            text: text.to_string(),
            embedding,
            timestamp,
            workspace,
            tags,
        };

        let db_path = get_db_path();
        
        let _lock = get_db_mutex().lock().await;
        
        let mut entries: Vec<MemoryEntry> = if db_path.exists() {
            let data = fs::read_to_string(&db_path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };

        entries.push(new_entry);

        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(&entries)?;
        fs::write(db_path, serialized)?;

        Ok(json!({
            "status": "success",
            "message": "Fact stored successfully in the shared memory bus."
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
        "Recall relevant facts, guidelines, or code patterns from the shared memory bus based on a semantic query."
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
        let db_path = get_db_path();

        let _lock = get_db_mutex().lock().await;

        if !db_path.exists() {
            return Ok(json!({
                "status": "success",
                "matches": []
            }));
        }

        let data = fs::read_to_string(&db_path)?;
        let entries: Vec<MemoryEntry> = serde_json::from_str(&data).unwrap_or_default();

        let mut scored_results = Vec::new();

        for entry in &entries {
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

            let sim = cosine_similarity(&query_embed, &entry.embedding);
            scored_results.push((sim, entry));
        }

        // Sort descending by similarity score
        scored_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let matches: Vec<Value> = scored_results.into_iter()
            .take(top_k)
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
            "matches": matches
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
        "Clear stored memories from the shared memory bus, either from the current workspace or entirely."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["workspace", "all"],
                    "description": "The scope of memory deletion. 'workspace' deletes memories for the current project. 'all' deletes all stored memories (default 'workspace')."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("workspace");
        let db_path = get_db_path();

        let _lock = get_db_mutex().lock().await;

        if !db_path.exists() {
            return Ok(json!({
                "status": "success",
                "message": "Memory was already empty."
            }));
        }

        if scope == "all" {
            let _ = fs::remove_file(&db_path);
            Ok(json!({
                "status": "success",
                "message": "All memories cleared successfully."
            }))
        } else {
            let data = fs::read_to_string(&db_path)?;
            let entries: Vec<MemoryEntry> = serde_json::from_str(&data).unwrap_or_default();
            let current_ws = get_current_workspace();

            let (remaining, removed): (Vec<MemoryEntry>, Vec<MemoryEntry>) = entries.into_iter()
                .partition(|e| e.workspace != current_ws);

            let serialized = serde_json::to_string_pretty(&remaining)?;
            fs::write(db_path, serialized)?;

            Ok(json!({
                "status": "success",
                "message": format!("Cleared {} memories associated with the current workspace.", removed.len())
            }))
        }
    }
}

pub async fn consolidate_shared_memory(provider: &std::sync::Arc<dyn crate::providers::LLMProvider>) -> Result<()> {
    let db_path = get_db_path();
    
    // Acquire DB lock
    let _lock = get_db_mutex().lock().await;
    
    if !db_path.exists() {
        return Ok(());
    }
    
    let data = fs::read_to_string(&db_path)?;
    let mut entries: Vec<MemoryEntry> = serde_json::from_str(&data).unwrap_or_default();
    
    if entries.len() < 5 {
        return Ok(());
    }

    let mut consolidated_count = 0;
    
    // Loop up to 3 merges per compaction run to avoid excessive calls
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

        // We consolidate if they are highly similar (threshold 0.82)
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
                        
                        // Merge tags
                        let mut merged_tags = entry_a.tags.clone();
                        for t in &entry_b.tags {
                            if !merged_tags.contains(t) {
                                merged_tags.push(t.clone());
                            }
                        }

                        // Keep newer workspace/timestamp/id
                        let workspace = if entry_a.workspace == entry_b.workspace {
                            entry_a.workspace.clone()
                        } else {
                            entry_a.workspace.clone()
                        };

                        let consolidated_entry = MemoryEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            text: clean_text.clone(),
                            embedding: new_embed,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            workspace,
                            tags: merged_tags,
                        };

                        // Remove both elements (j is greater than i, so remove j first to preserve index i)
                        entries.remove(j);
                        entries.remove(i);
                        
                        entries.push(consolidated_entry);
                        consolidated_count += 1;
                        continue;
                    }
                }
            }
        }
        break;
    }

    if consolidated_count > 0 {
        let serialized = serde_json::to_string_pretty(&entries)?;
        fs::write(&db_path, serialized)?;
        
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
        
        // Isolate config directory for the test
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
            "tags": ["cargo", "debug"]
        })).await?;
        assert_eq!(res["status"], "success");

        // Store another memory
        let res = store_tool.call(&json!({
            "text": "Docker builds should utilize multi-stage builds to reduce final image size.",
            "tags": ["devops", "docker"]
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
        // Store 2 highly similar facts
        let _ = store_tool.call(&json!({
            "text": "Cargo check can sometimes fail due to lock conflicts. Run cargo clean to fix the compiler error.",
            "tags": ["cargo"]
        })).await?;
        let _ = store_tool.call(&json!({
            "text": "Cargo check can sometimes fail due to lock conflicts. Run cargo clean to fix this compiler error.",
            "tags": ["debug"]
        })).await?;
        // Store 3 other different facts so total is 5
        let _ = store_tool.call(&json!({
            "text": "Docker builds use multi-stage recipes to reduce image size.",
            "tags": ["docker"]
        })).await?;
        let _ = store_tool.call(&json!({
            "text": "Git worktrees allow isolated branching for concurrent agent tasks.",
            "tags": ["git"]
        })).await?;
        let _ = store_tool.call(&json!({
            "text": "Rust async traits require the async-trait crate macro.",
            "tags": ["rust"]
        })).await?;

        // 2. Setup mock provider for the merge result
        let mock_provider = Arc::new(MockProvider {
            response_content: "Consolidated Fact: Resolve cargo check lock conflicts by killing locks, cleaning target, or clean cargo registry.".to_string(),
        }) as Arc<dyn crate::providers::LLMProvider>;

        // 3. Consolidate memory
        let res = consolidate_shared_memory(&mock_provider).await;
        assert!(res.is_ok());

        // 4. Verify that two highly similar memories were consolidated into one,
        // so total number of memories is now 4 (5 - 2 + 1)
        let data = fs::read_to_string(temp_dir.join("shared_memory.json"))?;
        let entries: Vec<MemoryEntry> = serde_json::from_str(&data).unwrap_or_default();
        assert_eq!(entries.len(), 4);

        // Verify that the consolidated text is present
        assert!(entries.iter().any(|e| e.text.contains("Consolidated Fact: Resolve cargo check")));

        // Cleanup
        std::env::remove_var("OPENZ_CONFIG_DIR");
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}

