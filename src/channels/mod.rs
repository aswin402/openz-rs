use crate::config::provider_catalog;
use async_trait::async_trait;
use std::time::Duration;

use self::notifications::build_external_notification_requests;
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

pub fn channel_concurrency_semaphore() -> std::sync::Arc<tokio::sync::Semaphore> {
    std::sync::Arc::new(tokio::sync::Semaphore::new(5))
}

pub async fn intercept_channel_text_command(
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    text: &str,
    stop_message: &str,
) -> Option<String> {
    match is_stop_command(text) {
        true => {
            crate::shutdown::trigger_cli_cancel();
            return Some(stop_message.to_string());
        }
        false => {}
    }
    match session_command_text_response(session_manager, session_key, text).await {
        Some(response_text) => return Some(response_text),
        None => {}
    }
    model_switch_text_response(text)
}

pub fn enabled_lower_label(flag: bool) -> &'static str {
    match flag {
        true => "enabled",
        false => "disabled",
    }
}

pub fn format_stop_server_response(target: &str) -> String {
    let target = target.trim();
    match target.is_empty() {
        true => "Usage: /stop-server <id|all>".to_string(),
        false => match crate::shutdown::stop_registered_child(target) {
            Ok(0) => "No matching background server found.".to_string(),
            Ok(count) => format!("✓ Stopped {count} background server(s)."),
            Err(e) => format!("✕ Failed to stop server: {e}"),
        },
    }
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
    match items.is_empty() {
        true => "No previous sessions found for this channel.".to_string(),
        false => {
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
    }
}

pub async fn start_new_channel_session(
    session_manager: &crate::session::SessionManager,
    active_key: &str,
) -> anyhow::Result<bool> {
    match session_manager.load(active_key) {
        Ok(mut current_session) => match current_session.messages.is_empty() {
            false => {
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                current_session.key = format!("{}:history_{}", active_key, timestamp);
                session_manager.save(&current_session).await?;
            }
            true => {}
        },
        Err(_) => {}
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
    match selected_key == active_key {
        true => return Ok("Already using that session.".to_string()),
        false => {}
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
    match cmd {
        "/new-session" => Some(
            match start_new_channel_session(session_manager, active_key).await {
                Ok(_) => "Session reset. Starting a new session.".to_string(),
                Err(e) => format!("Failed to start new session: {e}"),
            },
        ),
        "/resume" => {
            let args: Vec<&str> = trimmed.split_whitespace().collect();
            let sessions = list_channel_sessions(&session_manager.dir, active_key, 10);
            match args.get(1) {
                None => Some(render_resume_list(&sessions, "/resume")),
                Some(raw_idx) => Some(match raw_idx.parse::<usize>() {
                    Ok(index) => match index == 0 || index > sessions.len() {
                        true => format!(
                            "Invalid session number. Use /resume to list 1..{}.",
                            sessions.len()
                        ),
                        false => {
                            match resume_channel_session(
                                session_manager,
                                active_key,
                                &sessions[index - 1].key,
                            )
                            .await
                            {
                                Ok(msg) => msg,
                                Err(e) => format!("Failed to resume session: {e}"),
                            }
                        }
                    },
                    Err(_) => "Usage: /resume or /resume <number>".to_string(),
                }),
            }
        }
        _ => None,
    }
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
    match std::fs::read_dir(session_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let target_opt = path
                    .is_file()
                    .then(|| path.file_name().and_then(|f| f.to_str()))
                    .flatten()
                    .and_then(|filename| filename.strip_prefix(prefix))
                    .and_then(|s| s.strip_suffix(".json"))
                    .filter(|target_str| {
                        !target_str.contains("history") && !target_str.contains("direct")
                    });
                match target_opt {
                    Some(target_str) => targets.push(target_str.to_string()),
                    None => {}
                }
            }
        }
        Err(_) => {}
    }
    targets
}

pub fn select_random_message(messages: &[&str]) -> String {
    match messages.is_empty() {
        true => String::new(),
        false => {
            use rand::Rng;
            let idx = rand::thread_rng().gen_range(0..messages.len());
            messages[idx].to_string()
        }
    }
}

static EXTERNAL_BOT_CHANNEL_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn mark_external_bot_channel_active() {
    EXTERNAL_BOT_CHANNEL_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn strip_ansi_codes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_escape = false;
    for ch in input.chars() {
        match (in_escape, ch) {
            (false, '\x1b') => in_escape = true,
            (false, other) => out.push(other),
            (true, c) => match c.is_ascii_alphabetic() {
                true => in_escape = false,
                false => {}
            },
        }
    }
    out
}

pub async fn shutdown_gateways(config: &crate::config::schema::Config) {
    let silent = std::env::var("OPENZ_SILENT").is_ok();
    match silent {
        false => crate::tui_println!("Shutting down gateways..."),
        true => {}
    }

    match EXTERNAL_BOT_CHANNEL_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
        true => {
            let sessions_dir = crate::config::loader::sessions_dir();
            let client =
                crate::core::http::custom_http_client(SHUTDOWN_HTTP_TIMEOUT, SHUTDOWN_HTTP_TIMEOUT);

            let offline_message = select_random_message(OFFLINE_MESSAGES);
            for request in
                build_external_notification_requests(config, &sessions_dir, &offline_message)
            {
                send_external_notification(&client, &request).await;
            }
        }
        false => {}
    }

    // Unload the active Ollama model and stop the local service when spawned locally
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
        let should_include =
            config.is_provider_available(&name) && !configured.iter().any(|(n, _)| n == &name);
        match should_include {
            true => {
                let default_model = config.custom_provider_default_model(&name);
                let display = format!(
                    "Custom: {} ({})",
                    name,
                    default_model.as_deref().unwrap_or("custom model")
                );
                configured.push((name, display));
            }
            false => {}
        }
    }

    configured
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    match a.len() == b.len() {
        false => false,
        true => {
            let mut accum = 0;
            for (x, y) in a.iter().zip(b.iter()) {
                accum |= x ^ y;
            }
            accum == 0
        }
    }
}

