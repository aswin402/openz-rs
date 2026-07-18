use super::*;
use crate::tools::Tool;
use anyhow::Result;
use serde_json::json;
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

    // Clear any existing memories from previous tests
    let _ = clear_tool.call(&json!({ "scope": "all" })).await?;

    // 1. Initial recall should return empty list
    let res = recall_tool
        .call(&json!({
            "query": "something",
            "scope": "global"
        }))
        .await?;
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
    let res = store_tool
        .call(&json!({
            "text": "Docker builds should utilize multi-stage builds to reduce final image size.",
            "tags": ["devops", "docker"],
            "importance": 0.8
        }))
        .await?;
    assert_eq!(res["status"], "success");

    // 3. Recall memory (semantic search)
    let res = recall_tool
        .call(&json!({
            "query": "How do I fix cargo lock or compilation errors?",
            "top_k": 1,
            "scope": "global"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    let matches = res["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert!(matches[0]["text"].as_str().unwrap().contains("Cargo check"));
    assert!(matches[0]["score"].as_f64().unwrap() > 0.1);

    // 4. Recall with tag filter
    let res = recall_tool
        .call(&json!({
            "query": "deployment",
            "tags": ["docker"],
            "scope": "global"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    let matches = res["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert!(matches[0]["text"]
        .as_str()
        .unwrap()
        .contains("Docker builds"));

    // Test decay pruning directly
    {
        let count = with_db(|conn| prune_decayed_memories(conn))?;
        assert_eq!(count, 0); // No memory should decay yet
    }

    // 5. Clear memories
    let res = clear_tool
        .call(&json!({
            "scope": "all"
        }))
        .await?;
    assert_eq!(res["status"], "success");

    // Verify cleared
    let res = recall_tool
        .call(&json!({
            "query": "cargo",
            "scope": "global"
        }))
        .await?;
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
    let temp_dir =
        std::env::temp_dir().join(format!("openz_mem_con_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    std::env::set_var("OPENZ_CONFIG_DIR", &temp_dir);

    let store_tool = StoreMemoryTool;
    let clear_tool = ClearMemoryTool;
    let _ = clear_tool.call(&json!({ "scope": "all" })).await?;

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
    let _ = store_tool
        .call(&json!({
            "text": "Docker builds use multi-stage recipes to reduce image size.",
            "tags": ["docker"],
            "importance": 0.7
        }))
        .await?;
    let _ = store_tool
        .call(&json!({
            "text": "Git worktrees allow isolated branching for concurrent agent tasks.",
            "tags": ["git"],
            "importance": 0.7
        }))
        .await?;
    let _ = store_tool
        .call(&json!({
            "text": "Rust async traits require the async-trait crate macro.",
            "tags": ["rust"],
            "importance": 0.9
        }))
        .await?;

    // 2. Setup mock provider for the merge result
    let mock_provider = Arc::new(MockProvider {
        response_content: "Consolidated Fact: Resolve cargo check lock conflicts by killing locks, cleaning target, or clean cargo registry.".to_string(),
    }) as Arc<dyn crate::providers::LLMProvider>;

    // 3. Consolidate memory
    let res = consolidate_shared_memory(&mock_provider).await;
    assert!(res.is_ok());

    // 4. Verify that SQLite database has 4 entries (5 - 2 + 1)
    let (count, text_found) = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM cognitive_memory")?;
        let count: i64 = stmt.query_row([], |r| r.get(0))?;

        // Verify that the consolidated text is present
        let mut stmt_check = conn
            .prepare("SELECT text FROM cognitive_memory WHERE text LIKE '%Consolidated Fact%'")?;
        let text_found: String = stmt_check.query_row([], |r| r.get(0))?;
        Ok((count, text_found))
    })?;
    assert_eq!(count, 4);
    assert!(text_found.contains("Consolidated Fact: Resolve cargo check"));

    // Cleanup
    std::env::remove_var("OPENZ_CONFIG_DIR");
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_research_archive_workflow() -> Result<()> {
    let _lock = TestLock::acquire();
    let temp_dir =
        std::env::temp_dir().join(format!("openz_research_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    std::env::set_var("OPENZ_CONFIG_DIR", &temp_dir);

    let archive_tool = ArchiveResearchTool;
    let search_tool = SearchResearchTool;

    // 1. Archive some mock web research data
    let res = archive_tool.call(&json!({
        "query": "Rust actix-web tutorial and basic examples",
        "content": "To set up actix-web in Rust, add actix-web = \"4\" to Cargo.toml. Then use HttpServer::new and App::new.",
        "source": "web_fetch: https://actix.rs/docs/"
    })).await?;
    assert_eq!(res["status"], "success");

    // 2. Search for the research data
    let res = search_tool
        .call(&json!({
            "query": "actix-web examples",
            "top_k": 1
        }))
        .await?;
    assert_eq!(res["status"], "success");
    let matches = res["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert!(matches[0]["content"]
        .as_str()
        .unwrap()
        .contains("To set up actix-web"));
    assert!(matches[0]["source"].as_str().unwrap().contains("actix.rs"));

    // Cleanup
    std::env::remove_var("OPENZ_CONFIG_DIR");
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_interaction_history_workflow() -> Result<()> {
    let _lock = TestLock::acquire();
    let temp_dir =
        std::env::temp_dir().join(format!("openz_interact_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    std::env::set_var("OPENZ_CONFIG_DIR", &temp_dir);

    // 1. Log interaction with a unique marker. The shared-memory SQLite
    // connection is process-global, so this test must not assume the history
    // table is otherwise empty when the full suite runs in parallel.
    let marker = uuid::Uuid::new_v4().to_string();
    let query = format!("build the web UI dashboard {marker}");
    let id = log_interaction("test_session", &query).await?;
    assert!(!id.is_empty());

    // 2. Fetch recent interactions and find our unique row
    let history = get_recent_interactions(20).await?;
    let entry = history
        .iter()
        .find(|item| item["query"] == query)
        .expect("unique interaction row should be present");
    assert_eq!(entry["success"], true);

    // 3. Update errors
    update_interaction_errors(&id, "Cargo build failed with exit status 101").await?;

    // 4. Verify updated history
    let history2 = get_recent_interactions(20).await?;
    let updated = history2
        .iter()
        .find(|item| item["query"] == query)
        .expect("updated unique interaction row should be present");
    assert_eq!(updated["success"], false);
    assert_eq!(
        updated["errors"].as_str().unwrap(),
        "Cargo build failed with exit status 101"
    );

    // Cleanup
    std::env::remove_var("OPENZ_CONFIG_DIR");
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_chunk_content_by_headings() {
    let query = "my query";
    let content =
        "# Heading 1\nLine 1\nLine 2\n## Heading 2\nLine 3\n--- Sheet: Sheet1 ---\nLine 4";
    let chunks = chunk_content_by_headings(query, content);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].0, "my query - # Heading 1");
    assert!(chunks[0].1.contains("Line 1"));
    assert_eq!(chunks[1].0, "my query - ## Heading 2");
    assert!(chunks[1].1.contains("Line 3"));
    assert_eq!(chunks[2].0, "my query - --- Sheet: Sheet1 ---");
    assert!(chunks[2].1.contains("Line 4"));
}
