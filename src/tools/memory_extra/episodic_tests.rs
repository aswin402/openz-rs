use super::*;
use crate::tools::graph_memory::test_lock;
use crate::tools::Tool;
use serde_json::json;

#[tokio::test]
async fn test_log_and_retrieve_reflections() {
    let _l = test_lock().lock().await;
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
    let _l = test_lock().lock().await;
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
async fn test_record_query_tool_performance() {
    let _l = test_lock().lock().await;
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
