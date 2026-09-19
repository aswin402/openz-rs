use super::*;

#[tokio::test]
async fn test_file_watcher_status() -> Result<()> {
    let tool = FileWatcherTool;
    let res = tool
        .call(&json!({
            "action": "status"
        }))
        .await?;

    assert_eq!(res["status"], "inactive");
    Ok(())
}

#[tokio::test]
async fn test_file_watcher_direct_string_and_aliases() -> Result<()> {
    let tool = FileWatcherTool;

    // Direct string status
    let res = tool.call(&json!("status")).await?;
    assert_eq!(res["status"], "inactive");

    // Direct string stop
    let res2 = tool.call(&json!("stop")).await?;
    assert_eq!(res2["status"], "success");

    // Alias mode: query
    let res3 = tool.call(&json!({ "mode": "query" })).await?;
    assert_eq!(res3["status"], "inactive");

    Ok(())
}
