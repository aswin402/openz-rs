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
