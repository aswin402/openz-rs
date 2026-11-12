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

pub(crate) fn spawn_telegram_msg(client: Client, token: String, chat_id: i64, text: String) {
    tokio::spawn(async move {
        let send_url = telegram_api_url(&token, "sendMessage");
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text
        });
        let _ = client.post(&send_url).json(&payload).send().await;
    });
}

pub(crate) fn spawn_telegram_markdown(client: Client, token: String, chat_id: i64, text: String) {
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

fn resolve_telegram_bot_info() -> Option<(String, Client)> {
    match super::state::get_telegram_bot_info() {
        Some(pair) => Some(pair),
        None => {
            let cfg_token = crate::config::loader::load_config()
                .ok()
                .and_then(|cfg| cfg.channels.telegram)
                .map(|tg| tg.bot_token)
                .unwrap_or_default();
            let token = match cfg_token.trim().is_empty() {
                true => std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default(),
                false => cfg_token,
            };
            let trimmed = token.trim().to_string();
            (!trimmed.is_empty()).then(|| (trimmed, crate::core::http::default_http_client()))
        }
    }
}

/// Send a text response through the active Telegram bot, splitting messages at
/// Telegram's message-size limit.
pub async fn send_text_message(chat_id: i64, text: &str) -> anyhow::Result<()> {
    send_text_message_to_target(&chat_id.to_string(), text).await
}

pub async fn send_text_message_to_target(target: &str, text: &str) -> anyhow::Result<()> {
    let (bot_token, client) = resolve_telegram_bot_info()
        .ok_or_else(|| anyhow::anyhow!("Telegram bot token is not configured"))?;

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
                    match attempt < 2 {
                        true => {
                            tokio::time::sleep(Duration::from_millis(250 * (attempt + 1) as u64))
                                .await;
                            continue;
                        }
                        false => break,
                    }
                }
            };
            let api = serde_json::from_str::<super::types::TelegramApiResponse>(&response.body).ok();
            let is_ok =
                response.status.is_success() && api.as_ref().is_some_and(|result| result.ok);
            match is_ok {
                true => {
                    last_error = None;
                    break;
                }
                false => {}
            }

            let description = api
                .and_then(|result| result.description)
                .unwrap_or_else(|| response.body.clone());
            last_error = Some(format!("HTTP {}: {}", response.status, description));
            let retryable =
                (response.status.is_server_error() || response.status.as_u16() == 429) && attempt < 2;
            match retryable {
                true => {
                    tokio::time::sleep(Duration::from_millis(250 * (attempt + 1) as u64)).await;
                }
                false => break,
            }
        }

        match last_error {
            Some(error) => return Err(anyhow::anyhow!("Telegram sendMessage failed: {}", error)),
            None => {}
        }
    }
    Ok(())
}

pub async fn deliver_remote_reply(
    sender: &str,
    surface_label: &str,
    outcome: Result<&str, &str>,
) {
    let chat_id_str = match sender.strip_prefix("telegram:") {
        Some(target) => target.trim(),
        None => return,
    };
    match chat_id_str.parse::<i64>() {
        Ok(chat_id) => {
            crate::agent::activity::clear_remote_turn_pending(chat_id);
            super::state::stop_typing_indicator(chat_id);
        }
        Err(_) => {}
    }

    let reply_text = match outcome {
        Ok(content) => {
            let trimmed = content.trim();
            match trimmed.is_empty() {
                true => format!("✓ [{surface_label}] Completed (no text output)."),
                false => trimmed.to_string(),
            }
        }
        Err(err) => format!("❌ [{surface_label}] Error processing remote request: {err}"),
    };

    let _ = send_text_message_to_target(chat_id_str, &reply_text).await;
}
