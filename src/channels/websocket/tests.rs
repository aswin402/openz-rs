use super::*;

#[tokio::test]
async fn websocket_cron_commands_update_inventory_and_logs() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_ws_cron_commands_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let job = crate::cron::CronJob::new(
                "job-a".to_string(),
                "10s".to_string(),
                "say hi".to_string(),
                true,
                false,
            );
            crate::cron::save_jobs_raw(&[job]).unwrap();
            let config = crate::config::schema::Config::default();

            let missing =
                cron_update_event("pause_cron_job", &serde_json::json!({}), &config, None)
                    .await;
            assert_eq!(missing["event"], "error");

            let paused = cron_update_event(
                "pause_cron_job",
                &serde_json::json!({ "id": "job-a" }),
                &config,
                None,
            )
            .await;
            assert_eq!(paused["event"], "cron_jobs_updated");
            assert_eq!(paused["status"], "paused");
            assert_eq!(paused["inventory"]["counts"]["cronJobs"], 1);
            assert_eq!(paused["inventory"]["counts"]["activeCronJobs"], 0);

            let resumed = cron_update_event(
                "resume_cron_job",
                &serde_json::json!({ "id": "job-a" }),
                &config,
                None,
            )
            .await;
            assert_eq!(resumed["event"], "cron_jobs_updated");
            assert_eq!(resumed["status"], "resumed");
            assert_eq!(resumed["inventory"]["counts"]["activeCronJobs"], 1);

            crate::cron::append_cron_run_record(&crate::cron::CronRunRecord {
                run_id: "run-a".to_string(),
                job_id: "job-a".to_string(),
                schedule: "10s".to_string(),
                started_at: "2026-08-21T00:00:00Z".to_string(),
                finished_at: Some("2026-08-21T00:00:01Z".to_string()),
                status: crate::cron::CronJobStatus::Success,
                log_path: Some("/tmp/job-a.log".to_string()),
                summary: Some("ok".to_string()),
                error: None,
            })
            .unwrap();
            let logs = cron_logs_event(&serde_json::json!({ "id": "job-a", "limit": 5 }));
            assert_eq!(logs["event"], "cron_logs");
            assert_eq!(logs["runs"].as_array().unwrap().len(), 1);
            assert_eq!(logs["runs"][0]["job_id"], "job-a");

            let deleted = cron_update_event(
                "delete_cron_job",
                &serde_json::json!({ "id": "job-a" }),
                &config,
                None,
            )
            .await;
            assert_eq!(deleted["event"], "cron_jobs_updated");
            assert_eq!(deleted["status"], "deleted");
            assert_eq!(deleted["inventory"]["counts"]["cronJobs"], 0);
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

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

#[test]
fn mask_config_secret_redacts_present_values() {
    assert_eq!(mask_config_secret(""), "");
    assert_eq!(mask_config_secret("secret"), "••••••••");
}

#[test]
fn config_update_requires_gateway_token_for_provider_api_key() {
    assert!(config_update_requires_gateway_token(&serde_json::json!({
        "type": "set_config",
        "providers": {
            "openai": { "api_key": "sk-live" }
        }
    })));

    assert!(!config_update_requires_gateway_token(&serde_json::json!({
        "type": "set_config",
        "providers": {
            "openai": { "api_key": "••••••••" }
        }
    })));
}

#[test]
fn config_update_requires_gateway_token_for_security_mode_and_workspace() {
    assert!(config_update_requires_gateway_token(&serde_json::json!({
        "defaults": { "security_mode": "auto" }
    })));
    assert!(config_update_requires_gateway_token(&serde_json::json!({
        "defaults": { "workspace": "/tmp" }
    })));
    assert!(config_update_requires_gateway_token(&serde_json::json!({
        "defaults": { "whitelisted_command_prefixes": ["cargo check"] }
    })));
}

#[test]
fn config_update_requires_gateway_token_for_channel_tokens() {
    assert!(config_update_requires_gateway_token(&serde_json::json!({
        "channels": {
            "telegram": { "bot_token": "123:abc" },
            "whatsapp": { "verify_token": "webhook-secret" }
        }
    })));
}

#[test]
fn config_update_allows_benign_preferences_without_gateway_token() {
    assert!(!config_update_requires_gateway_token(&serde_json::json!({
        "defaults": {
            "model": "openai/gpt-4o",
            "temperature": 0.2,
            "streaming": true,
            "bot_name": "OpenZ"
        },
        "providers": {
            "openai": {
                "api_base": "https://api.openai.com/v1",
                "default_model": "gpt-4o"
            }
        },
        "channels": {
            "whatsapp": {
                "enabled": false,
                "phone_number_id": "12345",
                "webhook_port": 8090
            }
        }
    })));
}

#[tokio::test]
async fn websocket_set_config_updates_live_config_and_broadcasts() {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_ws_set_config_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let config = crate::config::schema::Config::default();
            let _ = crate::config::loader::save_config(&config);

            let dummy_provider = std::sync::Arc::new(crate::providers::openai::OpenAIProvider::new(
                "key".to_string(),
                "https://api.openai.com/v1".to_string(),
                "gpt-4o".to_string(),
            ));
            let dummy_agent_loop = std::sync::Arc::new(crate::agent::agent_loop::AgentLoop {
                config: config.clone(),
                provider: dummy_provider,
                tools: crate::tools::ToolRegistry::new(),
                session_manager: crate::session::SessionManager::new(temp_dir.join("sessions")),
            });
            let live_config = std::sync::Arc::new(std::sync::RwLock::new(config.clone()));
            let state = super::WsState {
                config: config.channels.websocket.clone().unwrap_or_default(),
                agent_loop: dummy_agent_loop,
                live_config: live_config.clone(),
                _config_watcher: std::sync::Arc::new(None),
            };

            let (tx, mut rx) = tokio::sync::mpsc::channel(10);
            let (other_tx, mut other_rx) = tokio::sync::mpsc::channel(10);

            let other_client_id = format!("other-client-{}", uuid::Uuid::new_v4());
            let other_chat_id = format!("other-chat-{}", uuid::Uuid::new_v4());

            // Register another client in active senders to verify broadcast
            if let Ok(mut senders) = crate::channels::get_active_ws_senders().lock() {
                senders.insert(other_client_id.clone(), other_tx);
            }
            if let Ok(mut client_chats) = crate::channels::get_active_ws_client_chats().lock() {
                client_chats.insert(other_client_id.clone(), other_chat_id);
            }

            let update_envelope = serde_json::json!({
                "defaults": {
                    "model": "gpt-4o-mini",
                    "temperature": 0.7,
                }
            });

            let handled = super::commands::handle_post_message_command(
                "set_config",
                &update_envelope,
                "ws_chat_1",
                "calling-client",
                &state,
                &tx,
            )
            .await;

            assert!(handled);
            assert_eq!(live_config.read().unwrap().agents.defaults.model, "gpt-4o-mini");

            // Calling client receives the update
            let msg = rx.recv().await.expect("calling client should receive update event");
            let text = match msg {
                axum::extract::ws::Message::Text(t) => t,
                _ => panic!("expected text message"),
            };
            assert!(text.contains("config_updated"));

            // Other client also receives the broadcast
            let other_msg = other_rx.recv().await.expect("other client should receive broadcast");
            let other_text = match other_msg {
                axum::extract::ws::Message::Text(t) => t,
                _ => panic!("expected text message"),
            };
            assert!(other_text.contains("config_updated"));

            // Clean up sender and client chat
            remove_active_ws_sender(&other_client_id);
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}


