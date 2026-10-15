//! Shared external notification request construction and delivery.
//!
//! Channel listeners keep their inbound protocol handling local. Outbound
//! notifications use these typed requests so shutdown, progress, and
//! background notices share payload/auth behavior.

use crate::channels::get_active_session_targets;
pub(crate) use super::transport::send_external_notification;
#[cfg(test)]
pub(crate) use super::transport::redact_notification_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotificationAuth {
    None,
    Bearer(String),
    Header { name: &'static str, value: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotificationTarget {
    Telegram { chat_id: String },
    Discord { channel_id: String },
    WhatsApp { recipient: String },
}

impl NotificationTarget {
    pub(crate) fn channel_name(&self) -> &'static str {
        match self {
            Self::Telegram { .. } => "Telegram",
            Self::Discord { .. } => "Discord",
            Self::WhatsApp { .. } => "WhatsApp",
        }
    }

    pub(crate) fn display_id(&self) -> &str {
        match self {
            Self::Telegram { chat_id } => chat_id,
            Self::Discord { channel_id } => channel_id,
            Self::WhatsApp { recipient } => recipient,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NotificationRequest {
    pub(crate) target: NotificationTarget,
    pub(crate) url: String,
    pub(crate) payload: serde_json::Value,
    pub(crate) auth: NotificationAuth,
}

/// Build a Telegram Bot API endpoint without duplicating URL formatting at
/// each notification producer.
pub(crate) fn telegram_api_url(token: &str, method: &str) -> String {
    format!("https://api.telegram.org/bot{token}/{method}")
}

/// Build the Discord channel-message endpoint used by outbound text notices.
pub(crate) fn discord_message_url(channel_id: &str) -> String {
    format!("https://discord.com/api/v10/channels/{channel_id}/messages")
}

/// Build the WhatsApp Business text-message endpoint.
pub(crate) fn whatsapp_message_url(phone_number_id: &str) -> String {
    format!("https://graph.facebook.com/v18.0/{phone_number_id}/messages")
}

/// Split outbound text without breaking UTF-8 code points or needlessly
/// carrying a leading newline into the next protocol message.
pub(crate) fn chunk_message(text: &str, max_len: usize) -> Vec<String> {
    match text.len() <= max_len {
        true => return vec![text.to_string()],
        false => {}
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        match remaining.len() <= max_len {
            true => {
                chunks.push(remaining.to_string());
                break;
            }
            false => {}
        }

        let mut split_at = max_len;
        while split_at > 0 && !remaining.is_char_boundary(split_at) {
            split_at -= 1;
        }
        match split_at {
            0 => {
                split_at = 1;
                while split_at < remaining.len() && !remaining.is_char_boundary(split_at) {
                    split_at += 1;
                }
            }
            _ => {}
        }

        let candidate = &remaining[..split_at];
        let final_split = match candidate.rfind('\n') {
            Some(0) | None => split_at,
            Some(idx) => idx,
        };

        chunks.push(remaining[..final_split].to_string());
        remaining = remaining[final_split..].trim_start_matches('\n');
    }
    chunks
}

fn configured_or_env(config_value: &str, env_var: &str) -> Option<String> {
    match config_value.trim().is_empty() {
        true => std::env::var(env_var).ok().filter(|v| !v.trim().is_empty()),
        false => Some(config_value.to_string()),
    }
}

pub(crate) fn build_telegram_notification_requests(
    token: String,
    targets: Vec<String>,
    msg: &str,
) -> Vec<NotificationRequest> {
    targets
        .into_iter()
        .filter_map(|target| {
            let chat_id = match target.parse::<i64>() {
                Ok(chat_id) => chat_id,
                Err(_) => {
                    tracing::warn!(target = %target, "Skipping invalid Telegram notification target");
                    return None;
                }
            };
            Some(NotificationRequest {
                target: NotificationTarget::Telegram {
                    chat_id: target,
                },
                url: telegram_api_url(&token, "sendMessage"),
                payload: serde_json::json!({
                    "chat_id": chat_id,
                    "text": msg,
                    "parse_mode": "Markdown"
                }),
                auth: NotificationAuth::None,
            })
        })
        .collect()
}

pub(crate) fn build_discord_notification_requests(
    token: String,
    targets: Vec<String>,
    msg: &str,
) -> Vec<NotificationRequest> {
    targets
        .into_iter()
        .map(|target| NotificationRequest {
            url: discord_message_url(&target),
            target: NotificationTarget::Discord { channel_id: target },
            payload: serde_json::json!({ "content": msg }),
            auth: NotificationAuth::Header {
                name: "Authorization",
                value: format!("Bot {token}"),
            },
        })
        .collect()
}

pub(crate) fn build_whatsapp_notification_requests(
    api_key: String,
    phone_number_id: &str,
    targets: Vec<String>,
    msg: &str,
) -> Vec<NotificationRequest> {
    match phone_number_id.trim().is_empty() || api_key.trim().is_empty() {
        true => {
            tracing::warn!(
                "Skipping WhatsApp notifications because api_key or phone_number_id is empty"
            );
            return Vec::new();
        }
        false => {}
    }

    targets
        .into_iter()
        .map(|target| NotificationRequest {
            target: NotificationTarget::WhatsApp {
                recipient: target.clone(),
            },
            url: whatsapp_message_url(phone_number_id),
            payload: serde_json::json!({
                "messaging_product": "whatsapp",
                "recipient_type": "individual",
                "to": target,
                "type": "text",
                "text": { "body": msg }
            }),
            auth: NotificationAuth::Bearer(api_key.clone()),
        })
        .collect()
}

pub(crate) fn build_external_notification_requests(
    config: &crate::config::schema::Config,
    sessions_dir: &std::path::Path,
    msg: &str,
) -> Vec<NotificationRequest> {
    let mut requests = Vec::new();

    match &config.channels.telegram {
        Some(tg_config) => match (
            tg_config.enabled,
            configured_or_env(&tg_config.bot_token, "TELEGRAM_BOT_TOKEN"),
        ) {
            (true, Some(token)) => requests.extend(build_telegram_notification_requests(
                token,
                get_active_session_targets(sessions_dir, "telegram_"),
                msg,
            )),
            (true, None) => {
                tracing::warn!(
                    "Skipping Telegram notifications because no bot token is configured"
                );
            }
            (false, _) => {}
        },
        None => {}
    }

    match &config.channels.discord {
        Some(dc_config) => match (
            dc_config.enabled,
            configured_or_env(&dc_config.bot_token, "DISCORD_BOT_TOKEN"),
        ) {
            (true, Some(token)) => requests.extend(build_discord_notification_requests(
                token,
                get_active_session_targets(sessions_dir, "discord_"),
                msg,
            )),
            (true, None) => {
                tracing::warn!("Skipping Discord notifications because no bot token is configured");
            }
            (false, _) => {}
        },
        None => {}
    }

    match &config.channels.whatsapp {
        Some(wa_config) => match wa_config.enabled {
            true => requests.extend(build_whatsapp_notification_requests(
                wa_config.api_key.clone(),
                &wa_config.phone_number_id,
                get_active_session_targets(sessions_dir, "whatsapp_"),
                msg,
            )),
            false => {}
        },
        None => {}
    }

    requests
}

#[cfg(test)]
#[path = "notifications_tests.rs"]
mod tests;
