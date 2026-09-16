use super::*;

#[tokio::test]
async fn websocket_archive_session_moves_persisted_session() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_ws_archive_session_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let sessions_dir = temp_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let manager = crate::session::SessionManager::new(sessions_dir.clone());
    let mut session = crate::session::Session::new("ws:control");
    session.add_message("user", "archive from control center");
    manager.save(&session).await.unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let event = archive_session_event(
                &manager,
                &serde_json::json!({ "chat_id": "ws_control" }),
            )
            .await;

            assert_eq!(event["event"], "session_archived");
            assert_eq!(event["status"], "success");
            assert_eq!(event["session_key"], "ws:control");
            assert_eq!(event["archived"], true);
            assert!(event["archive_path"].as_str().unwrap().contains("archive"));
            assert!(manager.load("ws:control").is_err());
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn websocket_delete_session_removes_persisted_session() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_ws_delete_session_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let sessions_dir = temp_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let manager = crate::session::SessionManager::new(sessions_dir.clone());
    let mut session = crate::session::Session::new("ws:control");
    session.add_message("user", "hello from control center");
    manager.save(&session).await.unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let event =
                delete_session_event(&manager, &serde_json::json!({ "chat_id": "ws_control" }))
                    .await;

            assert_eq!(event["event"], "session_deleted");
            assert_eq!(event["status"], "success");
            assert_eq!(event["session_key"], "ws:control");
            assert_eq!(event["deleted"], true);
            assert!(manager.load("ws:control").is_err());
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}
