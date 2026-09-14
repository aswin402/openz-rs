use super::*;

#[tokio::test]
async fn test_obscura_browser_tool_metadata() -> Result<()> {
    let tool = ObscuraBrowserTool::new();
    assert_eq!(tool.name(), "obscura_browser");
    let params = tool.parameters();
    assert!(params.get("properties").is_some());
    Ok(())
}
