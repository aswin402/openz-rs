use async_trait::async_trait;
use crate::config::provider_catalog;
use std::time::Duration;

use self::notifications::{
    build_external_notification_requests,
};
use self::transport::send_external_notification;

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

pub use crate::providers::model_prefs::{
    load_model_prefs, record_recent_model, save_model_prefs, toggle_favorite_model, ModelPrefs,
    ModelRef,
};

pub use crate::providers::risk::{classify_model_risk, ModelRisk};


#[derive(Debug, Clone)]
pub struct ChannelSessionItem {
    pub key: String,
    pub display_title: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
}

pub fn list_channel_sessions(
    session_dir: &std::path::Path,
    prefix: &str,
    limit: usize,
) -> Vec<ChannelSessionItem> {
    let manager = crate::session::SessionManager::new(session_dir.to_path_buf());
    let mut items: Vec<ChannelSessionItem> = manager
        .list_summaries()
        .into_iter()
        .filter(|s| s.key.starts_with(prefix) && s.message_count > 0)
        .map(|s| {
            let display_title = s.preview_title(70, "Empty session");
            ChannelSessionItem {
                key: s.key,
                display_title,
                updated_at: s.updated_at,
                message_count: s.message_count,
            }
        })
        .collect();
    items.truncate(limit);
    items
}

pub fn render_resume_list(items: &[ChannelSessionItem], command_name: &str) -> String {
    if items.is_empty() {
        return "No previous sessions found for this channel.".to_string();
    }
    let mut out = String::from("Previous sessions:\n");
    for (idx, item) in items.iter().enumerate() {
        out.push_str(&format!(
            "{}. {} | {} msgs | {}\n",
            idx + 1,
            item.updated_at.format("%Y-%m-%d %H:%M"),
            item.message_count,
            item.display_title
        ));
    }
    out.push_str(&format!(
        "\nUse `{command_name} <number>` to resume, for example `{command_name} 1`."
    ));
    out
}

pub async fn start_new_channel_session(
    session_manager: &crate::session::SessionManager,
    active_key: &str,
) -> anyhow::Result<bool> {
    if let Ok(mut current_session) = session_manager.load(active_key) {
        if !current_session.messages.is_empty() {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
            current_session.key = format!("{}:history_{}", active_key, timestamp);
            session_manager.save(&current_session).await?;
        }
    }
    let empty_session = crate::session::Session::new(active_key);
    session_manager.save(&empty_session).await?;
    Ok(true)
}

pub async fn resume_channel_session(
    session_manager: &crate::session::SessionManager,
    active_key: &str,
    selected_key: &str,
) -> anyhow::Result<String> {
    if selected_key == active_key {
        return Ok("Already using that session.".to_string());
    }
    let mut selected = session_manager.load(selected_key)?;
    start_new_channel_session(session_manager, active_key).await?;
    selected.key = active_key.to_string();
    let title = crate::agent::activity::session_preview_from_messages(&selected.messages);
    session_manager.save(&selected).await?;
    Ok(format!("Resumed session: {title}"))
}

