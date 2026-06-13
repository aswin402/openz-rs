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

pub async fn fetch_provider_models(provider_name: &str, config: &crate::config::schema::Config) -> Option<Vec<String>> {
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();
    
    let (api_key, api_base) = match provider_name {
        "anthropic" => {
            let p = config.providers.anthropic.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            (key, base)
        }
        "openai" => {
            let p = config.providers.openai.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            (key, base)
        }
        "openrouter" => {
            let p = config.providers.openrouter.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
            (key, base)
        }
        "deepseek" => {
            let p = config.providers.deepseek.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string());
            (key, base)
        }
        "groq" => {
            let p = config.providers.groq.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("GROQ_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string());
            (key, base)
        }
        "ollama" => {
            let p = config.providers.ollama.as_ref();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
            (String::new(), base)
        }
        "minimax" => {
            let p = config.providers.minimax.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("MINIMAX_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.minimax.io/v1".to_string());
            (key, base)
        }
        "mistral" => {
            let p = config.providers.mistral.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("MISTRAL_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string());
            (key, base)
        }
        "z.ai" => {
            let p = config.providers.z_ai.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("Z_AI_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.z.ai/api/paas/v4/".to_string());
            (key, base)
        }
        "nvidia" => {
            let p = config.providers.nvidia.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("NVIDIA_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://integrate.api.nvidia.com/v1".to_string());
            (key, base)
        }
        "opencode_zen" => {
            let p = config.providers.opencode_zen.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENCODE_ZEN_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://opencode.ai/zen/v1".to_string());
            (key, base)
        }
        "cerebres" => {
            let p = config.providers.cerebres.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("CEREBRES_API_KEY").ok())
                .or_else(|| std::env::var("CEBRAS_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.cerebras.ai/v1".to_string());
            (key, base)
        }
        "google_ai_studio" => {
            let p = config.providers.google_ai_studio.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("GOOGLE_AI_STUDIO_API_KEY").ok())?;
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta/openai/".to_string());
            (key, base)
        }
        _ => return None,
    };

    if provider_name != "ollama" && api_key.is_empty() {
        return None;
    }

    let url = if api_base.ends_with('/') {
        format!("{}models", api_base)
    } else {
        format!("{}/models", api_base)
    };

    let mut req = client.get(&url);
    if provider_name == "anthropic" {
        req = req.header("x-api-key", &api_key)
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

pub mod websocket;
pub mod cli;
pub mod telegram;
pub mod discord;
pub mod whatsapp;
pub mod email;

pub use websocket::WsGateway;
pub use cli::CliChannel;
pub use telegram::TelegramChannel;
pub use discord::DiscordChannel;
pub use whatsapp::WhatsAppChannel;
pub use email::EmailChannel;
