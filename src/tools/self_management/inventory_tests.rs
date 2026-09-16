use super::*;
use crate::tools::Tool;

#[test]
fn test_openz_inventory_reports_runtime_identity() {
    let registry = crate::tools::ToolRegistry::new();
    registry.register(std::sync::Arc::new(
        crate::tools::self_management::ManageConfigTool,
    ));

    let inventory = OpenZInventoryTool::new(registry);
    let res = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(inventory.call(&serde_json::json!({
            "include_tools": false,
            "include_subagents": false,
            "prompt": "what model are you and which language are you best at"
        })))
        .unwrap();

    assert_eq!(res["success"].as_bool().unwrap(), true);
    assert!(res["runtime_identity"].is_object());
    assert!(res["runtime_identity"]["configured_model"]
        .as_str()
        .is_some());
    assert!(res["runtime_identity"]["configured_provider"]
        .as_str()
        .is_some());
    assert!(res["runtime_identity"]["model_supports_vision"]
        .as_bool()
        .is_some());
    assert!(res["guidance"]
        .as_str()
        .unwrap()
        .contains("model/provider identity"));
}