pub fn secure_compare(a: &str, b: &str) -> bool {
    use sha2::{Digest, Sha256};
    let hash_a = Sha256::digest(a.as_bytes());
    let hash_b = Sha256::digest(b.as_bytes());
    constant_time_eq(&hash_a, &hash_b)
}

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

pub use discord::DiscordChannel;
pub use email::EmailChannel;
pub use model_catalog::{
    configured_provider_model_options, configured_provider_models, curated_models_for,
    fetch_provider_models, preview_models_for_provider, provider_model_catalog,
    provider_model_catalog_options, provider_model_option_by_name, provider_models_by_name,
    resolved_provider_models_for_webui, ProviderModels, ProviderModelsOption, PROVIDER_REGISTRY,
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
    crate::channels::ratatui::send_ratatui_turn_event(
        crate::channels::ratatui::TurnEvent::SingleMessage(
            crate::channels::ratatui::app::ChatMessage::notice(msg.to_string()),
        ),
    );

    let clean_msg = strip_ansi_codes(msg);
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(async move {
                let ws_senders = match get_active_ws_senders().lock() {
                    Ok(senders) => senders
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<Vec<_>>(),
                    Err(_) => Vec::new(),
                };

                let evt = crate::channels::websocket::protocol::notification(clean_msg);
                match serde_json::to_string(&evt) {
                    Ok(evt_str) => {
                        for (id, tx) in ws_senders {
                            match tx.send(Message::Text(evt_str.clone())).await {
                                Ok(()) => {}
                                Err(_) => match get_active_ws_senders().lock() {
                                    Ok(mut senders) => {
                                        senders.remove(&id);
                                    }
                                    Err(_) => {}
                                },
                            }
                        }
                    }
                    Err(_) => {}
                }
            });
        }
        Err(_) => {}
    }
}

pub fn send_session_notice(session_key: &str, msg: &str) {
    match (
        session_key.starts_with("cli:"),
        session_key.starts_with("ws:"),
    ) {
        (true, _) => {
            match crate::channels::ratatui::is_ratatui_session_active(Some(session_key)) {
                true => {
                    let _ = crate::channels::ratatui::RATATUI_EVENT_CHANNEL.0.send(
                        crate::channels::ratatui::TurnEvent::SingleMessage(
                            crate::channels::ratatui::app::ChatMessage::notice(msg.to_string()),
                        ),
                    );
                }
                false => {}
            }
        }
        (false, true) => {
            let clean_msg = strip_ansi_codes(msg);
            match crate::channels::websocket::ws_chat_id(session_key) {
                Some(chat_id) => {
                    let evt = crate::channels::websocket::protocol::notification(clean_msg);
                    crate::channels::websocket::events::publish_ws_event_for_chat(&chat_id, evt);
                }
                None => {}
            }
        }
        (false, false) => {}
    }
}

pub fn send_external_notification_broadcast(msg: &str) {
    let clean_msg = strip_ansi_codes(msg);
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(async move {
                match crate::config::loader::load_config() {
                    Ok(config) => {
                        let sessions_dir = crate::config::loader::sessions_dir();
                        let client = crate::core::http::custom_http_client(
                            SHUTDOWN_HTTP_TIMEOUT,
                            SHUTDOWN_HTTP_TIMEOUT,
                        );

                        for request in build_external_notification_requests(
                            &config,
                            &sessions_dir,
                            &clean_msg,
                        ) {
                            send_external_notification(&client, &request).await;
                        }
                    }
                    Err(_) => {}
                }
            });
        }
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests;