pub async fn session_command_text_response(
    session_manager: &crate::session::SessionManager,
    active_key: &str,
    text: &str,
) -> Option<String> {
    let trimmed = text.trim();
    let cmd = trimmed.split_whitespace().next().unwrap_or("");
    if cmd != "/new-session" && cmd != "/resume" {
        return None;
    }
    if cmd == "/new-session" {
        return Some(
            match start_new_channel_session(session_manager, active_key).await {
                Ok(_) => "Session reset. Starting a new session.".to_string(),
                Err(e) => format!("Failed to start new session: {e}"),
            },
        );
    }

    let args: Vec<&str> = trimmed.split_whitespace().collect();
    let sessions = list_channel_sessions(&session_manager.dir, active_key, 10);
    if args.len() == 1 {
        return Some(render_resume_list(&sessions, "/resume"));
    }
    Some(if let Ok(index) = args[1].parse::<usize>() {
        if index == 0 || index > sessions.len() {
            format!(
                "Invalid session number. Use /resume to list 1..{}.",
                sessions.len()
            )
        } else {
            match resume_channel_session(session_manager, active_key, &sessions[index - 1].key)
                .await
            {
                Ok(msg) => msg,
                Err(e) => format!("Failed to resume session: {e}"),
            }
        }
    } else {
        "Usage: /resume or /resume <number>".to_string()
    })
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

    let sessions_dir = crate::config::loader::sessions_dir();
    let client =
        crate::core::http::custom_http_client(SHUTDOWN_HTTP_TIMEOUT, SHUTDOWN_HTTP_TIMEOUT);

    let offline_message = select_random_message(OFFLINE_MESSAGES);
    for request in build_external_notification_requests(config, &sessions_dir, &offline_message) {
        send_external_notification(&client, &request).await;
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

/// Build provider list from real config — only configured/available providers + custom providers.
pub fn build_configured_providers(config: &crate::config::schema::Config) -> Vec<(String, String)> {
    let mut configured: Vec<(String, String)> = PROVIDER_REGISTRY
        .iter()
        .filter_map(|p| {
            let descriptor = provider_catalog::find_provider(p.name)?;
            config
                .is_provider_configured(descriptor.canonical_name)
                .then(|| {
                    (
                        descriptor.canonical_name.to_string(),
                        p.display.to_string(),
                    )
                })
        })
        .collect();

    for name in config.custom_provider_names() {
        if config.is_provider_available(&name) && !configured.iter().any(|(n, _)| n == &name) {
            let default_model = config.custom_provider_default_model(&name);
            let display = format!(
                "Custom: {} ({})",
                name,
                default_model.as_deref().unwrap_or("custom model")
            );
            configured.push((name, display));
        }
    }

    configured
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
pub mod model_catalog;
pub mod model_switch;
pub mod notifications;
pub mod ratatui;
pub mod telegram;
pub mod transport;
pub mod websocket;
pub mod whatsapp;

pub use cli::CliChannel;
pub use discord::DiscordChannel;
pub use email::EmailChannel;
pub use model_catalog::{
    configured_provider_model_options, configured_provider_models, curated_models_for,
    fetch_provider_models, preview_models_for_provider, provider_model_catalog,
    provider_model_catalog_options, provider_model_option_by_name, provider_models_by_name,
    resolved_provider_models_for_webui, ProviderModels, ProviderModelsOption,
    PROVIDER_REGISTRY,
};
pub use model_switch::*;
pub use ratatui::handle_ratatui_tui;
pub use telegram::TelegramChannel;
pub use websocket::WsGateway;
pub use whatsapp::WhatsAppChannel;

use axum::extract::ws::Message;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::sync::mpsc;

pub type WsSendersMap = HashMap<String, mpsc::Sender<Message>>;
pub type WsClientChatsMap = HashMap<String, String>;

pub fn get_active_ws_client_chats() -> &'static Mutex<WsClientChatsMap> {
    static CLIENT_CHATS: OnceLock<Mutex<WsClientChatsMap>> = OnceLock::new();
    CLIENT_CHATS.get_or_init(|| Mutex::new(HashMap::new()))
}

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
            senders
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let evt = crate::channels::websocket::protocol::notification(msg_str.clone());
        if let Ok(evt_str) = serde_json::to_string(&evt) {
            for (id, tx) in ws_senders {
                if tx.send(Message::Text(evt_str.clone())).await.is_err() {
                    if let Ok(mut senders) = get_active_ws_senders().lock() {
                        senders.remove(&id);
                    }
                }
            }
        }

        // Load config to check if external channels (Telegram, Discord, WhatsApp) are enabled.
        if let Ok(config) = crate::config::loader::load_config() {
            let sessions_dir = crate::config::loader::sessions_dir();
            let client = crate::core::http::custom_http_client(
                SHUTDOWN_HTTP_TIMEOUT,
                SHUTDOWN_HTTP_TIMEOUT,
            );

            for request in build_external_notification_requests(&config, &sessions_dir, &msg_str) {
                send_external_notification(&client, &request).await;
            }
        }
    });
}

#[cfg(test)]
mod tests;

