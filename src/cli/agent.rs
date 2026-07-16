use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::agent::style::{select_menu_with_history, HistoryItem};
use crate::channels::{
    Channel, CliChannel, DiscordChannel, EmailChannel, TelegramChannel, WhatsAppChannel, WsGateway,
};
use crate::cli::builder::build_agent_loop;
use crate::config::loader::{load_config, resolve_path};
use crate::cron::scheduler::start_scheduler;
use crate::session::SessionManager;
use crate::{eprintln, println};

#[derive(Deserialize)]
struct SessionMetadataOnly {
    key: String,
    updated_at: DateTime<Utc>,
    messages: Vec<MessageMetadataOnly>,
}

#[derive(Deserialize)]
struct MessageMetadataOnly {
    role: String,
    content: String,
}

pub fn load_session_history() -> Result<Vec<HistoryItem>> {
    let sessions_dir = resolve_path("~/.openz/sessions");
    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    for entry in std::fs::read_dir(sessions_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(session) = serde_json::from_str::<SessionMetadataOnly>(&content) {
                    if !session.messages.is_empty() {
                        let preview = session
                            .messages
                            .iter()
                            .find(|m| m.role == "user")
                            .map(|m| {
                                let mut text = m.content.clone();
                                if text.len() > 50 {
                                    text.truncate(47);
                                    text.push_str("...");
                                }
                                text
                            })
                            .unwrap_or_else(|| "Empty session".to_string());

                        items.push(HistoryItem {
                            key: session.key.clone(),
                            display_title: preview,
                            updated_at: session.updated_at,
                        });
                    }
                }
            }
        }
    }

    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(items)
}

pub async fn archive_current_session(
    session_manager: &SessionManager,
    session_key: &str,
) -> Result<()> {
    if let Ok(mut current_session) = session_manager.load(session_key) {
        if !current_session.messages.is_empty() {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
            let archive_key = format!("cli:history_{}", timestamp);
            current_session.key = archive_key;
            session_manager.save(&current_session).await?;

            let empty_session = crate::session::Session::new(session_key);
            session_manager.save(&empty_session).await?;
        }
    }
    Ok(())
}

