use super::*;
use crate::tools::graph_memory::test_lock;
use crate::tools::Tool;
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
