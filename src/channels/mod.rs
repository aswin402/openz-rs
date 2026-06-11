use async_trait::async_trait;

#[async_trait]
pub trait Channel: Send + Sync {
    /// Unique name of the channel
    fn name(&self) -> &'static str;

    /// Runs/starts the listener loop for the channel
    async fn start(&self) -> anyhow::Result<()>;
}

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
                        if let Some(target_str) = filename.strip_prefix(prefix).and_then(|s| s.strip_suffix(".json")) {
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

pub fn select_random_message(messages: &[&str]) -> String {
    let uuid_bytes = uuid::Uuid::new_v4().into_bytes();
    let idx = (uuid_bytes[0] as usize) % messages.len();
    messages[idx].to_string()
}

pub async fn shutdown_gateways(config: &crate::config::schema::Config) {
    let silent = std::env::var("OPENZ_SILENT").is_ok();
    if !silent {
        println!("Shutting down gateways...");
    }
    
    let sessions_dir = crate::config::loader::resolve_path("~/.openz/sessions");
    let client = reqwest::Client::builder().use_rustls_tls().build().unwrap_or_default();
    
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
                            "text": msg,
                            "parse_mode": "Markdown"
                        });
                        match client.post(&send_url).json(&payload).send().await {
                            Ok(resp) => {
                                let status = resp.status();
                                if !status.is_success() {
                                    if let Ok(body) = resp.text().await {
                                        eprintln!("Failed to send Telegram offline message: status {}, response: {}", status, body);
                                    } else {
                                        eprintln!("Failed to send Telegram offline message: status {}", status);
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
                    let send_url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);
                    let payload = serde_json::json!({
                        "content": msg
                    });
                    match client.post(&send_url)
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
                                    eprintln!("Failed to send Discord offline message: status {}", status);
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
                let send_url = format!("https://graph.facebook.com/v18.0/{}/messages", wa_config.phone_number_id);
                let payload = serde_json::json!({
                    "messaging_product": "whatsapp",
                    "recipient_type": "individual",
                    "to": phone_number,
                    "type": "text",
                    "text": {
                        "body": msg
                    }
                });
                match client.post(&send_url)
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
                                eprintln!("Failed to send WhatsApp offline message: status {}", status);
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
}

pub mod websocket;
pub mod cli;
pub mod telegram;
pub mod discord;
pub mod whatsapp;

pub use websocket::WsGateway;
pub use cli::CliChannel;
pub use telegram::TelegramChannel;
pub use discord::DiscordChannel;
pub use whatsapp::WhatsAppChannel;
