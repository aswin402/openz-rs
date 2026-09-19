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

#[tokio::test]
async fn test_check_port_direct_primitives() -> Result<()> {
    let tool = CheckPortTool;

    // Direct number port
    let res = tool.call(&json!(58294)).await?;
    assert_eq!(res["status"], "success");
    assert_eq!(res["port"], 58294);
    assert!(res["listening"].as_bool().is_some());

    // Direct address string
    let res = tool.call(&json!("127.0.0.1:58295")).await?;
    assert_eq!(res["status"], "success");
    assert_eq!(res["port"], 58295);
    assert_eq!(res["host"], "127.0.0.1");
    assert!(res["listening"].as_bool().is_some());

    // Object with target containing port and alias mode
    let res = tool.call(&json!({
        "target": "127.0.0.1:58296",
        "mode": "free"
    })).await?;
    assert_eq!(res["status"], "success");
    assert_eq!(res["port"], 58296);
    assert!(res["available"].as_bool().is_some());

    Ok(())
}

