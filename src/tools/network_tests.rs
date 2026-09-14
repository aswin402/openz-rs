use super::*;

#[tokio::test]
async fn test_check_port() -> Result<()> {
    let tool = CheckPortTool;
    let res = tool
        .call(&json!({
            "port": 58291,
            "action": "check_free"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["available"].as_bool().is_some());
    Ok(())
}
