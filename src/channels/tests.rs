use super::notifications::{
    build_external_notification_requests, build_whatsapp_notification_requests, NotificationAuth,
};
use super::*;

#[test]
fn test_shutdown_timeout_is_short_enough_for_interactive_exit() {
    assert!(super::SHUTDOWN_HTTP_TIMEOUT.as_secs() <= 3);
    assert!(super::SHUTDOWN_GATEWAYS_TIMEOUT.as_secs() <= 5);
}

#[test]
fn notification_payloads_preserve_channel_contracts() {
    let dir = std::env::temp_dir().join(format!(
        "openz_notification_targets_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("telegram_12345.json"), "{}").unwrap();
    std::fs::write(dir.join("telegram_bad-chat.json"), "{}").unwrap();
    std::fs::write(dir.join("discord_98765.json"), "{}").unwrap();
    std::fs::write(dir.join("whatsapp_15551234567.json"), "{}").unwrap();
    std::fs::write(dir.join("telegram_history.json"), "{}").unwrap();

    let mut config = crate::config::schema::Config::default();
    if let Some(tg) = config.channels.telegram.as_mut() {
        tg.enabled = true;
        tg.bot_token = "tg-token".to_string();
    }
    if let Some(dc) = config.channels.discord.as_mut() {
        dc.enabled = true;
        dc.bot_token = "dc-token".to_string();
    }
    if let Some(wa) = config.channels.whatsapp.as_mut() {
        wa.enabled = true;
        wa.api_key = "wa-token".to_string();
        wa.phone_number_id = "phone-id".to_string();
    }

    let requests = build_external_notification_requests(&config, &dir, "hello");

    assert_eq!(requests.len(), 3);
    let telegram = requests
        .iter()
        .find(|request| request.target.channel_name() == "Telegram")
        .unwrap();
    assert_eq!(telegram.target.display_id(), "12345");
    assert!(telegram.url.contains("tg-token"));
    assert_eq!(telegram.payload["chat_id"], 12345);
    assert_eq!(telegram.auth, NotificationAuth::None);

    let discord = requests
        .iter()
        .find(|request| request.target.channel_name() == "Discord")
        .unwrap();
    assert_eq!(discord.target.display_id(), "98765");
    assert_eq!(discord.payload["content"], "hello");
    assert_eq!(
        discord.auth,
        NotificationAuth::Header {
            name: "Authorization",
            value: "Bot dc-token".to_string(),
        }
    );

    let whatsapp = requests
        .iter()
        .find(|request| request.target.channel_name() == "WhatsApp")
        .unwrap();
    assert_eq!(whatsapp.target.display_id(), "15551234567");
    assert_eq!(whatsapp.payload["text"]["body"], "hello");
    assert_eq!(
        whatsapp.auth,
        NotificationAuth::Bearer("wa-token".to_string())
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_whatsapp_notification_requires_credentials() {
    let requests = build_whatsapp_notification_requests(
        String::new(),
        "phone-id",
        vec!["15551234567".to_string()],
        "hello",
    );
    assert!(requests.is_empty());

    let requests = build_whatsapp_notification_requests(
        "wa-token".to_string(),
        "",
        vec!["15551234567".to_string()],
        "hello",
    );
    assert!(requests.is_empty());
}

#[tokio::test]
async fn test_ws_sender_registration_and_cleanup() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<axum::extract::ws::Message>(10);
    let client_id = "test-client-123".to_string();

    // 1. Register sender
    {
        let mut senders = get_active_ws_senders().lock().unwrap();
        senders.insert(client_id.clone(), tx);
    }

    // Verify it is registered
    {
        let senders = get_active_ws_senders().lock().unwrap();
        assert!(senders.contains_key(&client_id));
        assert_eq!(senders.len(), 1);
    }

    // 2. Send notification via broker
    send_notification("Test broadcast message");

    // Receive the message from the receiver to check if it got routed
    let received = rx.recv().await;
    assert!(received.is_some());
    if let Some(axum::extract::ws::Message::Text(txt)) = received {
        assert!(txt.contains("notification"));
        assert!(txt.contains("Test broadcast message"));
    } else {
        panic!("Expected Text message");
    }

    // 3. Clean up sender
    {
        let mut senders = get_active_ws_senders().lock().unwrap();
        senders.remove(&client_id);
    }

    // Verify it is removed
    {
        let senders = get_active_ws_senders().lock().unwrap();
        assert!(!senders.contains_key(&client_id));
        assert_eq!(senders.len(), 0);
    }
}

pub mod stop_command_tests {
    use super::*;

    #[test]
    fn stop_command_matches_slash_stop_only() {
        assert!(is_stop_command("/stop"));
        assert!(is_stop_command(" /stop now"));
        assert!(is_stop_command("/cancel"));
        assert!(is_stop_command("/tui-esc"));
        assert!(is_stop_command("/tui-cancel"));
        assert!(!is_stop_command("please stop"));
        assert!(!is_stop_command("/stopped"));
        assert!(!is_stop_command("/remote"));
    }
}

pub mod channel_session_tests {
    use super::*;

    #[test]
    fn render_resume_list_shows_numbered_sessions() {
        let item = ChannelSessionItem {
            key: "telegram:1:history_20260717_100000".to_string(),
            display_title: "hello from old session".to_string(),
            updated_at: chrono::DateTime::parse_from_rfc3339("2026-07-17T10:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            message_count: 4,
        };
        let out = render_resume_list(&[item], "/resume");
        assert!(out.contains("1. 2026-07-17 10:00"));
        assert!(out.contains("/resume 1"));
    }
}

pub mod model_switch_tests {
    use super::*;

    #[test]
    fn model_risk_marks_unknown_free_models() {
        let risk = classify_model_risk("opencode_zen", "big-pickle");
        assert!(risk.risky);
        assert!(risk
            .reasons
            .iter()
            .any(|reason| reason.contains("not in OpenZ curated")));
    }

    #[test]
    fn model_risk_allows_known_strong_default() {
        let risk = classify_model_risk("opencode_zen", "deepseek-v4-flash-free");
        assert!(!risk.risky);
        assert_eq!(risk.tier, "strong");
    }

    #[test]
    fn model_risk_warns_for_small_models() {
        let risk = classify_model_risk("groq", "llama-3.1-8b-instant");
        assert!(risk.risky);
        assert!(risk
            .reasons
            .iter()
            .any(|reason| reason.contains("small/weak")));
    }

    #[test]
    fn parses_switch_model_provider_and_model_commands() {
        assert_eq!(
            parse_model_switch_command("/switch-model"),
            ModelSwitchCommand::ShowProviders
        );
        assert_eq!(
            parse_model_switch_command(" /switch-model deepseek "),
            ModelSwitchCommand::ShowModels {
                provider: "deepseek".to_string(),
            }
        );
        assert_eq!(
            parse_model_switch_command("/switch-model opencode_zen deepseek-v4-flash-free"),
            ModelSwitchCommand::Set {
                provider: "opencode_zen".to_string(),
                model: "deepseek-v4-flash-free".to_string(),
            }
        );
    }

    #[test]
    fn webui_provider_preview_limits_models_and_keeps_default() {
        let mut config = crate::config::schema::Config::default();
        config.providers.openrouter = Some(crate::config::schema::ProviderConfig {
            api_key: Some("key".to_string()),
            api_key_env: None,
            api_key_file: None,
            api_base: Some("https://openrouter.ai/api/v1".to_string()),
            default_model: Some("custom-default-model".to_string()),
            extra: std::collections::HashMap::new(),
        });
        let provider = configured_provider_model_options(&config)
            .into_iter()
            .find(|provider| provider.name == "openrouter")
            .expect("openrouter should be configured");
        let preview = preview_models_for_provider(&provider, &config, 4);

        assert_eq!(
            preview.first().map(String::as_str),
            Some("custom-default-model")
        );
        assert_eq!(preview.len(), 4);
    }

    #[test]
    fn webui_model_options_stay_configured_only() {
        let config = crate::config::schema::Config::default();
        let configured = configured_provider_model_options(&config);

        assert!(configured.iter().all(|provider| provider.available));
        assert!(!configured.iter().any(|provider| provider.name == "openai"));
        assert!(configured
            .iter()
            .any(|provider| provider.name == "ollama_local"));
    }

    #[test]
    fn model_switch_lists_custom_providers_and_models() {
        let mut config = crate::config::schema::Config::default();
        config.providers.others.insert(
            "acme".to_string(),
            crate::config::schema::ProviderConfig {
                api_key: Some("key".to_string()),
                api_key_env: None,
                api_key_file: None,
                api_base: Some("https://acme.example/v1".to_string()),
                default_model: Some("acme-model".to_string()),
                extra: std::collections::HashMap::new(),
            },
        );

        let providers = render_model_switch_providers(&config);
        assert!(providers.contains("`acme`"));
        assert!(providers.contains("Custom: acme"));

        let models = render_model_switch_models(&config, "acme");
        assert!(models.contains("`acme-model`"));
        assert!(models.contains("/switch-model acme <model>"));
    }

    #[test]
    fn ignores_non_switch_model_commands() {
        assert_eq!(
            parse_model_switch_command("/model"),
            ModelSwitchCommand::None
        );
        assert_eq!(
            parse_model_switch_command("/remote"),
            ModelSwitchCommand::None
        );
        assert_eq!(
            parse_model_switch_command("/switch-models deepseek"),
            ModelSwitchCommand::None
        );
    }
}
