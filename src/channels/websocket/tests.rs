use super::*;

#[test]
fn attachment_policy_rejects_unsafe_mime_and_aggregate_overflow() {
    assert!(attachment_mime_allowed("image/png"));
    assert!(attachment_mime_allowed("application/pdf"));
    assert!(!attachment_mime_allowed("application/x-sh"));
    assert!(attachment_total_within_quota(0, MAX_ATTACHMENT_BYTES));
    assert!(!attachment_total_within_quota(
        MAX_ATTACHMENT_TOTAL_BYTES - 1,
        2,
    ));
}

#[test]
fn webui_capabilities_include_runtime_policy() {
    let config = crate::config::schema::Config::default();
    let capabilities = webui_capabilities(&config);
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

#[tokio::test]
async fn websocket_turn_stop_is_scoped_to_owner() {
    let turn_id = format!("turn-test-{}", uuid::Uuid::new_v4());
    let token = crate::tools::subagent::CancellationToken::new();
    register_ws_turn(
        turn_id.clone(),
        "client-a".to_string(),
        "ws-chat-a".to_string(),
        token.clone(),
    );

    assert!(!cancel_ws_turn(&turn_id, "client-b", "ws-chat-a"));
    assert!(!token.is_cancelled());
    assert!(!cancel_ws_turn(&turn_id, "client-a", "ws-chat-b"));
    assert!(!token.is_cancelled());

    assert!(cancel_ws_turn(&turn_id, "client-a", "ws-chat-a"));
    assert!(token.is_cancelled());
    assert!(!cancel_ws_turn(&turn_id, "client-a", "ws-chat-a"));
}

#[tokio::test]
async fn websocket_turn_stop_does_not_cancel_another_client() {
    let first_turn = format!("turn-first-{}", uuid::Uuid::new_v4());
    let second_turn = format!("turn-second-{}", uuid::Uuid::new_v4());
    let first_token = crate::tools::subagent::CancellationToken::new();
    let second_token = crate::tools::subagent::CancellationToken::new();
    register_ws_turn(
        first_turn.clone(),
        "client-a".to_string(),
        "ws-chat".to_string(),
        first_token.clone(),
    );
    register_ws_turn(
        second_turn.clone(),
        "client-b".to_string(),
        "ws-chat".to_string(),
        second_token.clone(),
    );

    assert!(cancel_ws_turn(&first_turn, "client-a", "ws-chat"));
    assert!(first_token.is_cancelled());
    assert!(!second_token.is_cancelled());

    cancel_ws_turn(&second_turn, "client-b", "ws-chat");
}

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
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let (other_tx, mut other_rx) = tokio::sync::mpsc::channel(1);
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .insert(client_id.clone(), tx);
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .insert(other_client_id.clone(), other_tx);

    assert!(publish_ws_event_to_client(
        &client_id,
        serde_json::json!({ "event": "security_request", "req_id": "req-1" }),
    ));
    assert!(rx.try_recv().is_ok());
    assert!(other_rx.try_recv().is_err());

    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .remove(&client_id);
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .remove(&other_client_id);
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
    let event = security_response_rejected_event("req-1", "ws-chat-a");
    assert_eq!(event["event"], "security_response_rejected");
    assert_eq!(event["req_id"], "req-1");
    assert_eq!(event["chat_id"], "ws-chat-a");
    assert!(event["detail"].as_str().unwrap().contains("pending"));
    assert!(!event.to_string().contains("tool_name"));
}

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn orchestration_lifecycle_events_survive_queue_pressure() {
    let client_id = format!("test-orchestration-{}", uuid::Uuid::new_v4());
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .insert(client_id.clone(), tx);

    let publisher = tokio::spawn(async {
        super::publish_ws_event(serde_json::json!({"event": "progress"}));
        for payload in [
            serde_json::json!({"type": "run_started", "run_id": "run-1"}),
            serde_json::json!({"type": "step_started", "run_id": "run-1"}),
            serde_json::json!({"type": "step_finished", "run_id": "run-1"}),
            serde_json::json!({"type": "run_finished", "run_id": "run-1"}),
        ] {
            super::publish_orchestration_event("chat-1", payload);
        }
    });

    let mut types = Vec::new();
    for _ in 0..5 {
        let message = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
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
    crate::channels::get_active_ws_senders()
        .lock()
        .unwrap()
        .remove(&client_id);
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

#[test]
fn websocket_chat_uses_shared_stop_command_detection() {
    assert!(crate::channels::is_stop_command("/stop"));
    assert!(!crate::channels::is_stop_command("/stopwatch"));
}

#[test]
fn test_normalize_model_name() {
    assert_eq!(normalize_model_name("gpt-4o"), "openai/gpt-4o");
    assert_eq!(
        normalize_model_name("claude-3-5-sonnet"),
        "anthropic/claude-3-5-sonnet"
    );
    assert_eq!(
        normalize_model_name("deepseek-chat"),
        "deepseek/deepseek-chat"
    );
    assert_eq!(normalize_model_name("custom/my-model"), "custom/my-model");
}

#[test]
fn test_determine_routed_model_complex() {
    let mut config = crate::config::schema::Config::default();
    config.agents.defaults.model = "anthropic/claude-3-5-sonnet".to_string();

    // Complex prompts should use requested or default premium
    let model =
        determine_routed_model(&config, "gpt-4o", "Please fix this error in my rust code");
    assert_eq!(model, "gpt-4o");

    let model_fallback = determine_routed_model(
        &config,
        "some-random-model",
        "Please design a new database schema for a blog",
    );
    assert_eq!(model_fallback, "anthropic/claude-3-5-sonnet");
}

#[test]
fn test_determine_routed_model_simple_fallback() {
    let mut config = crate::config::schema::Config::default();
    config.agents.defaults.model = "anthropic/claude-3-5-sonnet".to_string();

    // Simple prompt with env vars -> routes to cheapest available provider
    let _model = determine_routed_model(&config, "gpt-4o", "Hello!");

    // Simple prompt, deepseek key set -> should route to deepseek-chat
    config.providers.deepseek = Some(crate::config::schema::ProviderConfig {
        api_key: Some("test-key".to_string()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: std::collections::HashMap::new(),
    });
    let model_routed = determine_routed_model(&config, "gpt-4o", "Hi there");
    assert_eq!(model_routed, "deepseek/deepseek-chat");
}

#[test]
fn gateway_token_required_for_host_rejects_public_binds() {
    assert!(!gateway_token_required_for_host("127.0.0.1"));
    assert!(!gateway_token_required_for_host("localhost"));
    assert!(!gateway_token_required_for_host("::1"));
    assert!(gateway_token_required_for_host("0.0.0.0"));
    assert!(gateway_token_required_for_host("192.168.1.10"));
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

#[test]
fn websocket_origin_validation_rejects_untrusted_browser_origins() {
    let config = WebSocketChannelConfig {
        enabled: true,
        host: "127.0.0.1".to_string(),
        port: 8765,
        start_on_boot: false,
        start_on_tui: false,
    };

    assert!(websocket_origin_allowed(Some("http://127.0.0.1:8765"), &config, false));
    assert!(websocket_origin_allowed(Some("http://localhost:5173"), &config, false));
    assert!(!websocket_origin_allowed(Some("https://evil.example"), &config, false));
    assert!(!websocket_origin_allowed(None, &config, false));
    assert!(websocket_origin_allowed(None, &config, true));
}

#[test]
fn websocket_cors_origins_include_configured_and_vite_origins() {
    let config = WebSocketChannelConfig {
        enabled: true,
        host: "192.168.1.20".to_string(),
        port: 9000,
        start_on_boot: false,
        start_on_tui: false,
    };

    let origins = websocket_cors_origins(&config);
    assert!(origins.iter().any(|origin| origin == "http://192.168.1.20:9000"));
    assert!(origins.iter().any(|origin| origin == "https://192.168.1.20:9000"));
    assert!(origins.iter().any(|origin| origin == "http://localhost:5173"));
    assert!(origins.iter().any(|origin| origin == "http://127.0.0.1:5173"));
}

#[test]
fn test_is_authorized() {
    use axum::http::HeaderMap;

    // Unset token -> open access (allow all)
    std::env::remove_var("OPENZ_GATEWAY_TOKEN");
    let headers = HeaderMap::new();
    assert!(is_authorized(&headers, None));
    assert!(is_authorized(&headers, Some("test")));

    // Empty token -> open access (allow all)
    std::env::set_var("OPENZ_GATEWAY_TOKEN", "");
    assert!(is_authorized(&headers, None));
    assert!(is_authorized(&headers, Some("")));

    // Set token -> verify query token and header
    std::env::set_var("OPENZ_GATEWAY_TOKEN", "super-secret-token");
    assert!(!is_authorized(&headers, None));
    assert!(!is_authorized(&headers, Some("wrong-token")));
    assert!(is_authorized(&headers, Some("super-secret-token")));

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        axum::http::HeaderValue::from_static("Bearer super-secret-token"),
    );
    assert!(is_authorized(&headers, None));

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        axum::http::HeaderValue::from_static("Bearer wrong-token"),
    );
    assert!(!is_authorized(&headers, None));

    // Clean up
    std::env::remove_var("OPENZ_GATEWAY_TOKEN");
}

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
                config: config.channels.websocket.clone().unwrap_or(WebSocketChannelConfig {
                    enabled: true,
                    host: "127.0.0.1".to_string(),
                    port: 8765,
                    start_on_boot: false,
                    start_on_tui: false,
                }),
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

#[test]
fn websocket_protocol_events_serialize_safely() {
    let ready_val = protocol::ready("chat-1", "client-1");
    assert_eq!(ready_val["event"], "ready");
    assert_eq!(ready_val["chat_id"], "chat-1");
    assert_eq!(ready_val["client_id"], "client-1");

    let delta_val = protocol::delta("chat-1", Some("turn-1".to_string()), "hello world");
    assert_eq!(delta_val["event"], "delta");
    assert_eq!(delta_val["chat_id"], "chat-1");
    assert_eq!(delta_val["turn_id"], "turn-1");
    assert_eq!(delta_val["content"], "hello world");

    let status_val = protocol::gateway_status(5, 1, 6);
    assert_eq!(status_val["event"], "status");
    assert_eq!(status_val["mcp"]["loaded"], 5);

    let notif_val = protocol::notification("system alert");
    assert_eq!(notif_val["event"], "notification");
    assert_eq!(notif_val["message"], "system alert");
}

