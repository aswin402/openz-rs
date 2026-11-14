use super::*;

#[test]
fn websocket_request_id_is_trimmed_and_bounded() {
    assert_eq!(
        ws_request_id(&serde_json::json!({"request_id": " req-7 "})),
        Some("req-7".to_string())
    );
    assert_eq!(ws_request_id(&serde_json::json!({"request_id": ""})), None);
    let oversized = "x".repeat(MAX_WS_REQUEST_ID_LEN + 1);
    assert_eq!(
        ws_request_id(&serde_json::json!({"request_id": oversized})),
        None
    );
}

#[test]
fn websocket_command_ack_has_stable_wire_shape() {
    let ack = command_ack_event("req-7", "get_status", "accepted", None);
    assert_eq!(ack["event"], "command_ack");
    assert_eq!(ack["request_id"], "req-7");
    assert_eq!(ack["command"], "get_status");
    assert_eq!(ack["status"], "accepted");
}

#[test]
fn websocket_commands_use_typed_session_controls_without_slash_commands() {
    assert!(is_known_ws_command("stop_turn"));
    assert!(is_known_ws_command("clear_session"));
    assert!(!is_known_ws_command("get_slash_commands"));
}
