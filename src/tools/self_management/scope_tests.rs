use super::*;
use crate::tools::Tool;
use anyhow::Result;

#[test]
fn request_tool_scope_returns_structured_scope_request() {
    let tool = RequestToolScopeTool::new(crate::tools::ToolRegistry::new());
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(tool.call(&serde_json::json!({
            "reason": "need to inspect repository files",
            "needed_domains": ["code", "filesystem"],
            "needed_tools": ["grep_search", "read_file"]
        })))
        .unwrap();

    assert_eq!(result["status"], "scope_request_recorded");
    assert_eq!(result["needed_domains"][0], "code");
    assert_eq!(result["needed_tools"][0], "grep_search");
}

#[test]
fn test_optimize_tool_scope() {
    let registry = crate::tools::ToolRegistry::new();
    struct DummyTool;
    #[async_trait::async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            "dummy_tool"
        }
        fn description(&self) -> &str {
            "dummy"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
            Ok(serde_json::json!({ "ok": true }))
        }
    }

    let dummy = std::sync::Arc::new(DummyTool);
    registry.register(dummy.clone());

    let optimizer = OptimizeToolScopeTool::new(registry.clone());
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(optimizer.call(&serde_json::json!({ "active_prefixes": ["other_"] })))
        .unwrap();

    // DummyTool starts with "dummy_", which does not match prefix filter "other_"
    // It should be filtered out
    assert!(registry.get("dummy_tool").is_none());

    // Restore filter
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(optimizer.call(&serde_json::json!({ "active_prefixes": [] })))
        .unwrap();
    assert!(registry.get("dummy_tool").is_some());
}
