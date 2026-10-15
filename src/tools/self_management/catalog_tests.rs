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
    assert_eq!(
        read_file["resource_policy"]["decision"].as_str(),
        Some("allow")
    );
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

#[tokio::test]
async fn test_tool_catalog_bm25_search_and_hot_mount() {
    let registry = crate::tools::ToolRegistry::new();
    registry.register(std::sync::Arc::new(crate::tools::shell::ExecCommandTool));
    registry.register(std::sync::Arc::new(crate::tools::filesystem::ReadFileTool));
    registry.register(std::sync::Arc::new(crate::tools::grep::GrepSearchTool));

    let catalog = ToolCatalogTool::new(registry.clone());
    let res = catalog
        .call(&serde_json::json!({
            "query": "read file and inspect contents",
            "mount": true
        }))
        .await
        .unwrap();

    assert_eq!(res["success"].as_bool().unwrap(), true);
    assert_eq!(res["mounted"].as_bool().unwrap(), true);
    let mounted = res["mounted_tools"].as_array().unwrap();
    assert!(mounted.iter().any(|v| v.as_str() == Some("read_file")));

    // Verify hot-mounting in registry pending_scope:
    let (tools, _) = registry.current_turn_scope();
    assert!(tools.contains(&"read_file".to_string()));
}

#[tokio::test]
async fn test_tool_catalog_batch_execute_runs_safe_tools_and_enforces_security() {
    struct SafeDomainTool;
    #[async_trait::async_trait]
    impl Tool for SafeDomainTool {
        fn name(&self) -> &str {
            "openmedia_inspect_svg"
        }
        fn description(&self) -> &str {
            "Inspect SVG metadata"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
            Ok(serde_json::json!({"svg_ok": true}))
        }
    }

    let temp_dir =
        std::env::temp_dir().join(format!("openz_batch_exec_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let sample_file = temp_dir.join("sample.txt");
    std::fs::write(&sample_file, "line alpha\nline beta\n").unwrap();

    let registry = crate::tools::ToolRegistry::new();
    registry.register(std::sync::Arc::new(crate::tools::filesystem::ReadFileTool));
    registry.register(std::sync::Arc::new(crate::tools::filesystem::ListDirTool));
    registry.register(std::sync::Arc::new(SafeDomainTool));
    registry.register(std::sync::Arc::new(crate::tools::shell::ExecCommandTool));

    let catalog = ToolCatalogTool::new(registry.clone());
    let res = catalog
        .call(&serde_json::json!({
            "batch_execute": [
                {
                    "tool": "read_file",
                    "arguments": {
                        "filePath": sample_file.to_str().unwrap(),
                        "include_hashes": true
                    }
                },
                {
                    "tool": "openmedia_inspect_svg",
                    "arguments": {}
                },
                {
                    "tool": "exec_command",
                    "arguments": {
                        "command": "rm -rf /"
                    }
                }
            ]
        }))
        .await
        .unwrap();

    assert_eq!(res["success"].as_bool(), Some(true));
    assert_eq!(res["batch_executed_count"].as_u64(), Some(3));
    let batch_results = res["batch_results"].as_array().unwrap();
    assert_eq!(batch_results[0]["tool"], "read_file");
    assert_eq!(batch_results[0]["status"], "success");
    assert!(batch_results[0]["output"]
        .as_str()
        .unwrap()
        .contains("line alpha"));

    assert_eq!(batch_results[1]["tool"], "openmedia_inspect_svg");
    assert_eq!(batch_results[1]["status"], "success");

    assert_eq!(batch_results[2]["tool"], "exec_command");
    assert_eq!(batch_results[2]["status"], "blocked");

    assert_eq!(res["batch_execution_mode"], "sequential");

    let (turn_scoped, _) = registry.current_turn_scope();
    assert!(turn_scoped.contains(&"read_file".to_string()));
    assert!(turn_scoped.contains(&"openmedia_inspect_svg".to_string()));

    let sticky = registry.current_sticky_tools();
    assert!(sticky.contains(&"openmedia_inspect_svg".to_string()));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_tool_catalog_batch_execute_concurrent_read_only_mode() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_batch_concurrent_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let sample_file = temp_dir.join("notes.txt");
    std::fs::write(&sample_file, "first\nsecond\n").unwrap();

    let registry = crate::tools::ToolRegistry::new();
    registry.register(std::sync::Arc::new(crate::tools::filesystem::ReadFileTool));
    registry.register(std::sync::Arc::new(crate::tools::filesystem::ListDirTool));

    let catalog = ToolCatalogTool::new(registry);
    let res = catalog
        .call(&serde_json::json!({
            "batch_execute": [
                {
                    "tool": "read_file",
                    "arguments": {
                        "path": sample_file.to_str().unwrap(),
                        "include_hashes": true
                    }
                },
                {
                    "tool": "list_dir",
                    "arguments": {
                        "path": temp_dir.to_str().unwrap()
                    }
                }
            ]
        }))
        .await
        .unwrap();

    assert_eq!(res["success"].as_bool(), Some(true));
    assert_eq!(res["batch_executed_count"].as_u64(), Some(2));
    assert_eq!(res["batch_execution_mode"], "concurrent");
    assert_eq!(res["tools_omitted"].as_bool(), Some(true));
    assert!(res.get("tools").is_none());
    let batch_results = res["batch_results"].as_array().unwrap();
    assert_eq!(batch_results[0]["tool"], "read_file");
    assert_eq!(batch_results[0]["status"], "success");
    assert_eq!(batch_results[1]["tool"], "list_dir");
    assert_eq!(batch_results[1]["status"], "success");

    let combined = catalog
        .call(&serde_json::json!({
            "query": "read file",
            "batch_execute": [
                {
                    "tool": "read_file",
                    "arguments": {
                        "path": sample_file.to_str().unwrap()
                    }
                }
            ]
        }))
        .await
        .unwrap();
    assert!(combined.get("tools").and_then(|v| v.as_array()).is_some());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
