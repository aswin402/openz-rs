use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn orchestration_lifecycle_events_survive_queue_pressure() {
    let client_id = format!("test-orchestration-{}", uuid::Uuid::new_v4());
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .insert(client_id.clone(), tx);
    crate::channels::get_active_ws_client_chats()
        .lock()
        .unwrap()
        .insert(client_id.clone(), "chat-1".to_string());

    let publisher = tokio::spawn(async {
        publish_ws_event(serde_json::json!({"event": "progress"}));
        for payload in [
            serde_json::json!({"type": "run_started", "run_id": "run-1"}),
            serde_json::json!({"type": "step_started", "run_id": "run-1"}),
            serde_json::json!({"type": "step_finished", "run_id": "run-1"}),
            serde_json::json!({"type": "run_finished", "run_id": "run-1"}),
        ] {
            publish_orchestration_event("chat-1", payload);
        }
    });

    let mut types = Vec::new();
    while types.len() < 4 {
        let message = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("event delivery timed out")
            .expect("sender closed");
        if let axum::extract::ws::Message::Text(text) = message {
            let event: serde_json::Value = serde_json::from_str(&text).unwrap();
            if event["event"] == "orchestration_event" {
                types.push(event["payload"]["type"].as_str().unwrap().to_string());
            }
        }
    }

    publisher.await.unwrap();
    assert_eq!(
        types,
        [
            "run_started",
            "step_started",
            "step_finished",
            "run_finished"
        ]
    );
    remove_active_ws_sender(&client_id);
}

#[test]
fn orchestration_event_snapshot_matches_raw_webui_chat_id() {
    let client_id = format!("client-{}", uuid::Uuid::new_v4());
    let other_client_id = format!("client-{}", uuid::Uuid::new_v4());
    let raw_chat_id = uuid::Uuid::new_v4().to_string();
    let other_chat_id = uuid::Uuid::new_v4().to_string();
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    let (other_tx, _other_rx) = tokio::sync::mpsc::channel(1);

    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .insert(client_id.clone(), tx);
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .insert(other_client_id.clone(), other_tx);
    crate::channels::get_active_ws_client_chats()
        .lock()
        .unwrap()
        .insert(client_id.clone(), raw_chat_id.clone());
    crate::channels::get_active_ws_client_chats()
        .lock()
        .unwrap()
        .insert(other_client_id.clone(), other_chat_id);

    let matching = active_ws_sender_snapshot_for_chat(Some(&format!("ws_{raw_chat_id}")))
        .into_iter()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();

    assert_eq!(matching, vec![client_id.clone()]);

    remove_active_ws_sender(&client_id);
    remove_active_ws_sender(&other_client_id);
}

#[tokio::test]
async fn test_send_progress_update_dual_dispatches_events() {
    let client_id = format!("client-progress-{}", uuid::Uuid::new_v4());
    let chat_id = format!("ws-chat-{}", uuid::Uuid::new_v4());
    let session_key = format!("ws:{}", chat_id);

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .insert(client_id.clone(), tx);

    crate::agent::agent_loop::tool_execution::send_progress_update(&session_key, "Processing chunk 1/3...").await;

    let mut events = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let axum::extract::ws::Message::Text(text) = msg {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(event_name) = v.get("event").and_then(|s| s.as_str()) {
                    events.push((event_name.to_string(), v));
                }
            }
        }
    }

    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .remove(&client_id);

    let expected_chat_id = ws_chat_id(&session_key).unwrap();
    assert!(events.iter().any(|(ev, p)| ev == "tool_progress" && p["chat_id"] == expected_chat_id && p["message"] == "Processing chunk 1/3..."));
    assert!(events.iter().any(|(ev, p)| ev == "activity_notice" && p["chat_id"] == expected_chat_id && p["kind"] == "progress" && p["detail"] == "Processing chunk 1/3..."));
}
