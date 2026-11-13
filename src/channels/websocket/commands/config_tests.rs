use super::*;

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
            let state = crate::channels::websocket::WsState {
                config: config.channels.websocket.clone().unwrap_or_default(),
                agent_loop: dummy_agent_loop,
                live_config: live_config.clone(),
                _config_watcher: std::sync::Arc::new(None),
            };

            let (tx, mut rx) = tokio::sync::mpsc::channel(10);
            let (other_tx, mut other_rx) = tokio::sync::mpsc::channel(10);

            let other_client_id = format!("other-client-{}", uuid::Uuid::new_v4());
            let other_chat_id = format!("other-chat-{}", uuid::Uuid::new_v4());

            crate::channels::get_active_ws_senders()
                .lock()
                .unwrap()
                .insert(other_client_id.clone(), other_tx);
            crate::channels::get_active_ws_client_chats()
                .lock()
                .unwrap()
                .insert(other_client_id.clone(), other_chat_id);

            let update_envelope = serde_json::json!({
                "defaults": {
                    "model": "gpt-4o-mini",
                    "temperature": 0.7,
                }
            });

            let handled = crate::channels::websocket::commands::handle_post_message_command(
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

            let msg = rx
                .recv()
                .await
                .expect("calling client should receive update event");
            let text = match msg {
                axum::extract::ws::Message::Text(t) => t,
                _ => panic!("expected text message"),
            };
            assert!(text.contains("config_updated"));

            let other_msg = other_rx
                .recv()
                .await
                .expect("other client should receive broadcast");
            let other_text = match other_msg {
                axum::extract::ws::Message::Text(t) => t,
                _ => panic!("expected text message"),
            };
            assert!(other_text.contains("config_updated"));

            crate::channels::websocket::events::remove_active_ws_sender(&other_client_id);

            let disk_cfg = crate::config::loader::load_config().unwrap();
            let webui_prof = disk_cfg.ui_profile("webui").unwrap();
            assert_eq!(webui_prof.model.as_deref(), Some("gpt-4o-mini"));
            assert_eq!(webui_prof.temperature, Some(0.7));
            assert!(disk_cfg.ui_profile("tui").is_none());
            assert_ne!(disk_cfg.agents.defaults.model, "gpt-4o-mini");
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn webui_config_update_isolates_settings_from_tui_and_core_defaults() {
    let mut disk_cfg = crate::config::schema::Config::default();
    disk_cfg.agents.defaults.model = "claude-3-5-sonnet".to_string();
    disk_cfg.agents.defaults.provider = "anthropic".to_string();
    {
        let tui = disk_cfg.ui_profile_mut("tui");
        tui.model = Some("claude-3-7-sonnet".to_string());
        tui.provider = Some("anthropic".to_string());
        tui.theme = Some("cyberpunk".to_string());
    }

    let webui_patch = serde_json::json!({
        "defaults": {
            "model": "deepseek-chat",
            "provider": "deepseek",
            "theme": "light",
            "temperature": 0.6,
            "streaming": false,
            "caveman_mode": false
        }
    });

    apply_webui_disk_config_update(&mut disk_cfg, &webui_patch);

    assert_eq!(disk_cfg.agents.defaults.model, "claude-3-5-sonnet");
    assert_eq!(disk_cfg.agents.defaults.provider, "anthropic");
    let tui_prof = disk_cfg.ui_profile("tui").unwrap();
    assert_eq!(tui_prof.model.as_deref(), Some("claude-3-7-sonnet"));
    assert_eq!(tui_prof.theme.as_deref(), Some("cyberpunk"));

    let webui_prof = disk_cfg.ui_profile("webui").unwrap();
    assert_eq!(webui_prof.model.as_deref(), Some("deepseek-chat"));
    assert_eq!(webui_prof.provider.as_deref(), Some("deepseek"));
    assert_eq!(webui_prof.theme.as_deref(), Some("light"));
    assert_eq!(webui_prof.streaming, Some(false));
    assert_eq!(webui_prof.caveman_mode, Some(false));

    let tui_effective = disk_cfg.with_ui_surface("tui");
    let webui_effective = disk_cfg.with_ui_surface("webui");
    assert_eq!(tui_effective.agents.defaults.model, "claude-3-7-sonnet");
    assert_eq!(tui_effective.ui_theme("tui"), "cyberpunk");
    assert_eq!(webui_effective.agents.defaults.model, "deepseek-chat");
    assert_eq!(webui_effective.ui_theme("webui"), "light");
}

#[tokio::test]
async fn websocket_gateway_autostart_settings_persist_and_gate_non_loopback_host() {
    let mut disk_cfg = crate::config::schema::Config::default();

    let loopback_autostart = serde_json::json!({
        "channels": {
            "websocket": {
                "enabled": true,
                "start_on_tui": true,
                "start_on_boot": true,
                "host": "127.0.0.1",
                "port": 8765
            }
        }
    });
    assert!(!config_update_requires_gateway_token(&loopback_autostart));

    let remote_bind = serde_json::json!({
        "channels": {
            "websocket": {
                "enabled": true,
                "host": "0.0.0.0",
                "port": 8765
            }
        }
    });
    assert!(config_update_requires_gateway_token(&remote_bind));

    apply_webui_disk_config_update(&mut disk_cfg, &loopback_autostart);
    let ws = disk_cfg.channels.websocket.as_ref().expect("websocket config");
    assert!(ws.enabled);
    assert!(ws.start_on_tui);
    assert!(ws.start_on_boot);
    assert_eq!(ws.host, "127.0.0.1");
    assert_eq!(ws.port, 8765);

    let event = config_data_event(&disk_cfg).await;
    assert_eq!(event["channels"]["websocket"]["start_on_tui"], true);
    assert_eq!(event["channels"]["websocket"]["start_on_boot"], true);
    let has_ws_capability = event["capabilities"]["channels"]
        .as_array()
        .map(|items| items.iter().any(|item| item["name"] == "websocket"))
        .unwrap_or(false);
    assert!(has_ws_capability);
}

#[tokio::test]
async fn webui_theme_and_ui_customization_roundtrip_without_gateway_token() {
    let mut disk_cfg = crate::config::schema::Config::default();

    // Fresh config has no explicit WebUI theme yet
    let initial_event = config_data_event(&disk_cfg).await;
    assert!(initial_event["defaults"]["theme_explicit"].is_null());

    let ui_patch = serde_json::json!({
        "defaults": {
            "theme": "light",
            "active_theme_id": "catppuccin",
            "ui_customization": {
                "activeThemeId": "catppuccin",
                "chatWidth": "wide",
                "approvalStyle": "beui-tool-approval"
            }
        }
    });
    assert!(!config_update_requires_gateway_token(&ui_patch));

    apply_webui_disk_config_update(&mut disk_cfg, &ui_patch);
    let updated_event = config_updated_event(&disk_cfg);
    assert_eq!(updated_event["defaults"]["theme"], "light");
    assert_eq!(updated_event["defaults"]["theme_explicit"], "light");
    assert_eq!(updated_event["defaults"]["active_theme_id"], "catppuccin");
    assert_eq!(
        updated_event["defaults"]["ui_customization"]["chatWidth"],
        "wide"
    );
}
