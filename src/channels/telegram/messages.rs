use crate::channels::notifications::{
    chunk_message, telegram_api_url, NotificationAuth, NotificationRequest, NotificationTarget,
};
use crate::channels::transport::send_notification_request;
use reqwest::Client;
use std::time::Duration;

pub(crate) const TELEGRAM_MAX_MESSAGE_BYTES: usize = 4096;

pub(crate) fn telegram_channel_silent() -> bool {
    std::env::var("OPENZ_SILENT").is_ok() || crate::cli::is_silent_mode()
}

pub(crate) fn remote_timeout_secs() -> u64 {
    std::env::var("OPENZ_REMOTE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(900)
        .clamp(60, 3600)
}

pub(crate) fn escape_markdown(s: &str) -> String {
    let mut res = String::new();
    for c in s.chars() {
        match c {
            '_' | '*' | '`' | '[' => {
                res.push('\\');
                res.push(c);
            }
            _ => res.push(c),
        }
    }
    res
}

pub(crate) fn spawn_telegram_msg(
    client: Client,
    token: String,
    chat_id: i64,
    text: String,
) {
    tokio::spawn(async move {
        let send_url = telegram_api_url(&token, "sendMessage");
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text
        });
        let _ = client.post(&send_url).json(&payload).send().await;
    });
}

pub(crate) fn spawn_telegram_markdown(
    client: Client,
    token: String,
    chat_id: i64,
    text: String,
) {
    tokio::spawn(async move {
        let send_url = telegram_api_url(&token, "sendMessage");
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown"
        });
        let _ = client.post(&send_url).json(&payload).send().await;
    });
}

pub(crate) fn spawn_telegram_keyboard(
    client: Client,
    token: String,
    chat_id: i64,
    text: String,
    keyboard: serde_json::Value,
) {
    tokio::spawn(async move {
        let send_url = telegram_api_url(&token, "sendMessage");
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "reply_markup": { "inline_keyboard": keyboard }
        });
        let _ = client.post(&send_url).json(&payload).send().await;
    });
}

/// Send a text response through the active Telegram bot, splitting messages at
/// Telegram's message-size limit.
pub async fn send_text_message(chat_id: i64, text: &str) -> anyhow::Result<()> {
    send_text_message_to_target(&chat_id.to_string(), text).await
}

pub async fn send_text_message_to_target(target: &str, text: &str) -> anyhow::Result<()> {
    let (bot_token, client) = super::state::get_telegram_bot_info()
        .ok_or_else(|| anyhow::anyhow!("Telegram bot is not active"))?;

    for chunk in chunk_message(text, TELEGRAM_MAX_MESSAGE_BYTES) {
        let request = NotificationRequest {
            target: NotificationTarget::Telegram {
                chat_id: target.to_string(),
            },
            url: telegram_api_url(&bot_token, "sendMessage"),
            payload: serde_json::json!({
                "chat_id": target,
                "text": chunk
            }),
            auth: NotificationAuth::None,
        };
        let mut last_error = None;
        for attempt in 0..3 {
            let response = match send_notification_request(&client, &request).await {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(error.to_string());
                    if attempt < 2 {
                        tokio::time::sleep(Duration::from_millis(250 * (attempt + 1) as u64)).await;
                        continue;
                    }
                    break;
                }
            };
            let api = serde_json::from_str::<super::types::TelegramApiResponse>(&response.body).ok();
            if response.status.is_success() && api.as_ref().is_some_and(|result| result.ok) {
                last_error = None;
                break;
            }

            let description = api
                .and_then(|result| result.description)
                .unwrap_or_else(|| response.body.clone());
            last_error = Some(format!("HTTP {}: {}", response.status, description));
            if !(response.status.is_server_error() || response.status.as_u16() == 429)
                || attempt == 2
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250 * (attempt + 1) as u64)).await;
        }

        if let Some(error) = last_error {
            return Err(anyhow::anyhow!("Telegram sendMessage failed: {}", error));
        }
    }
    Ok(())
}
