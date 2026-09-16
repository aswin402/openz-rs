use super::*;
use crate::tools::graph_memory::{test_lock, with_db};
use crate::tools::memory_extra::working;
use crate::tools::Tool;
use serde_json::json;

#[tokio::test]
async fn test_store_retrieve_shared_memory() {
    let _l = test_lock().lock().await;
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
        with_db(|conn| query_fts5(conn, "context-aware", 10, uid, sid, aid)).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["nodeId"], "fts-fact-1");
}

#[tokio::test]
async fn test_hybrid_search_uses_semantic_embeddings() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_hybrid_vec_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let (uid, sid, aid) = ("*", &scope, "*");

    working::store_semantic_fact(
        "hybrid-vector-fact",
        "zqxw plasma calibrator resonance handshake",
        0.9,
        uid,
        sid,
        aid,
    )
    .unwrap();

    let tool = HybridSearchTool;
    let res = tool
        .call(&json!({
            "query": "zqxw plasma calibrator resonance handshake",
            "sessionId": scope,
            "limit": 3
        }))
        .await
        .unwrap();
    let arr = res.as_array().unwrap();
    let matched = arr
        .iter()
        .find(|item| item["nodeId"] == "hybrid-vector-fact")
        .expect("hybrid search should return the stored fact");
    assert!(
        matched["vectorSimilarity"].as_f64().unwrap_or(0.0) > 0.0,
        "hybrid search result should include vector similarity from semantic embedding"
    );
}

#[tokio::test]
async fn test_text_similarity() {
    assert!((text_similarity("hello world", "hello world") - 1.0).abs() < 0.01);
    assert!((text_similarity("hello world", "hello there") - 0.333).abs() < 0.01);
    assert_eq!(text_similarity("hello", "world"), 0.0);
}

