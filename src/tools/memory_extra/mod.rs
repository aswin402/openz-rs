pub mod codebase;
pub mod episodic;
pub mod facts;
pub mod graph;
pub mod search;
pub mod working;

pub use codebase::{
    AnalyzeCodeImpactTool, CompactMemoriesTool, CompressContextTool, IndexCodebaseTool,
    MemoryStatsTool, QueryCodeGraphTool,
};
pub use episodic::{
    LogExecutionEpisodeTool, LogReflectionTool, QueryToolPerformanceTool,
    RecordToolPerformanceTool, RetrieveEpisodicReflectionsTool,
};
pub use facts::{
    ExtractAndStoreFactsTool, InvalidateFactTool, ProactiveRecallTool, QueryAsOfTool,
    QueryFactHistoryTool, SmartStoreTool,
};
pub use graph::{
    AnalyzeGraphCommunitiesTool, DetectAndResolveConflictsTool, FindPathTool,
    LogRepositoryEvolutionTool, QueryRepositoryEvolutionTool, TraverseGraphTool,
};
pub use search::{
    HybridSearchTool, RetrieveSharedTeamMemoryTool, SearchTextTool, StoreSharedTeamMemoryTool,
};
pub use working::{
    EvictExpiredWorkingMemoryTool, GetWorkingMemoryTool, PromoteWorkingMemoryTool,
    SetWorkingMemoryTool,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::graph_memory::test_lock;
    use crate::tools::graph_memory::with_db;
    use crate::tools::Tool;
    use rusqlite::params;
    use serde_json::json;

    #[tokio::test]
    async fn test_set_get_working_memory() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_wm_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let set_tool = SetWorkingMemoryTool;
        let res = set_tool
            .call(&json!({
                "key": "test_key", "value": "test_value", "ttl": 60, "sessionId": scope
            }))
            .await
            .unwrap();
        assert!(res["status"].as_str().unwrap().contains("test_key"));

        let get_tool = GetWorkingMemoryTool;
        let res2 = get_tool
            .call(&json!({
                "key": "test_key", "sessionId": scope
            }))
            .await
            .unwrap();
        assert_eq!(res2["value"], "test_value");
    }

    #[tokio::test]
    async fn test_get_working_memory_expired() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_wm_exp_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let set_tool = SetWorkingMemoryTool;
        set_tool
            .call(&json!({
                "key": "exp_key", "value": "exp_value", "ttl": 0, "sessionId": scope
            }))
            .await
            .unwrap();

        // Wait a tiny bit to ensure expiration
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let get_tool = GetWorkingMemoryTool;
        let res = get_tool
            .call(&json!({
                "key": "exp_key", "sessionId": scope
            }))
            .await
            .unwrap();
        assert!(res["status"].as_str().unwrap().contains("expired"));
    }

    #[tokio::test]
    async fn test_log_and_retrieve_reflections() {
        let scope = format!(
            "test_ref_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let log_tool = LogReflectionTool;
        log_tool
            .call(&json!({
                "taskDescription": "Test task",
                "status": "Success",
                "attemptNumber": 1,
                "stepsTaken": "Step 1",
                "reflection": "It worked",
                "sessionId": scope
            }))
            .await
            .unwrap();

        let retrieve_tool = RetrieveEpisodicReflectionsTool;
        let res = retrieve_tool
            .call(&json!({
                "query": "Test task",
                "sessionId": scope
            }))
            .await
            .unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
        assert_eq!(arr[0]["taskDescription"], "Test task");
    }

    #[tokio::test]
    async fn test_log_execution_episode() {
        let scope = format!(
            "test_ep_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let tool = LogExecutionEpisodeTool;
        let res = tool
            .call(&json!({
                "taskDescription": "Test episode",
                "executionStatus": "Completed",
                "stepsTaken": "Did something",
                "sessionId": scope
            }))
            .await
            .unwrap();
        assert_eq!(res["status"], "Episode logged successfully");
    }

    #[tokio::test]
    async fn test_store_retrieve_shared_memory() {
        let scope = format!(
            "test_shr_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let store = StoreSharedTeamMemoryTool;
        store
            .call(&json!({
                "key": "shared_key",
                "value": "shared_value",
                "sourceAgent": "agent_a",
                "targetAgents": ["agent_b"],
                "sessionId": scope
            }))
            .await
            .unwrap();

        let retrieve = RetrieveSharedTeamMemoryTool;
        let res = retrieve
            .call(&json!({
                "agentId": "agent_b",
                "sessionId": scope
            }))
            .await
            .unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
        assert_eq!(arr[0]["key"], "shared_key");
    }

    #[tokio::test]
    async fn test_record_query_tool_performance() {
        let scope = format!(
            "test_perf_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let record = RecordToolPerformanceTool;
        record
            .call(&json!({
                "toolName": "test_tool",
                "modelName": "test_model",
                "taskType": "coding",
                "successCount": 5,
                "failureCount": 1,
                "averageLatency": 0.5,
                "sessionId": scope
            }))
            .await
            .unwrap();

        let query = QueryToolPerformanceTool;
        let res = query
            .call(&json!({
                "taskType": "coding",
                "sessionId": scope
            }))
            .await
            .unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
        assert_eq!(arr[0]["toolName"], "test_tool");
    }

    #[tokio::test]
    async fn test_text_similarity() {
        assert!((search::text_similarity("hello world", "hello world") - 1.0).abs() < 0.01);
        assert!((search::text_similarity("hello world", "hello there") - 0.333).abs() < 0.01);
        assert_eq!(search::text_similarity("hello", "world"), 0.0);
    }

    #[tokio::test]
    async fn test_extract_facts() {
        let facts = facts::extract_facts("Alice uses Rust. Bob created Python.");
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].from, "Alice");
        assert_eq!(facts[0].to, "Rust");
        assert_eq!(facts[0].relation, "uses");
    }

    #[tokio::test]
    async fn test_promote_working_memory() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_prom_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let set_tool = SetWorkingMemoryTool;
        set_tool
            .call(&json!({
                "key": "prom_key", "value": "prom_value", "ttl": 60, "sessionId": scope
            }))
            .await
            .unwrap();

        let promote = PromoteWorkingMemoryTool;
        let res = promote
            .call(&json!({
                "key": "prom_key", "sessionId": scope
            }))
            .await
            .unwrap();
        assert!(res["status"].as_str().unwrap().contains("Promoted"));

        // Should be gone from working memory
        let get_tool = GetWorkingMemoryTool;
        let res2 = get_tool
            .call(&json!({
                "key": "prom_key", "sessionId": scope
            }))
            .await
            .unwrap();
        assert!(res2["status"].as_str().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_static_semantic_fact_store() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_sf_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let (uid, sid, aid) = ("*", &scope, "*");

        working::store_semantic_fact(
            "test-fact-1",
            "Rust is a systems language.",
            0.8,
            uid,
            sid,
            aid,
        )
        .unwrap();

        let results =
            with_db(|conn| search::query_fts5(conn, "systems language", 10, uid, sid, aid))
                .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0]["nodeId"], "test-fact-1");
    }

    #[tokio::test]
    async fn test_search_text_fts5() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_fts_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let (uid, sid, aid) = ("*", &scope, "*");

        working::store_semantic_fact(
            "fts-fact-1",
            "MCP defines a standard protocol for context-aware AI tools.",
            0.9,
            uid,
            sid,
            aid,
        )
        .unwrap();
        working::store_semantic_fact(
            "fts-fact-2",
            "SQLite is a self-contained SQL database engine.",
            0.7,
            uid,
            sid,
            aid,
        )
        .unwrap();

        let results =
            with_db(|conn| search::query_fts5(conn, "context-aware", 10, uid, sid, aid)).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["nodeId"], "fts-fact-1");
    }

    #[tokio::test]
    async fn test_query_fact_history() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_qfh_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

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
        let res = tool
            .call(&json!({
                "entityName": "EntityA",
                "sessionId": scope
            }))
            .await
            .unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
        assert_eq!(arr[0]["from"], "EntityA");
    }

    #[tokio::test]
    async fn test_invalidate_semantic_fact() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_inv_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let (uid, sid, aid) = ("*", &scope, "*");

        working::store_semantic_fact("inv-fact-1", "Temporary data.", 0.5, uid, sid, aid).unwrap();

        let tool = InvalidateFactTool;
        let res = tool
            .call(&json!({
                "factId": "inv-fact-1",
                "sessionId": scope
            }))
            .await
            .unwrap();
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
        assert!(
            res["originalLength"].as_u64().unwrap() > res["compressedLength"].as_u64().unwrap()
        );
    }

    #[tokio::test]
    async fn test_proactive_recall() {
        let scope = format!(
            "test_pr_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let (uid, sid, aid) = ("*", &scope, "*");

        working::store_semantic_fact(
            "pr-fact-1",
            "Rust compiler optimizations improve performance.",
            0.9,
            uid,
            sid,
            aid,
        )
        .unwrap();

        let tool = ProactiveRecallTool;
        let res = tool
            .call(&json!({
                "query": "rust compiler",
                "sessionId": scope
            }))
            .await
            .unwrap();
        let arr = res.as_array().unwrap();
        assert!(!arr.is_empty());
    }

    #[tokio::test]
    async fn test_smart_store_text() {
        let scope = format!(
            "test_sst_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let tool = SmartStoreTool;
        let res = tool
            .call(&json!({
                "text": "Smart store test fact.",
                "sessionId": scope
            }))
            .await
            .unwrap();
        assert_eq!(res["action"], "add");
    }

    #[tokio::test]
    async fn test_evict_expired_working_memory() {
        let _l = test_lock().lock().await;
        let scope = format!(
            "test_ev_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let set_tool = SetWorkingMemoryTool;
        set_tool
            .call(&json!({
                "key": "evict_key", "value": "evict_value", "ttl": 0, "sessionId": scope
            }))
            .await
            .unwrap();

        // Wait a moment for expiration
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let evict = EvictExpiredWorkingMemoryTool;
        let res = evict.call(&json!({ "sessionId": scope })).await.unwrap();
        assert!(res["status"].as_str().unwrap().contains("Evicted"));
    }
}
