use super::*;

#[tokio::test]
async fn test_manage_whitelist_tool() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_whitelist_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let config_path = temp_dir.join("config.json");
    let initial_config = crate::config::schema::Config::default();
    std::fs::write(
        &config_path,
        serde_json::to_string_pretty(&initial_config).unwrap(),
    )
    .unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let tool = ManageWhitelistTool;

            let add_cmd_args = json!({
                "action": "add_command",
                "value": "cargo build"
            });
            let res = tool.call(&add_cmd_args).await.unwrap();
            let prefixes = res
                .get("whitelisted_command_prefixes")
                .unwrap()
                .as_array()
                .unwrap();
            assert!(prefixes.iter().any(|v| v.as_str() == Some("cargo build")));

            let add_path_args = json!({
                "action": "add_path",
                "value": "/tmp/test_whitelist_path"
            });
            let res = tool.call(&add_path_args).await.unwrap();
            let paths = res.get("whitelisted_paths").unwrap().as_array().unwrap();
            assert!(paths
                .iter()
                .any(|v| v.as_str().unwrap().contains("test_whitelist_path")));

            let list_args = json!({
                "action": "list"
            });
            let res = tool.call(&list_args).await.unwrap();
            assert!(res.get("whitelisted_command_prefixes").is_some());
            assert!(res.get("whitelisted_paths").is_some());

            let remove_cmd_args = json!({
                "action": "remove_command",
                "value": "cargo build"
            });
            let res = tool.call(&remove_cmd_args).await.unwrap();
            let prefixes = res
                .get("whitelisted_command_prefixes")
                .unwrap()
                .as_array()
                .unwrap();
            assert!(!prefixes.iter().any(|v| v.as_str() == Some("cargo build")));

            let remove_path_args = json!({
                "action": "remove_path",
                "value": "/tmp/test_whitelist_path"
            });
            let res = tool.call(&remove_path_args).await.unwrap();
            let paths = res.get("whitelisted_paths").unwrap().as_array().unwrap();
            assert!(!paths
                .iter()
                .any(|v| v.as_str().unwrap().contains("test_whitelist_path")));
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}
