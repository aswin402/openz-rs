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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSwitchCommand {
    None,
    ShowProviders,
    ShowModels { provider: String },
    Set { provider: String, model: String },
}

pub use crate::providers::risk::{classify_model_risk, ModelRisk};

pub fn render_model_risk_warning(provider: &str, model: &str) -> String {
    let risk = classify_model_risk(provider, model);
    if !risk.risky {
        return String::new();
    }
    let registry = crate::model_registry::ModelRegistry::load();
    let health = registry.get(provider, model);
    let mut out = format!(
        "\n\nWarning: `{model}` via `{provider}` is marked `{}`. OpenZ will still allow it, but weak-model prompt safeguards will be used when applicable.",
        risk.tier
    );
    for reason in risk.reasons {
        out.push_str(&format!("\n- {reason}"));
    }
    if let Some(record) = health {
        if record.failure_count > 0
            || record.blank_response_count > 0
            || record.think_leak_count > 0
        {
            out.push_str(&format!(
                "\n- prior health: {} failures, {} blank replies, {} think leaks",
                record.failure_count, record.blank_response_count, record.think_leak_count
            ));
        }
    }
    out
}

pub fn model_menu_options_with_prefs(provider: &str, models: Vec<String>) -> Vec<String> {
    let prefs = load_model_prefs();
    let mut out = Vec::new();
    out.push("★ Favorite/Unfavorite current model".to_string());

    for fav in prefs
        .favorites
        .iter()
        .filter(|entry| entry.provider == provider)
    {
        let label = format!("★ {}", fav.model);
        if !out.iter().any(|existing| existing == &label) {
            out.push(label);
        }
    }
    for recent in prefs
        .recent
        .iter()
        .filter(|entry| entry.provider == provider)
    {
        let label = format!("◷ {}", recent.model);
        if !out.iter().any(|existing| existing == &label) {
            out.push(label);
        }
    }
    for model in models {
        if !out
            .iter()
            .any(|existing| model_menu_model_name(existing).eq_ignore_ascii_case(&model))
        {
            out.push(model);
        }
    }
    out
}

pub fn model_menu_model_name(item: &str) -> &str {
    item.strip_prefix("★ ")
        .or_else(|| item.strip_prefix("◷ "))
        .unwrap_or(item)
}

pub fn parse_model_switch_command(text: &str) -> ModelSwitchCommand {
    let trimmed = text.trim();
    let mut parts = trimmed.split_whitespace();
    if parts.next() != Some("/switch-model") {
        return ModelSwitchCommand::None;
    }
    match (parts.next(), parts.next()) {
        (None, _) => ModelSwitchCommand::ShowProviders,
        (Some(provider), None) => ModelSwitchCommand::ShowModels {
            provider: provider.to_string(),
        },
        (Some(provider), Some(model)) => {
            let mut model_name = model.to_string();
            for rest in parts {
                model_name.push(' ');
                model_name.push_str(rest);
            }
            ModelSwitchCommand::Set {
                provider: provider.to_string(),
                model: model_name,
            }
        }
    }
}

pub fn model_switch_text_response(text: &str) -> Option<String> {
    let command = parse_model_switch_command(text);
    if command == ModelSwitchCommand::None {
        return None;
    }
    let config = match crate::config::loader::load_config() {
        Ok(config) => config,
        Err(e) => return Some(format!("Failed to load OpenZ config: {e}")),
    };
    Some(render_model_switch_command(&config, command))
}

pub fn render_model_switch_command(
    config: &crate::config::schema::Config,
    command: ModelSwitchCommand,
) -> String {
    match command {
        ModelSwitchCommand::None => String::new(),
        ModelSwitchCommand::ShowProviders => render_model_switch_providers(config),
        ModelSwitchCommand::ShowModels { provider } => {
            render_model_switch_models(config, &provider)
        }
        ModelSwitchCommand::Set { provider, model } => {
            match save_default_model_selection(config, &provider, &model) {
                Ok(()) => format!(
                    "Model switched to `{}` with provider `{}`. New channel turns will use this default.{}",
                    model,
                    provider,
                    render_model_risk_warning(&provider, &model)
                ),
                Err(e) => format!("Failed to switch model: {e}"),
            }
        }
    }
}

pub fn render_model_switch_providers(config: &crate::config::schema::Config) -> String {
    let providers = configured_provider_model_options(config);
    if providers.is_empty() {
        return "No configured LLM providers found. Run `openz configure` first.".to_string();
    }

    let mut response = format!(
        "Current default: `{}` via `{}`\n\nChoose a provider:\n",
        config.agents.defaults.model, config.agents.defaults.provider
    );
    for provider in providers {
        response.push_str(&format!("- `{}` ({})\n", provider.name, provider.display));
    }
    response.push_str("\nUsage: `/switch-model <provider>` to list models, then `/switch-model <provider> <model>` to switch.");
    response
}

