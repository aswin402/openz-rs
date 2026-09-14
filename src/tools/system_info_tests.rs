use super::*;

#[tokio::test]
async fn test_system_info() -> Result<()> {
    let tool = SystemInfoTool;
    let res = tool.call(&json!({})).await?;
    assert_eq!(res["status"], "success");
    assert!(res["os"].as_str().is_some());
    assert!(res["architecture"].as_str().is_some());
    Ok(())
}
