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
            assert_eq!(event["archive_path"].as_str().unwrap().contains("archive"), true);
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

#[tokio::test]
async fn webui_sessions_list_isolates_only_ws_and_web_sessions() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_ws_isolation_session_test_{}",
        uuid::Uuid::new_v4()
    ));
    let sessions_dir = temp_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let manager = crate::session::SessionManager::new(sessions_dir);

    let mut tui_session = crate::session::Session::new("cli:direct");
    tui_session.add_message("user", "hello from tui");
    manager.save(&tui_session).await.unwrap();

    let mut tg_session = crate::session::Session::new("telegram:998877");
    tg_session.add_message("user", "hello from telegram");
    manager.save(&tg_session).await.unwrap();

    let mut web_session = crate::session::Session::new("ws:chat_101");
    web_session.add_message("user", "hello from webui");
    manager.save(&web_session).await.unwrap();

    let list_evt = fetch_real_sessions_list(&manager).await;
    let sessions = list_evt["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], "ws:chat_101");

    assert_eq!(ensure_webui_session_key("cli:direct"), "ws:cli_direct");
    assert_eq!(ensure_webui_session_key("telegram:998877"), "ws:telegram_998877");
    assert_eq!(ensure_webui_session_key("ws:chat_101"), "ws:chat_101");

    let _ = std::fs::remove_dir_all(temp_dir);
}
