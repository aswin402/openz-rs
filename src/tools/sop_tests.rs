use super::*;

#[tokio::test]
async fn test_trigger_sop_tool_metadata() {
    let config = Config::default();
    let tool = TriggerSopTool { config };
    assert_eq!(tool.name(), "trigger_sop");
    assert!(tool.description().contains("Trigger a stateful"));

    let params = tool.parameters();
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["sop_id"].is_object());
}
