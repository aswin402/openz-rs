use super::*;
use crate::tools::Tool;
use anyhow::Result;

#[test]
fn test_tool_catalog_reports_metadata_and_exposure() {
    let registry = crate::tools::ToolRegistry::new();
    registry.register(std::sync::Arc::new(crate::tools::shell::ExecCommandTool));
    registry.register(std::sync::Arc::new(crate::tools::filesystem::ReadFileTool));

    let catalog = ToolCatalogTool::new(registry);
    let res = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(catalog.call(&serde_json::json!({
            "include_schema": false,
            "prompt": "run cargo test and inspect files"
        })))
        .unwrap();

    assert_eq!(res["success"].as_bool().unwrap(), true);
    assert_eq!(res["tool_count"].as_u64().unwrap(), 2);
    assert_eq!(res["exposed_count"].as_u64().unwrap(), 2);

    let tools = res["tools"].as_array().unwrap();
    let exec = tools
        .iter()
        .find(|tool| tool["name"].as_str() == Some("exec_command"))
        .expect("exec_command entry");
    assert_eq!(exec["domain"].as_str().unwrap(), "shell");
    assert_eq!(exec["risk"].as_str().unwrap(), "high");
    assert_eq!(exec["spawns_process"].as_bool().unwrap(), true);
    assert_eq!(exec["requires_approval"].as_bool().unwrap(), true);
    assert_eq!(exec["matched_prompt_domain"].as_bool().unwrap(), true);
    assert!(exec["selection_reason"]
        .as_str()
        .unwrap()
        .contains("prompt_domain"));
    assert!(exec["selected_score"].as_i64().unwrap() > 0);
    assert!(exec["aliases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|alias| alias.as_str() == Some("shell command")));
    assert!(exec["examples"].as_array().unwrap().iter().any(|example| {
        example
            .as_str()
            .unwrap_or("")
            .contains("safe project-local command")
    }));
    assert!(exec["when_to_use"]
        .as_str()
        .unwrap()
        .contains("shell commands"));
    assert!(exec["when_not_to_use"]
        .as_str()
        .unwrap()
        .contains("file reads"));

    let selected_domains = res["selected_domains"].as_array().unwrap();
    assert!(selected_domains.iter().any(|d| d.as_str() == Some("shell")));

    let read_file = tools
        .iter()
        .find(|tool| tool["name"].as_str() == Some("read_file"))
        .expect("read_file entry");
    assert_eq!(read_file["domain"].as_str().unwrap(), "filesystem");
    assert_eq!(read_file["risk"].as_str().unwrap(), "low");
    assert_eq!(read_file["writes_disk"].as_bool().unwrap(), false);
}

#[test]
fn test_tool_catalog_reports_resource_policy_visibility() {
    struct NetworkTool;
    #[async_trait::async_trait]
    impl Tool for NetworkTool {
        fn name(&self) -> &str {
            "web_fetch"
        }
        fn description(&self) -> &str {
            "Fetch a web page"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
            Ok(serde_json::json!({"ok": true}))
        }
    }

    let registry = crate::tools::ToolRegistry::new();
    registry.register(std::sync::Arc::new(NetworkTool));

    let catalog = ToolCatalogTool::new(registry);
    let res = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(catalog.call(&serde_json::json!({
            "prompt": "fetch this webpage",
            "resource_overrides": {
                "allow_network_tools": false,
                "free_disk_gb": 100.0,
                "active_process_tools": 0
            }
        })))
        .unwrap();

    let tools = res["tools"].as_array().unwrap();
    let web_fetch = tools
        .iter()
        .find(|tool| tool["name"].as_str() == Some("web_fetch"))
        .expect("web_fetch entry");

    assert_eq!(
        web_fetch["resource_policy"]["decision"].as_str(),
        Some("block")
    );
    assert!(web_fetch["resource_policy"]["reason"]
        .as_str()
        .unwrap()
        .contains("Network tools are disabled"));
    assert_eq!(
        web_fetch["resource_policy"]["free_disk_gb"].as_f64(),
        Some(100.0)
    );
    assert_eq!(
        web_fetch["resource_policy"]["active_process_tools"].as_u64(),
        Some(0)
    );
}
