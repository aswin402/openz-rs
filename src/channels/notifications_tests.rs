use super::*;

#[test]
fn notification_target_normalization_preserves_channel_contracts() {
    let telegram = build_telegram_notification_requests(
        "telegram-token".to_string(),
        vec!["00123".to_string(), "not-a-chat".to_string()],
        "hello",
    );
    assert_eq!(telegram.len(), 1);
    assert!(matches!(
        &telegram[0].target,
        NotificationTarget::Telegram { chat_id } if chat_id == "00123"
    ));
    assert_eq!(telegram[0].payload["chat_id"], 123);

    let discord = build_discord_notification_requests(
        "discord-token".to_string(),
        vec!["channel-1".to_string()],
        "hello",
    );
    assert!(matches!(
        &discord[0].target,
        NotificationTarget::Discord { channel_id } if channel_id == "channel-1"
    ));

    let whatsapp = build_whatsapp_notification_requests(
        "whatsapp-token".to_string(),
        "phone-1",
        vec!["15551234567".to_string()],
        "hello",
    );
    assert!(matches!(
        &whatsapp[0].target,
        NotificationTarget::WhatsApp { recipient } if recipient == "15551234567"
    ));
}

#[test]
fn notification_errors_redact_credentials() {
    let request = NotificationRequest {
        target: NotificationTarget::Telegram {
            chat_id: "123".to_string(),
        },
        url: "https://api.telegram.org/bottelegram-secret/sendMessage".to_string(),
        payload: serde_json::json!({"text": "hello"}),
        auth: NotificationAuth::None,
    };
    let error = redact_notification_text(
        &request,
        "request failed for https://api.telegram.org/bottelegram-secret/sendMessage",
    );
    assert!(!error.contains("telegram-secret"));
    assert!(error.contains("<notification endpoint>"));

    let request = NotificationRequest {
        target: NotificationTarget::WhatsApp {
            recipient: "15551234567".to_string(),
        },
        url: "https://graph.facebook.com/v18.0/phone/messages".to_string(),
        payload: serde_json::json!({"text": "hello"}),
        auth: NotificationAuth::Bearer("whatsapp-secret".to_string()),
    };
    let error = redact_notification_text(
        &request,
        "401 response included whatsapp-secret in the body",
    );
    assert!(!error.contains("whatsapp-secret"));
    assert!(error.contains("<redacted credential>"));
}