pub async fn handle_agent() -> Result<()> {
    let config = load_config()?;
    let defaults = config.agents.defaults.clone();
    let session_key = crate::config::loader::get_cli_session_key();
    start_scheduler(config.clone());

    let sessions_dir = resolve_path("~/.openz/sessions");
    let session_manager = SessionManager::new(sessions_dir);

    let history = load_session_history()?;
    if history.is_empty() {
        archive_current_session(&session_manager, &session_key).await?;
    } else {
        let selected = select_menu_with_history("Welcome to OpenZ! Select an option:", &history)?;
        if selected == 0 {
            archive_current_session(&session_manager, &session_key).await?;
        } else {
            let selected_item = &history[selected - 1];
            if selected_item.key != session_key {
                archive_current_session(&session_manager, &session_key).await?;
                let mut session = session_manager.load(&selected_item.key)?;
                session.key = session_key.clone();
                session_manager.save(&session).await?;
            }
        }
    }

    let agent_loop = build_agent_loop(config.clone()).await?;
    let tui_started_at = chrono::Utc::now().to_rfc3339();
    let tui_cwd = std::env::current_dir().unwrap_or_default();
    let initial_preview = session_manager
        .load(&session_key)
        .ok()
        .map(|session| crate::agent::activity::session_preview_from_messages(&session.messages))
        .unwrap_or_else(|| "No user prompt yet".to_string());
    let active_tui = crate::agent::activity::make_active_tui_session(
        &session_key,
        &tui_cwd,
        &tui_started_at,
        &defaults.model,
        &defaults.provider,
        &initial_preview,
    );
    let _ = crate::agent::activity::upsert_active_tui_session(&active_tui);
    let heartbeat_session_key = session_key.clone();
    let heartbeat_cwd = tui_cwd.clone();
    let heartbeat_started_at = tui_started_at.clone();
    let heartbeat_model = defaults.model.clone();
    let heartbeat_provider = defaults.provider.clone();
    let heartbeat_session_manager = session_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            let preview = heartbeat_session_manager
                .load(&heartbeat_session_key)
                .ok()
                .map(|session| {
                    crate::agent::activity::session_preview_from_messages(&session.messages)
                })
                .unwrap_or_else(|| "No user prompt yet".to_string());
            let active_tui = crate::agent::activity::make_active_tui_session(
                &heartbeat_session_key,
                &heartbeat_cwd,
                &heartbeat_started_at,
                &heartbeat_model,
                &heartbeat_provider,
                &preview,
            );
            let _ = crate::agent::activity::upsert_active_tui_session(&active_tui);
        }
    });

    // Mark silent mode for background channels via thread-safe AtomicBool
    crate::cli::set_silent_mode(true);

    // Auto-start WebSocket gateway in the background if enabled and configured to start on TUI
    if let Some(ws_config) = &config.channels.websocket {
        if ws_config.enabled && ws_config.start_on_tui {
            let config_clone = config.clone();
            let ws_config_clone = ws_config.clone();
            tokio::spawn(async move {
                if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                    let gateway = WsGateway::new(ws_config_clone, agent_loop);
                    let _ = gateway.start().await;
                }
            });
        }
    }

    // Auto-start Telegram channel in the background if enabled
    if let Some(tg_config) = &config.channels.telegram {
        if tg_config.enabled {
            let token = if tg_config.bot_token.is_empty() {
                std::env::var("TELEGRAM_BOT_TOKEN").ok()
            } else {
                Some(tg_config.bot_token.clone())
            };
            if let Some(token) = token {
                let config_clone = config.clone();
                tokio::spawn(async move {
                    if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                        let channel = TelegramChannel::new(token, agent_loop);
                        let _ = channel.start().await;
                    }
                });
            }
        }
    }

    // Auto-start Discord channel in the background if enabled
    if let Some(dc_config) = &config.channels.discord {
        if dc_config.enabled {
            let token = if dc_config.bot_token.is_empty() {
                std::env::var("DISCORD_BOT_TOKEN").ok()
            } else {
                Some(dc_config.bot_token.clone())
            };
            if let Some(token) = token {
                let config_clone = config.clone();
                tokio::spawn(async move {
                    if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                        let channel = DiscordChannel::new(token, agent_loop);
                        let _ = channel.start().await;
                    }
                });
            }
        }
    }

    // Auto-start WhatsApp channel in the background if enabled
    if let Some(wa_config) = &config.channels.whatsapp {
        if wa_config.enabled {
            let config_clone = config.clone();
            let wa_config_clone = wa_config.clone();
            tokio::spawn(async move {
                if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                    let channel = WhatsAppChannel::new(
                        wa_config_clone.api_key,
                        wa_config_clone.phone_number_id,
                        agent_loop,
                    );
                    let _ = channel.start().await;
                }
            });
        }
    }

    // Auto-start Email channel in the background if enabled
    if let Some(email_config) = &config.channels.email {
        if email_config.enabled {
            let config_clone = config.clone();
            tokio::spawn(async move {
                if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                    let channel = EmailChannel::new(agent_loop);
                    let _ = channel.start().await;
                }
            });
        }
    }

    let channel = CliChannel::new(agent_loop, defaults);
    let mut shutdown_rx = match crate::shutdown::receiver() {
        Some(rx) => rx,
        None => {
            let (_, rx) = tokio::sync::watch::channel(false);
            rx
        }
    };

    tokio::select! {
        res = channel.start() => {
            if let Err(e) = res {
                eprintln!("TUI error: {}", e);
            }
        }
        _ = shutdown_rx.changed() => {
            println!("\r\nExiting OpenZ...");
        }
    }

    crate::agent::activity::remove_active_tui_session(&session_key);
    crate::shutdown::trigger();
    crate::channels::shutdown_gateways_bounded(&config).await;
    Ok(())
}
