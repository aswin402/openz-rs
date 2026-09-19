use super::*;

#[tokio::test]
async fn test_onpkg_tool() -> Result<()> {
    let tool = OnpkgTool;
    let res = tool
        .call(&json!({
            "action": "doctor"
        }))
        .await?;

    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("Doctor complete"));

    // Test default action (defaults to list_stacks)
    let res = tool.call(&json!({})).await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("Available Stacks") || res["stdout"].as_str().unwrap().contains("stack"));

    // Test alias "list"
    let res = tool.call(&json!({ "action": "list" })).await?;
    assert_eq!(res["status"], "success");

    Ok(())
}

#[test]
fn test_sync_onpkg_manifest() -> Result<()> {
    let res = sync_onpkg_manifest();
    assert!(res.is_ok());
    Ok(())
}
