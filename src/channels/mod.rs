use async_trait::async_trait;
use std::time::Duration;

#[async_trait]
pub trait Channel: Send + Sync {
    /// Unique name of the channel
    fn name(&self) -> &'static str;

    /// Runs/starts the listener loop for the channel
    async fn start(&self) -> anyhow::Result<()>;
}

pub const SHUTDOWN_HTTP_TIMEOUT: Duration = Duration::from_secs(3);
pub const SHUTDOWN_GATEWAYS_TIMEOUT: Duration = Duration::from_secs(5);

pub const ACTIVE_MESSAGES: &[&str] = &[
    "Hey there, I'm back.",
    "Looks like we're ready to go.",
    "I'm here if you need anything.",
    "Ready when you are.",
    "What's happening today?",
    "Nice to see you again.",
    "Just arrived. What are we working on?",
    "I've got some free time. Need a hand?",
    "Let's get things moving.",
    "Waiting for your next idea.",
];

pub fn is_stop_command(text: &str) -> bool {
    matches!(
        text.split_whitespace().next(),
        Some("/stop" | "/cancel" | "/tui-esc" | "/tui-cancel")
    )
}

pub const OFFLINE_MESSAGES: &[&str] = &[
    "I'm going to get some rest now.",
    "I'll catch up with you later.",
    "Time to call it a day.",
    "Don't have too much fun without me.",
    "I'll be around again soon.",
    "See you on the next adventure.",
    "I'm off for now.",
    "Thanks for the chat. Until next time.",
    "I'll leave you to it.",
    "Take care, and I'll see you later.",
];

