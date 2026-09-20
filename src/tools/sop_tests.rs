use super::*;

#[tokio::test]
async fn test_trigger_sop_tool_metadata() {
    let config = Config::default();
    let tool = TriggerSopTool { config };
    assert_eq!(tool.name(), "trigger_sop");
    assert!(tool.description().contains("Standard Operating Procedure"));

    let params = tool.parameters();
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["action"].is_object());
    assert!(params["properties"]["sop_id"].is_object());
    assert!(params["properties"]["instance_id"].is_object());
    assert_eq!(
        params["properties"]["payload"]["additionalProperties"],
        true
    );
}

#[tokio::test]
async fn test_trigger_sop_tool_list_action() {
    let config = Config::default();
    let tool = TriggerSopTool { config };
    let res = tool.call(&json!({ "action": "list" })).await.unwrap();
    assert_eq!(res["status"], "success");
    assert!(res["available_definitions"].is_array());
    assert!(res["recent_instances"].is_array());
}

#[tokio::test]
async fn test_trigger_sop_tool_missing_sop_id_for_trigger() {
    let config = Config::default();
    let tool = TriggerSopTool { config };
    let res = tool.call(&json!({ "action": "trigger" })).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Missing 'sop_id'"));
}

#[tokio::test]
async fn test_trigger_sop_tool_missing_instance_id_for_status() {
    let config = Config::default();
    let tool = TriggerSopTool { config };
    let res = tool.call(&json!({ "action": "status" })).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Missing 'instance_id'"));
}

#[tokio::test]
async fn test_trigger_sop_tool_missing_instance_id_for_output() {
    let config = Config::default();
    let tool = TriggerSopTool { config };
    let res = tool.call(&json!({ "action": "output" })).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Missing 'instance_id'"));
}

#[tokio::test]
async fn test_trigger_sop_tool_unknown_action() {
    let config = Config::default();
    let tool = TriggerSopTool { config };
    let res = tool.call(&json!({ "action": "nonexistent_action" })).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Unknown action"));
}

#[tokio::test]
async fn test_trigger_sop_tool_status_and_output_flow() {
    let config = Config::default();
    let tool = TriggerSopTool { config };

    // Create a mock instance
    let test_id = format!("test-sop-inst-{}", uuid::Uuid::new_v4());
    let inst = crate::sop::SopInstance {
        id: test_id.clone(),
        sop_id: "pr-review".to_string(),
        name: "Test PR Review".to_string(),
        status: crate::sop::SopStatus::Completed,
        current_step_index: 1,
        steps: vec![crate::sop::StepExecutionState {
            name: "analyze_diff".to_string(),
            status: "Completed".to_string(),
            started_at: Some("2026-09-21T00:00:00Z".to_string()),
            completed_at: Some("2026-09-21T00:01:00Z".to_string()),
            output: Some("Diff looks good. No security flaws.".to_string()),
            error: None,
        }],
        context: json!({ "pr_number": 42 }),
        started_at: "2026-09-21T00:00:00Z".to_string(),
        completed_at: Some("2026-09-21T00:01:00Z".to_string()),
    };

    crate::sop::save_instance(&inst).unwrap();

    // Query status
    let status_res = tool
        .call(&json!({
            "action": "status",
            "instance_id": test_id
        }))
        .await
        .unwrap();

    assert_eq!(status_res["status"], "success");
    assert_eq!(status_res["instance"]["id"], test_id);

    // Query output
    let output_res = tool
        .call(&json!({
            "action": "output",
            "instance_id": test_id
        }))
        .await
        .unwrap();

    assert_eq!(output_res["status"], "success");
    assert_eq!(output_res["instance_id"], test_id);
    assert_eq!(output_res["step_outputs"][0]["name"], "analyze_diff");
    assert_eq!(
        output_res["step_outputs"][0]["output"],
        "Diff looks good. No security flaws."
    );

    // Clean up
    let inst_file = crate::sop::sop_instances_dir().join(format!("{}.json", test_id));
    if inst_file.exists() {
        let _ = std::fs::remove_file(inst_file);
    }
}
