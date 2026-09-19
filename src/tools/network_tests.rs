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

    // Test string port with alias action "free"
    let res = tool
        .call(&json!({
            "port": "58292",
            "action": "free"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["available"].as_bool().is_some());

    // Test default action (check_listening) with string port
    let res = tool
        .call(&json!({
            "port": "58293"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["listening"].as_bool().is_some());

    Ok(())
}