pub fn get_active_session_targets(session_dir: &std::path::Path, prefix: &str) -> Vec<String> {
    let mut targets = Vec::new();
    if let Ok(entries) = std::fs::read_dir(session_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                    if filename.starts_with(prefix) && filename.ends_with(".json") {
                        if let Some(target_str) = filename
                            .strip_prefix(prefix)
                            .and_then(|s| s.strip_suffix(".json"))
                        {
                            if !target_str.contains("history") && !target_str.contains("direct") {
                                targets.push(target_str.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    targets
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NotificationAuth {
    None,
    Bearer(String),
    Header { name: &'static str, value: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NotificationRequest {
    channel: &'static str,
    target: String,
    url: String,
    payload: serde_json::Value,
    auth: NotificationAuth,
}

fn configured_or_env(config_value: &str, env_var: &str) -> Option<String> {
    if config_value.trim().is_empty() {
        std::env::var(env_var).ok().filter(|v| !v.trim().is_empty())
    } else {
        Some(config_value.to_string())
    }
}

fn build_telegram_notification_requests(
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
                channel: "Telegram",
                target,
                url: format!("https://api.telegram.org/bot{token}/sendMessage"),
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

fn build_discord_notification_requests(
    token: String,
    targets: Vec<String>,
    msg: &str,
) -> Vec<NotificationRequest> {
    targets
        .into_iter()
        .map(|target| NotificationRequest {
            channel: "Discord",
            url: format!("https://discord.com/api/v10/channels/{target}/messages"),
            target,
            payload: serde_json::json!({ "content": msg }),
            auth: NotificationAuth::Header {
                name: "Authorization",
                value: format!("Bot {token}"),
            },
        })
        .collect()
}

fn build_whatsapp_notification_requests(
    api_key: String,
    phone_number_id: &str,
    targets: Vec<String>,
    msg: &str,
) -> Vec<NotificationRequest> {
    if phone_number_id.trim().is_empty() || api_key.trim().is_empty() {
        tracing::warn!(
            "Skipping WhatsApp notifications because api_key or phone_number_id is empty"
        );
        return Vec::new();
    }

    targets
        .into_iter()
        .map(|target| NotificationRequest {
            channel: "WhatsApp",
            target: target.clone(),
            url: format!("https://graph.facebook.com/v18.0/{phone_number_id}/messages"),
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

fn build_external_notification_requests(
    config: &crate::config::schema::Config,
    sessions_dir: &std::path::Path,
    msg: &str,
) -> Vec<NotificationRequest> {
    let mut requests = Vec::new();

    if let Some(tg_config) = &config.channels.telegram {
        if tg_config.enabled {
            if let Some(token) = configured_or_env(&tg_config.bot_token, "TELEGRAM_BOT_TOKEN") {
                requests.extend(build_telegram_notification_requests(
                    token,
                    get_active_session_targets(sessions_dir, "telegram_"),
                    msg,
                ));
            } else {
                tracing::warn!(
                    "Skipping Telegram notifications because no bot token is configured"
                );
            }
        }
    }

    if let Some(dc_config) = &config.channels.discord {
        if dc_config.enabled {
            if let Some(token) = configured_or_env(&dc_config.bot_token, "DISCORD_BOT_TOKEN") {
                requests.extend(build_discord_notification_requests(
                    token,
                    get_active_session_targets(sessions_dir, "discord_"),
                    msg,
                ));
            } else {
                tracing::warn!("Skipping Discord notifications because no bot token is configured");
            }
        }
    }

    if let Some(wa_config) = &config.channels.whatsapp {
        if wa_config.enabled {
            requests.extend(build_whatsapp_notification_requests(
                wa_config.api_key.clone(),
                &wa_config.phone_number_id,
                get_active_session_targets(sessions_dir, "whatsapp_"),
                msg,
            ));
        }
    }

    requests
}

async fn send_external_notification(client: &reqwest::Client, request: &NotificationRequest) {
    let mut builder = client.post(&request.url).json(&request.payload);
    match &request.auth {
        NotificationAuth::None => {}
        NotificationAuth::Bearer(token) => {
            builder = builder.bearer_auth(token);
        }
        NotificationAuth::Header { name, value } => {
            builder = builder.header(*name, value);
        }
    }

    match builder.send().await {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    channel = request.channel,
                    target = %request.target,
                    status = %status,
                    response = %body,
                    "Failed to send external notification"
                );
            }
        }
        Err(err) => {
            tracing::warn!(
                channel = request.channel,
                target = %request.target,
                error = %err,
                "Error sending external notification"
            );
        }
    }
}

pub fn select_random_message(messages: &[&str]) -> String {
    if messages.is_empty() {
        return String::new();
    }
    use rand::Rng;
    let idx = rand::thread_rng().gen_range(0..messages.len());
    messages[idx].to_string()
}

pub async fn shutdown_gateways(config: &crate::config::schema::Config) {
    let silent = std::env::var("OPENZ_SILENT").is_ok();
    if !silent {
        crate::tui_println!("Shutting down gateways...");
    }

    let sessions_dir = crate::config::loader::resolve_path("~/.openz/sessions");
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .connect_timeout(SHUTDOWN_HTTP_TIMEOUT)
        .timeout(SHUTDOWN_HTTP_TIMEOUT)
        .build()
        .unwrap_or_default();

    // Telegram
    if let Some(tg_config) = &config.channels.telegram {
        if tg_config.enabled {
            let token = if tg_config.bot_token.is_empty() {
                std::env::var("TELEGRAM_BOT_TOKEN").ok()
            } else {
                Some(tg_config.bot_token.clone())
            };
            if let Some(token) = token {
                let chats = get_active_session_targets(&sessions_dir, "telegram_");
                let msg = select_random_message(OFFLINE_MESSAGES);
                for chat_str in chats {
                    if let Ok(chat_id) = chat_str.parse::<i64>() {
                        let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                        let payload = serde_json::json!({
                            "chat_id": chat_id,
                            "text": msg
                        });
                        match client.post(&send_url).json(&payload).send().await {
                            Ok(resp) => {
                                let status = resp.status();
                                if !status.is_success() {
                                    if let Ok(body) = resp.text().await {
                                        eprintln!("Failed to send Telegram offline message: status {}, response: {}", status, body);
                                    } else {
                                        eprintln!(
                                            "Failed to send Telegram offline message: status {}",
                                            status
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Error sending Telegram offline message: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    // Discord
    if let Some(dc_config) = &config.channels.discord {
        if dc_config.enabled {
            let token = if dc_config.bot_token.is_empty() {
                std::env::var("DISCORD_BOT_TOKEN").ok()
            } else {
                Some(dc_config.bot_token.clone())
            };
            if let Some(token) = token {
                let channels = get_active_session_targets(&sessions_dir, "discord_");
                let msg = select_random_message(OFFLINE_MESSAGES);
                for channel_id in channels {
                    let send_url = format!(
                        "https://discord.com/api/v10/channels/{}/messages",
                        channel_id
                    );
                    let payload = serde_json::json!({
                        "content": msg
                    });
                    match client
                        .post(&send_url)
                        .header("Authorization", format!("Bot {}", token))
                        .json(&payload)
                        .send()
                        .await
                    {
                        Ok(resp) => {
                            let status = resp.status();
                            if !status.is_success() {
                                if let Ok(body) = resp.text().await {
                                    eprintln!("Failed to send Discord offline message: status {}, response: {}", status, body);
                                } else {
                                    eprintln!(
                                        "Failed to send Discord offline message: status {}",
                                        status
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error sending Discord offline message: {}", e);
                        }
                    }
                }
            }
        }
    }

    // WhatsApp
    if let Some(wa_config) = &config.channels.whatsapp {
        if wa_config.enabled {
            let targets = get_active_session_targets(&sessions_dir, "whatsapp_");
            let msg = select_random_message(OFFLINE_MESSAGES);
            for phone_number in targets {
                let send_url = format!(
                    "https://graph.facebook.com/v18.0/{}/messages",
                    wa_config.phone_number_id
                );
                let payload = serde_json::json!({
                    "messaging_product": "whatsapp",
                    "recipient_type": "individual",
                    "to": phone_number,
                    "type": "text",
                    "text": {
                        "body": msg
                    }
                });
                match client
                    .post(&send_url)
                    .bearer_auth(&wa_config.api_key)
                    .json(&payload)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        let status = resp.status();
                        if !status.is_success() {
                            if let Ok(body) = resp.text().await {
                                eprintln!("Failed to send WhatsApp offline message: status {}, response: {}", status, body);
                            } else {
                                eprintln!(
                                    "Failed to send WhatsApp offline message: status {}",
                                    status
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error sending WhatsApp offline message: {}", e);
                    }
                }
            }
        }
    }

    // Unload the active Ollama model and stop the local service if spawned by us
    crate::providers::ollama_manager::unload_active_ollama_model(config).await;
    crate::providers::ollama_manager::stop_local_ollama();
}

pub async fn shutdown_gateways_bounded(config: &crate::config::schema::Config) {
    match tokio::time::timeout(SHUTDOWN_GATEWAYS_TIMEOUT, shutdown_gateways(config)).await {
        Ok(()) => {}
        Err(_) => {
            crate::tui_println!(
                "Gateway shutdown timed out after {}s; forcing local exit.",
                SHUTDOWN_GATEWAYS_TIMEOUT.as_secs()
            );
        }
    }
}

pub async fn fetch_provider_models(
    provider_name: &str,
    config: &crate::config::schema::Config,
) -> Option<Vec<String>> {
    static HTTP: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default()
    });

    let (api_key, api_base) = config.resolve_provider_config(provider_name);

    if provider_name != "ollama" && provider_name != "ollama_local" && api_key.is_empty() {
        return None;
    }

    let url = if api_base.ends_with('/') {
        format!("{}models", api_base)
    } else {
        format!("{}/models", api_base)
    };

    let mut req = client.get(&url);
    if provider_name == "anthropic" {
        req = req
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01");
    } else if !api_key.is_empty() {
        req = req.bearer_auth(&api_key);
    }

    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    let mut models = Vec::new();

    if let Some(data_arr) = json.get("data").and_then(|d| d.as_array()) {
        for m in data_arr {
            if let Some(id) = m.get("id").and_then(|id| id.as_str()) {
                models.push(id.to_string());
            }
        }
    } else if let Some(models_arr) = json.get("models").and_then(|m| m.as_array()) {
        for m in models_arr {
            if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                let name_cleaned = name.strip_prefix("models/").unwrap_or(name);
                models.push(name_cleaned.to_string());
            }
        }
    }

    if models.is_empty() {
        None
    } else {
        models.sort();
        Some(models)
    }
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut accum = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        accum |= x ^ y;
    }
    accum == 0
}

pub fn secure_compare(a: &str, b: &str) -> bool {
    use sha2::{Digest, Sha256};
    let hash_a = Sha256::digest(a.as_bytes());
    let hash_b = Sha256::digest(b.as_bytes());
    constant_time_eq(&hash_a, &hash_b)
}

pub mod cli;
pub mod discord;
pub mod email;
pub mod telegram;
pub mod websocket;
pub mod whatsapp;

pub use cli::CliChannel;
pub use discord::DiscordChannel;
pub use email::EmailChannel;
pub use telegram::TelegramChannel;
pub use websocket::WsGateway;
pub use whatsapp::WhatsAppChannel;

use axum::extract::ws::Message;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::sync::mpsc;

pub type WsSendersMap = HashMap<String, mpsc::Sender<Message>>;

pub fn get_active_ws_senders() -> &'static Mutex<WsSendersMap> {
    static SENDERS: OnceLock<Mutex<WsSendersMap>> = OnceLock::new();
    SENDERS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn send_notification(msg: &str) {
    // 1. Immediately output/queue to CLI (synchronously so it prints immediately in terminal)
    crate::channels::cli::queue_notification(msg);

    // 2. Broadcast to other active channels in the background
    let msg_str = msg.to_string();
    tokio::spawn(async move {
        // Broadcast to WebSocket WebUI clients
        let ws_senders = if let Ok(senders) = get_active_ws_senders().lock() {
            senders.values().cloned().collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let evt = serde_json::json!({
            "event": "notification",
            "message": msg_str
        });
        if let Ok(evt_str) = serde_json::to_string(&evt) {
            for tx in ws_senders {
                let _ = tx.send(Message::Text(evt_str.clone())).await;
            }
        }

        // Load config to check if external channels (Telegram, Discord, WhatsApp) are enabled.
        if let Ok(config) = crate::config::loader::load_config() {
            let sessions_dir = crate::config::loader::resolve_path("~/.openz/sessions");
            let client = reqwest::Client::builder()
                .use_rustls_tls()
                .connect_timeout(SHUTDOWN_HTTP_TIMEOUT)
                .timeout(SHUTDOWN_HTTP_TIMEOUT)
                .build()
                .unwrap_or_default();

            for request in build_external_notification_requests(&config, &sessions_dir, &msg_str) {
                send_external_notification(&client, &request).await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_shutdown_timeout_is_short_enough_for_interactive_exit() {
        assert!(super::SHUTDOWN_HTTP_TIMEOUT.as_secs() <= 3);
        assert!(super::SHUTDOWN_GATEWAYS_TIMEOUT.as_secs() <= 5);
    }

    use super::*;

    #[test]
    fn test_build_external_notification_requests_for_enabled_channels() {
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
            .find(|request| request.channel == "Telegram")
            .unwrap();
        assert_eq!(telegram.target, "12345");
        assert!(telegram.url.contains("tg-token"));
        assert_eq!(telegram.payload["chat_id"], 12345);
        assert_eq!(telegram.auth, NotificationAuth::None);

        let discord = requests
            .iter()
            .find(|request| request.channel == "Discord")
            .unwrap();
        assert_eq!(discord.target, "98765");
        assert_eq!(discord.payload["content"], "hello");
        assert_eq!(
            discord.auth,
            NotificationAuth::Header {
                name: "Authorization",
                value: "Bot dc-token".to_string()
            }
        );

        let whatsapp = requests
            .iter()
            .find(|request| request.channel == "WhatsApp")
            .unwrap();
        assert_eq!(whatsapp.target, "15551234567");
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
}

#[cfg(test)]
mod stop_command_tests {
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
