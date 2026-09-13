use super::*;

#[tokio::test]
async fn test_manage_mcp_actions() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("openz_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let tool = ManageMcpTool;

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async move {
            // Test action: add
            let add_args = json!({
                "action": "add",
                "name": "test-mcp",
                "command": "node",
                "args": ["test-server.js"],
                "enabled": true
            });
            let res = tool.call(&add_args).await?;
            assert_eq!(res["status"], "success");

            // Test action: list
            let list_args = json!({
                "action": "list"
            });
            let res = tool.call(&list_args).await?;
            assert_eq!(res["status"], "success");
            assert!(res["mcp_servers"]["test-mcp"].is_object());
            assert_eq!(res["mcp_servers"]["test-mcp"]["command"], "node");
            assert_eq!(res["mcp_servers"]["test-mcp"]["enabled"], true);

            // Test action: disable
            let disable_args = json!({
                "action": "disable",
                "name": "test-mcp"
            });
            let res = tool.call(&disable_args).await?;
            assert_eq!(res["status"], "success");

            // Verify disabled state via list
            let res = tool.call(&list_args).await?;
            assert_eq!(res["mcp_servers"]["test-mcp"]["enabled"], false);

            // Test action: enable
            let enable_args = json!({
                "action": "enable",
                "name": "test-mcp"
            });
            let res = tool.call(&enable_args).await?;
            assert_eq!(res["status"], "success");

            // Verify enabled state via list
            let res = tool.call(&list_args).await?;
            assert_eq!(res["mcp_servers"]["test-mcp"]["enabled"], true);

            // Test action: remove (requires confirmation)
            let remove_args = json!({
                "action": "remove",
                "name": "test-mcp"
            });
            let res = tool.call(&remove_args).await?;
            assert_eq!(res["status"], "requires_confirmation");

            // Test action: remove without confirm param should fail
            let remove_args = json!({
                "action": "remove",
                "name": "test-mcp",
                "confirm": true
            });
            let res = tool.call(&remove_args).await?;
            assert_eq!(res["status"], "success");

            // Verify removed state via list
            let res = tool.call(&list_args).await?;
            assert!(!res["mcp_servers"]
                .as_object()
                .unwrap()
                .contains_key("test-mcp"));

            // Clean up
            let _ = std::fs::remove_dir_all(&temp_dir);

            Ok(())
        })
        .await
}
