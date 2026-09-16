use super::*;

#[tokio::test]
async fn websocket_approval_rejects_wrong_client_or_chat() {
    let req_id = format!("approval-test-{}", uuid::Uuid::new_v4());
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    register_ws_approval(
        req_id.clone(),
        WsApprovalContext {
            client_id: "client-a".to_string(),
            chat_id: "ws-chat-a".to_string(),
        },
        tx,
    );

    assert!(!resolve_ws_approval(&req_id, "client-b", "ws-chat-a", true));
    assert!(rx.try_recv().is_err(), "mismatched client consumed approval");

    assert!(!resolve_ws_approval(&req_id, "client-a", "ws-chat-b", true));
    assert!(rx.try_recv().is_err(), "mismatched chat consumed approval");

    assert!(resolve_ws_approval(&req_id, "client-a", "ws-chat-a", true));
    assert_eq!(rx.await.unwrap(), true);
}

#[tokio::test]
async fn websocket_approval_event_targets_only_requesting_client() {
    let client_id = format!("approval-target-{}", uuid::Uuid::new_v4());
    let other_client_id = format!("approval-other-{}", uuid::Uuid::new_v4());
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let (other_tx, mut other_rx) = tokio::sync::mpsc::channel(8);
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .insert(client_id.clone(), tx);
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .insert(other_client_id.clone(), other_tx);

    assert!(crate::channels::websocket::publish_ws_event_to_client(
        &client_id,
        serde_json::json!({ "event": "security_request", "req_id": "req-1" }),
    ));

    let mut received_target = false;
    while let Ok(msg) = rx.try_recv() {
        if let axum::extract::ws::Message::Text(text) = msg {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if v.get("event").and_then(|s| s.as_str()) == Some("security_request")
                    && v.get("req_id").and_then(|s| s.as_str()) == Some("req-1")
                {
                    received_target = true;
                }
            }
        }
    }
    assert!(received_target, "target client should receive security_request event");

    while let Ok(msg) = other_rx.try_recv() {
        if let axum::extract::ws::Message::Text(text) = msg {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                assert_ne!(
                    v.get("event").and_then(|s| s.as_str()),
                    Some("security_request"),
                    "other client should never receive targeted security_request event"
                );
            }
        }
    }

    crate::channels::websocket::events::remove_active_ws_sender(&client_id);
    crate::channels::websocket::events::remove_active_ws_sender(&other_client_id);
}

#[tokio::test]
async fn websocket_approval_cancels_on_client_disconnect() {
    let req_id = format!("approval-disconnect-{}", uuid::Uuid::new_v4());
    let (tx, rx) = tokio::sync::oneshot::channel();
    register_ws_approval(
        req_id.clone(),
        WsApprovalContext {
            client_id: "client-disconnect".to_string(),
            chat_id: "ws-chat".to_string(),
        },
        tx,
    );

    cancel_ws_approvals_for_client("client-disconnect");

    assert_eq!(rx.await.unwrap(), false);
    assert!(!resolve_ws_approval(&req_id, "client-disconnect", "ws-chat", true));
}

#[test]
fn websocket_approval_rejection_event_is_safe_and_targeted() {
    let event = crate::channels::websocket::security_response_rejected_event("req-1", "ws-chat-a");
    assert_eq!(event["event"], "security_response_rejected");
    assert_eq!(event["req_id"], "req-1");
    assert_eq!(event["chat_id"], "ws-chat-a");
    assert!(event["detail"].as_str().unwrap().contains("pending"));
    assert!(!event.to_string().contains("tool_name"));
}
