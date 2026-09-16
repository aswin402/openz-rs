use super::*;

#[test]
fn webui_capabilities_include_runtime_policy() {
    let config = crate::config::schema::Config::default();
    let capabilities = crate::channels::websocket::webui_capabilities(&config);
    assert_eq!(capabilities["version"], 1);
    assert!(capabilities["securityModes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|mode| mode["value"] == "normal"));
    assert!(capabilities["providers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|provider| provider["name"] == "anthropic"));
    assert!(capabilities["channels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|channel| channel["name"] == "telegram"));
    assert_eq!(capabilities["attachments"]["maxCount"], 8);
    assert_eq!(capabilities["attachments"]["maxFileBytes"], 8 * 1024 * 1024);
    assert_eq!(capabilities["attachments"]["maxTotalBytes"], 24 * 1024 * 1024);
}

#[test]
fn websocket_protocol_events_serialize_safely() {
    let ready_val = ready("chat-1", "client-1");
    assert_eq!(ready_val["event"], "ready");
    assert_eq!(ready_val["chat_id"], "chat-1");
    assert_eq!(ready_val["client_id"], "client-1");

    let delta_val = delta("chat-1", Some("turn-1".to_string()), "hello world");
    assert_eq!(delta_val["event"], "delta");
    assert_eq!(delta_val["chat_id"], "chat-1");
    assert_eq!(delta_val["turn_id"], "turn-1");
    assert_eq!(delta_val["content"], "hello world");

    let status_val = gateway_status(5, 1, 6);
    assert_eq!(status_val["event"], "status");
    assert_eq!(status_val["mcp"]["loaded"], 5);

    let notif_val = notification("system alert");
    assert_eq!(notif_val["event"], "notification");
    assert_eq!(notif_val["message"], "system alert");

    let progress_val = tool_progress("chat-1", Some("turn-1".to_string()), "call-1", "web_fetch", "Fetching https://example.com...");
    assert_eq!(progress_val["event"], "tool_progress");
    assert_eq!(progress_val["chat_id"], "chat-1");
    assert_eq!(progress_val["turn_id"], "turn-1");
    assert_eq!(progress_val["tool_call_id"], "call-1");
    assert_eq!(progress_val["name"], "web_fetch");
    assert_eq!(progress_val["message"], "Fetching https://example.com...");
}

#[test]
fn test_tool_progress_and_activity_notice_payload_validity() {
    let chat_id = "test-chat-123";
    let turn_id = "turn-456";
    let tool_call_id = "call-789";
    let tool_name = "test_tool";
    let progress_text = "Processing chunk 1/3...";

    let tp = tool_progress(
        chat_id,
        Some(turn_id.to_string()),
        tool_call_id,
        tool_name,
        progress_text,
    );
    let an = activity_notice(
        chat_id,
        "progress",
        "Tool Progress",
        progress_text,
        1700000000,
    );

    assert_eq!(tp["event"], "tool_progress");
    assert_eq!(tp["chat_id"], chat_id);
    assert_eq!(tp["turn_id"], turn_id);
    assert_eq!(tp["tool_call_id"], tool_call_id);
    assert_eq!(tp["name"], tool_name);
    assert_eq!(tp["message"], progress_text);

    assert_eq!(an["event"], "activity_notice");
    assert_eq!(an["chat_id"], chat_id);
    assert_eq!(an["kind"], "progress");
    assert_eq!(an["title"], "Tool Progress");
    assert_eq!(an["detail"], progress_text);
    assert_eq!(an["timestamp"], 1700000000);
}
