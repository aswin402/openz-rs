use super::*;
use crate::tools::Tool;
use anyhow::Result;

#[test]
fn test_diagnose_openmedia_video_args_use_minimal_scene_for_placeholder() {
    let create_args = normalize_diagnose_mock_args(
        "openmedia_video_create",
        serde_json::json!({ "test": true }),
    );
    assert_eq!(create_args["scene"]["width"], 640);
    assert!(create_args.get("test").is_none());

    let preview_args =
        normalize_diagnose_mock_args("openmedia_video_preview", serde_json::json!({}));
    assert_eq!(preview_args["scene"]["fps"], 1);
    assert_eq!(preview_args["output_format"], "png");
}

#[test]
fn test_diagnose_openmedia_video_placeholder_is_visible() {
    let args = normalize_diagnose_mock_args("openmedia_video_create", serde_json::json!({}));
    assert_eq!(args["scene"]["background"], "#1e293b");
    assert_eq!(
        args["scene"]["scenes"][0]["elements"][0]["content"],
        "OpenZ"
    );
    assert_eq!(
        args["scene"]["scenes"][0]["elements"][0]["style"]["font_size"],
        48.0
    );
    assert_eq!(
        args["scene"]["scenes"][0]["elements"][0]["style"]["font_weight"],
        800
    );
}

#[test]
fn test_diagnose_tool() {
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

    let diagnose = DiagnoseToolTool::new(registry.clone());
    let res = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(diagnose.call(&serde_json::json!({ "tool_name": "dummy_tool" })))
        .unwrap();
    assert!(res["success"].as_bool().unwrap());
    assert_eq!(res["output"]["ok"].as_bool().unwrap(), true);
}

#[test]
fn test_diagnose_system() {
    let tool = DiagnoseSystemTool;
    let rt = tokio::runtime::Runtime::new().unwrap();

    // 1. Run diagnostic without latency checking to keep it fast
    let res = rt
        .block_on(tool.call(&serde_json::json!({
            "check_latency": false,
            "check_db_integrity": false
        })))
        .unwrap();

    assert_eq!(res["status"].as_str().unwrap(), "success");
    assert!(res["system"]["os"].as_str().is_some());
    assert!(res["system"]["architecture"].as_str().is_some());
    assert!(res["system"]["cores"].as_u64().unwrap() >= 1);

    // Verify directory statistics keys exist
    assert!(res["directories"]["sessions"].is_object());
    assert!(res["directories"]["tool_outputs"].is_object());
    assert!(res["directories"]["traces"].is_object());
    assert!(res["directories"]["skills"].is_object());

    // Verify databases status keys exist
    assert!(res["databases"]["memory"].is_object());
    assert!(res["databases"]["docs"].is_object());
    assert!(res["databases"]["graph_memory"].is_object());
    assert!(res["databases"]["ccr_cache"].is_object());
    assert!(res["databases"]["thoughts"].is_object());
}

#[test]
fn test_diagnose_system_string_boolean_coercion() {
    let tool = DiagnoseSystemTool;
    let rt = tokio::runtime::Runtime::new().unwrap();

    let res = rt
        .block_on(tool.call(&serde_json::json!({
            "check_latency": "false",
            "check_db_integrity": "false"
        })))
        .unwrap();

    assert_eq!(res["status"].as_str().unwrap(), "success");

    // Test direct string "quick"
    let res = rt.block_on(tool.call(&serde_json::json!("quick"))).unwrap();
    assert_eq!(res["status"].as_str().unwrap(), "success");

    // Test alias latency & integrity
    let res = rt
        .block_on(tool.call(&serde_json::json!({
            "latency": false,
            "integrity": false
        })))
        .unwrap();
    assert_eq!(res["status"].as_str().unwrap(), "success");
}

#[test]
fn test_diagnose_tool_direct_string() {
    let registry = crate::tools::ToolRegistry::new();
    struct DummyTool;
    #[async_trait::async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            "test_direct_tool"
        }
        fn description(&self) -> &str {
            "test_direct_tool"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
            Ok(serde_json::json!({ "called": true }))
        }
    }

    let dummy = std::sync::Arc::new(DummyTool);
    registry.register(dummy.clone());

    let diagnose = DiagnoseToolTool::new(registry.clone());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let res = rt
        .block_on(diagnose.call(&serde_json::json!("test_direct_tool")))
        .unwrap();
    assert!(res["success"].as_bool().unwrap());
    assert_eq!(res["output"]["called"].as_bool().unwrap(), true);
}
