use super::*;
use crate::tools::Tool;
use serde_json::json;

fn unique_scope(prefix: &str) -> String {
    format!(
        "{}_{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

#[test]
fn test_scope_from_args_accepts_camel_and_snake_case() {
    assert_eq!(
        scope_from_args(&json!({"userId": "u1", "sessionId": "s1", "agentId": "a1"})),
        ("u1".to_string(), "s1".to_string(), "a1".to_string())
    );
    assert_eq!(
        scope_from_args(&json!({"user_id": "u2", "session_id": "s2", "agent_id": "a2"})),
        ("u2".to_string(), "s2".to_string(), "a2".to_string())
    );
}

#[tokio::test]
async fn test_create_and_read_entities() {
    let _l = test_lock().lock().await;
    let scope_id = unique_scope("test");

    let tool_c = CreateEntitiesTool;
    let res = tool_c
        .call(&json!({
            "entities": [
                { "name": "Alice", "entityType": "Person", "observations": ["Loves coffee"] },
                { "name": "Bob", "entityType": "Person", "observations": ["Loves tea"] }
            ],
            "sessionId": scope_id
        }))
        .await
        .unwrap();
    assert!(res["result"].is_array());

    let tool_r = ReadGraphTool;
    let res2 = tool_r
        .call(&json!({ "sessionId": scope_id }))
        .await
        .unwrap();
    let entities = res2["entities"].as_array().unwrap();
    assert!(entities.len() >= 2);
}

#[tokio::test]
async fn test_create_relations() {
    let _l = test_lock().lock().await;
    let scope_id = unique_scope("test_rel");

    let tool_c = CreateEntitiesTool;
    tool_c
        .call(&json!({
            "entities": [
                { "name": "X", "entityType": "Item", "observations": [] },
                { "name": "Y", "entityType": "Item", "observations": [] }
            ],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    let tool_r = CreateRelationsTool;
    let res = tool_r
        .call(&json!({
            "relations": [{ "from": "X", "to": "Y", "relationType": "connects_to" }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();
    assert!(res["result"].is_array());
}

#[tokio::test]
async fn test_add_observations() {
    let _l = test_lock().lock().await;
    let scope_id = unique_scope("test_obs");

    CreateEntitiesTool
        .call(&json!({
            "entities": [{ "name": "Node1", "entityType": "Test", "observations": [] }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    let tool = AddObservationsTool;
    let res = tool
        .call(&json!({
            "observations": [{ "entityName": "Node1", "contents": ["New observation"] }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();
    let added = res["result"][0]["addedObservations"].as_array().unwrap();
    assert_eq!(added.len(), 1);
}

#[tokio::test]
async fn test_delete_entities() {
    let _l = test_lock().lock().await;
    let scope_id = unique_scope("test_del");

    CreateEntitiesTool
        .call(&json!({
            "entities": [{ "name": "ToDelete", "entityType": "Temp", "observations": [] }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    DeleteEntitiesTool
        .call(&json!({
            "entityNames": ["ToDelete"],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    let res = ReadGraphTool
        .call(&json!({ "sessionId": scope_id }))
        .await
        .unwrap();
    let found = res["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "ToDelete");
    assert!(!found);
}

#[tokio::test]
async fn test_search_nodes() {
    let _l = test_lock().lock().await;
    let scope_id = unique_scope("test_srch");

    CreateEntitiesTool.call(&json!({
        "entities": [{ "name": "UniqueSearchTarget", "entityType": "Searchable", "observations": ["special keyword"] }],
        "sessionId": scope_id
    })).await.unwrap();

    let res = SearchNodesTool
        .call(&json!({ "query": "UniqueSearch", "sessionId": scope_id }))
        .await
        .unwrap();
    assert!(!res["entities"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_delete_relations() {
    let _l = test_lock().lock().await;
    let scope_id = unique_scope("test_delrel");

    CreateEntitiesTool
        .call(&json!({
            "entities": [
                { "name": "A", "entityType": "Node", "observations": [] },
                { "name": "B", "entityType": "Node", "observations": [] }
            ],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    CreateRelationsTool
        .call(&json!({
            "relations": [{ "from": "A", "to": "B", "relationType": "connected" }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    DeleteRelationsTool
        .call(&json!({
            "relations": [{ "from": "A", "to": "B", "relationType": "connected" }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    let res = ReadGraphTool
        .call(&json!({ "sessionId": scope_id }))
        .await
        .unwrap();
    assert!(res["relations"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_open_nodes() {
    let _l = test_lock().lock().await;
    let scope_id = unique_scope("test_open");

    CreateEntitiesTool
        .call(&json!({
            "entities": [{ "name": "OpenMe", "entityType": "Test", "observations": ["visible"] }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    let res = OpenNodesTool
        .call(&json!({ "names": ["OpenMe"], "sessionId": scope_id }))
        .await
        .unwrap();
    assert_eq!(res["entities"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_delete_observations() {
    let _l = test_lock().lock().await;
    let scope_id = unique_scope("test_delobs");

    CreateEntitiesTool.call(&json!({
        "entities": [{ "name": "ObsTarget", "entityType": "Test", "observations": ["keep me", "delete me"] }],
        "sessionId": scope_id
    })).await.unwrap();

    DeleteObservationsTool
        .call(&json!({
            "deletions": [{ "entityName": "ObsTarget", "observations": ["delete me"] }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    let res = OpenNodesTool
        .call(&json!({ "names": ["ObsTarget"], "sessionId": scope_id }))
        .await
        .unwrap();
    let obs = res["entities"][0]["observations"].as_array().unwrap();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0], "keep me");
}

#[tokio::test]
async fn test_branch_commit_rollback() {
    let _l = test_lock().lock().await;
    let scope_id = unique_scope("test_branch");
    let branch_id = format!("br_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    // Create entity in main
    CreateEntitiesTool
        .call(&json!({
            "entities": [{ "name": "MainEntity", "entityType": "Test", "observations": [] }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    // Create branch
    CreateDatabaseBranchTool
        .call(&json!({ "branchId": branch_id }))
        .await
        .unwrap();

    // Add entity in branch
    CreateEntitiesTool
        .call(&json!({
            "entities": [{ "name": "BranchEntity", "entityType": "Test", "observations": [] }],
            "sessionId": scope_id
        }))
        .await
        .unwrap();

    // Verify branch has both
    let res = ReadGraphTool
        .call(&json!({ "sessionId": scope_id }))
        .await
        .unwrap();
    assert_eq!(res["entities"].as_array().unwrap().len(), 2);

    // Rollback
    RollbackDatabaseBranchTool.call(&json!({})).await.unwrap();

    // Verify only main entity remains
    let res2 = ReadGraphTool
        .call(&json!({ "sessionId": scope_id }))
        .await
        .unwrap();
    assert_eq!(res2["entities"].as_array().unwrap().len(), 1);
}