pub fn render_model_switch_models(
    config: &crate::config::schema::Config,
    provider: &str,
) -> String {
    let Some(provider_models) = provider_model_option_by_name(config, provider) else {
        return format!("Unknown provider `{provider}`. Use `/switch-model` to list providers.");
    };
    if !config.is_provider_available(provider) {
        return format!(
            "Provider `{provider}` is not configured. Run `openz configure` or set its API key first."
        );
    }

    let mut response = format!(
        "Models for `{}` ({}):\n",
        provider_models.name, provider_models.display
    );
    if provider_models.models.is_empty() {
        response.push_str("- Type any OpenAI-compatible model name manually\n");
    } else {
        for model in &provider_models.models {
            response.push_str(&format!("- `{model}`\n"));
        }
    }
    response.push_str(&format!(
        "\nUsage: `/switch-model {} <model>`",
        provider_models.name
    ));
    response
}

fn spawn_model_smoke_test(mut config: crate::config::schema::Config, provider: &str, model: &str) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let provider = provider.to_string();
    let model = model.trim().to_string();
    config.agents.defaults.provider = provider.clone();
    config.agents.defaults.model = model.clone();
    handle.spawn(async move {
        let risk = classify_model_risk(&provider, &model);
        let reasons: Vec<String> = risk
            .reasons
            .iter()
            .map(|reason| reason.to_string())
            .collect();
        let provider_instance =
            match crate::providers::resolver::resolve_provider_full(&config, &model) {
                Ok(resolved) => resolved.instance,
                Err(err) => {
                    let _ = crate::model_registry::record_model_failure(
                        &provider,
                        &model,
                        risk.tier,
                        risk.risky,
                        reasons,
                        &format!("resolve failed during smoke test: {err}"),
                    );
                    return;
                }
            };
        let settings = crate::providers::GenerationSettings {
            temperature: 0.0,
            max_tokens: 32,
            reasoning_effort: None,
        };
        let messages = vec![crate::session::Message {
            role: "user".to_string(),
            content: "Reply exactly: OPENZ_MODEL_OK".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        }];
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(12),
            provider_instance.chat(
                "You are OpenZ model validation. Reply exactly with the requested token.",
                &messages,
                &[],
                &settings,
            ),
        )
        .await;
        match result {
            Ok(Ok(resp)) => {
                let content = resp.content.unwrap_or_default();
                let blank = content.trim().is_empty();
                let think_leak = content.contains("<think>") || content.contains("</think>");
                let ok = content.trim().contains("OPENZ_MODEL_OK") && !blank;
                if ok {
                    let _ = crate::model_registry::record_model_success(
                        &provider, &model, risk.tier, risk.risky, reasons, blank, think_leak, false,
                    );
                } else {
                    let _ = crate::model_registry::record_model_failure(
                        &provider,
                        &model,
                        risk.tier,
                        risk.risky,
                        reasons,
                        &format!("smoke test unexpected response: {}", content.trim()),
                    );
                }
            }
            Ok(Err(err)) => {
                let _ = crate::model_registry::record_model_failure(
                    &provider,
                    &model,
                    risk.tier,
                    risk.risky,
                    reasons,
                    &format!("smoke test provider error: {err}"),
                );
            }
            Err(_) => {
                let _ = crate::model_registry::record_model_failure(
                    &provider,
                    &model,
                    risk.tier,
                    risk.risky,
                    reasons,
                    "smoke test timed out after 12s",
                );
            }
        }
    });
}

pub fn save_default_model_selection(
    base_config: &crate::config::schema::Config,
    provider: &str,
    model: &str,
) -> anyhow::Result<()> {
    if provider_models_by_name(provider).is_none() && !base_config.is_custom_provider(provider) {
        anyhow::bail!("unknown provider `{provider}`");
    }
    if !base_config.is_provider_available(provider) {
        anyhow::bail!("provider `{provider}` is not configured");
    }
    if model.trim().is_empty() {
        anyhow::bail!("model cannot be empty");
    }

    let risk = classify_model_risk(provider, model);
    let _ = crate::model_registry::record_model_risk(
        provider,
        model.trim(),
        risk.tier,
        risk.risky,
        risk.reasons
            .iter()
            .map(|reason| reason.to_string())
            .collect(),
    );

    let mut config = crate::config::loader::load_config().unwrap_or_else(|_| base_config.clone());
    config.agents.defaults.provider = provider.to_string();
    config.agents.defaults.model = model.trim().to_string();
    crate::config::loader::save_config(&config)?;
    spawn_model_smoke_test(config, provider, model);
    Ok(())
}

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
mod tests {
    use super::notifications::{build_whatsapp_notification_requests, NotificationAuth};

    #[test]
    fn test_shutdown_timeout_is_short_enough_for_interactive_exit() {
        assert!(super::SHUTDOWN_HTTP_TIMEOUT.as_secs() <= 3);
        assert!(super::SHUTDOWN_GATEWAYS_TIMEOUT.as_secs() <= 5);
    }

    use super::*;

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
                value: "Bot dc-token".to_string()
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

#[cfg(test)]
mod channel_session_tests {
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

#[cfg(test)]
mod model_switch_tests {
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
                provider: "deepseek".to_string()
            }
        );
        assert_eq!(
            parse_model_switch_command("/switch-model opencode_zen deepseek-v4-flash-free"),
            ModelSwitchCommand::Set {
                provider: "opencode_zen".to_string(),
                model: "deepseek-v4-flash-free".to_string()
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
