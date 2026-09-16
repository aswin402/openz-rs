use crate::agent::AgentLoop;
use crate::config::schema::WebSocketChannelConfig;
use axum::{
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::ServeDir;

pub(crate) mod protocol;
pub(crate) mod auth;
pub(crate) mod approvals;
pub(crate) mod attachments;
pub(crate) mod events;
pub(crate) mod commands;
pub(crate) mod turns;
pub(crate) mod socket;
pub(crate) mod handlers;
#[cfg(test)]
mod tests;

pub use approvals::{
    cancel_ws_approvals_for_client, current_ws_approval_context, register_ws_approval,
    resolve_ws_approval, with_ws_approval_context, WsApprovalContext,
};
pub use turns::{
    cancel_ws_turn, cancel_ws_turn_for_client_chat, cancel_ws_turns_for_client, register_ws_turn,
    remove_ws_turn,
};
pub use events::{
    publish_activity_notice, publish_orchestration_event, publish_ws_event, ws_chat_id,
};
#[cfg(test)]
pub(crate) use events::remove_active_ws_sender;
pub(crate) use events::normalize_ws_chat_id;
#[cfg(test)]
pub(crate) use commands::cron::{cron_logs_event, cron_update_event};
#[cfg(test)]
pub(crate) use commands::config::config_update_requires_gateway_token;
#[cfg(test)]
pub(crate) use commands::config::mask_config_secret;
#[cfg(test)]
pub(crate) use commands::sessions::{archive_session_event, delete_session_event};
pub(crate) use attachments::{
    ATTACHMENT_ALLOWED_MIME_TYPES, ATTACHMENT_TTL,
    MAX_ATTACHMENT_BYTES, MAX_ATTACHMENT_COUNT, MAX_ATTACHMENT_TOTAL_BYTES,
    MAX_WS_MESSAGE_SIZE,
};

pub(crate) use auth::{
    gateway_token_configured, gateway_token_required_for_host, websocket_cors_origins,
};

pub(crate) use socket::{send_event, ws_handler};
pub(crate) use handlers::{
    hono_log_middleware, openai_chat_completions, resume_sop_handler, trigger_sop_handler,
};

pub struct WsGateway {
    config: WebSocketChannelConfig,
    agent_loop: Arc<AgentLoop>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct WsState {
    pub(crate) config: WebSocketChannelConfig,
    pub(crate) agent_loop: Arc<AgentLoop>,
    pub(crate) live_config: Arc<std::sync::RwLock<crate::config::schema::Config>>,
    pub(crate) _config_watcher: Arc<Option<crate::config::watcher::ConfigWatcherGuard>>,
}

impl WsState {
    pub(crate) fn current_config(&self) -> crate::config::schema::Config {
        self.live_config
            .read()
            .map(|config| config.clone())
            .unwrap_or_else(|_| self.agent_loop.config.clone())
    }
}

/// Deliver an event to one authenticated WebSocket client.
pub fn publish_ws_event_to_client(client_id: &str, event: serde_json::Value) -> bool {
    let Ok(event_str) = serde_json::to_string(&event) else {
        return false;
    };
    let sender = crate::channels::get_active_ws_senders()
        .lock()
        .ok()
        .and_then(|senders| senders.get(client_id).cloned());
    sender
        .map(|sender| sender.try_send(axum::extract::ws::Message::Text(event_str)).is_ok())
        .unwrap_or(false)
}

pub(crate) fn security_response_rejected_event(req_id: &str, chat_id: &str) -> serde_json::Value {
    protocol::security_response_rejected(req_id, chat_id)
}

pub(crate) fn webui_capabilities(config: &crate::config::schema::Config) -> serde_json::Value {
    let providers = crate::channels::provider_model_catalog_options(config, false)
        .into_iter()
        .map(|provider| {
            let config_key = if provider.name == "z.ai" {
                "z_ai".to_string()
            } else {
                provider.name.clone()
            };
            serde_json::json!({
                "name": provider.name,
                "configKey": config_key,
                "display": provider.display,
                "available": provider.available,
                "apiBase": crate::config::provider_catalog::find_provider(&provider.name)
                    .map(|descriptor| descriptor.default_api_base),
                "apiBaseEditable": provider.name != "anthropic"
                    && provider.name != "google_ai_studio",
            })
        })
        .collect::<Vec<_>>();

    let security_modes = [
        ("strict", "Strict"),
        ("normal", "Normal"),
        ("loose", "Loose"),
    ]
    .into_iter()
    .map(|(value, label)| serde_json::json!({ "value": value, "label": label }))
    .collect::<Vec<_>>();

    let channels = vec![
        serde_json::json!({
            "name": "telegram",
            "label": "Telegram Bot Listener",
            "fields": [
                { "key": "enabled", "label": "Enabled", "kind": "boolean" },
                { "key": "bot_token", "label": "Bot API Token", "kind": "secret" }
            ],
            "defaults": { "enabled": false, "bot_token": "" }
        }),
        serde_json::json!({
            "name": "discord",
            "label": "Discord Bot Gateway",
            "fields": [
                { "key": "enabled", "label": "Enabled", "kind": "boolean" },
                { "key": "bot_token", "label": "Bot Token", "kind": "secret" }
            ],
            "defaults": { "enabled": false, "bot_token": "" }
        }),
        serde_json::json!({
            "name": "whatsapp",
            "label": "WhatsApp Webhook Receiver",
            "fields": [
                { "key": "enabled", "label": "Enabled", "kind": "boolean" },
                { "key": "api_key", "label": "API Key", "kind": "secret" },
                { "key": "phone_number_id", "label": "Phone Number ID", "kind": "text" },
                { "key": "webhook_port", "label": "Webhook Port", "kind": "number" },
                { "key": "verify_token", "label": "Verify Token", "kind": "secret" }
            ],
            "defaults": {
                "enabled": false,
                "api_key": "",
                "phone_number_id": "",
                "webhook_port": 8090,
                "verify_token": ""
            }
        })
    ];

    protocol::capabilities(
        providers,
        security_modes,
        channels,
        config.browser.firefox_webdriver_port,
        config.browser.firefox_attach_port,
        MAX_ATTACHMENT_COUNT,
        MAX_ATTACHMENT_BYTES,
        MAX_ATTACHMENT_TOTAL_BYTES,
        MAX_WS_MESSAGE_SIZE,
        ATTACHMENT_TTL.as_secs(),
        ATTACHMENT_ALLOWED_MIME_TYPES,
    )
}

impl WsGateway {
    pub fn new(config: WebSocketChannelConfig, agent_loop: AgentLoop) -> Self {
        WsGateway {
            config,
            agent_loop: Arc::new(agent_loop),
        }
    }

    pub fn build_app(&self) -> Router {
        let live_config = Arc::new(std::sync::RwLock::new(self.agent_loop.config.clone()));
        let config_watcher = match crate::config::watcher::spawn_default_config_watcher(
            live_config.clone(),
            move |updated_config| {
                let event = commands::config::config_updated_event(updated_config);
                events::publish_ws_event(event);
            },
        ) {
            Ok(guard) => Some(guard),
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize configuration file watcher: {:#}. Live config reload disabled.",
                    e
                );
                None
            }
        };

        let state = WsState {
            config: self.config.clone(),
            live_config,
            agent_loop: self.agent_loop.clone(),
            _config_watcher: Arc::new(config_watcher),
        };
        // Keep REST CORS aligned with the WebSocket origin policy and local dev ports.
        let has_wildcard = self.config.cors_origins.iter().any(|o| o == "*");
        let allow_origin = if has_wildcard {
            AllowOrigin::any()
        } else {
            AllowOrigin::list(websocket_cors_origins(&self.config))
        };
        let cors = CorsLayer::new()
            .allow_origin(allow_origin)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers(Any);

        let mut app = Router::new()
            .route("/ws", get(ws_handler))
            .route(
                "/v1/chat/completions",
                axum::routing::post(openai_chat_completions),
            )
            .route(
                "/webhook/sop/trigger/:sop_id",
                axum::routing::post(trigger_sop_handler),
            )
            .route(
                "/webhook/sop/instances/:instance_id/resume",
                axum::routing::post(resume_sop_handler),
            )
            .layer(axum::middleware::from_fn(hono_log_middleware))
            .layer(cors)
            .with_state(state);

        let silent = std::env::var("OPENZ_SILENT").is_ok();
        let addr_str = format!("{}:{}", self.config.host, self.config.port);
        if let Some(dist_path) = find_web_dist() {
            if !silent {
                println!("🌐 Serving WebUI static files from {:?}", dist_path);
            }
            let index_file = dist_path.join("index.html");
            let serve_dir = ServeDir::new(&dist_path)
                .fallback(tower_http::services::ServeFile::new(index_file));
            app = app.fallback_service(serve_dir);
        } else if !silent {
            println!(
                "⚠️ WebUI static directory not found. Serving WebSocket API only at ws://{}/ws",
                addr_str
            );
        }

        app
    }

    pub async fn start_with_listener(&self, listener: TcpListener) -> anyhow::Result<()> {
        let app = self.build_app();
        let silent = std::env::var("OPENZ_SILENT").is_ok();
        let local_addr = listener.local_addr()?;

        if !silent {
            println!("⚡ OpenZ Gateway running on http://{}", local_addr);
            if std::env::var("OPENZ_GATEWAY_TOKEN")
                .map(|t| t.is_empty())
                .unwrap_or(true)
            {
                println!("ℹ️  OPENZ_GATEWAY_TOKEN is not set. Gateway is open for local access.");
                println!(
                    "   Set OPENZ_GATEWAY_TOKEN to require authentication for remote clients."
                );
            }
        }

        if let Some(mut shutdown_rx) = crate::shutdown::receiver() {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    if *shutdown_rx.borrow() {
                        return;
                    }
                    let _ = shutdown_rx.changed().await;
                })
                .await?;
        } else {
            axum::serve(listener, app).await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl super::Channel for WsGateway {
    fn name(&self) -> &'static str {
        "websocket"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let addr_str = format!("{}:{}", self.config.host, self.config.port);
        let addr: SocketAddr = addr_str.parse()?;

        if gateway_token_required_for_host(&self.config.host) && !gateway_token_configured() {
            return Err(anyhow::anyhow!(
                "OPENZ_GATEWAY_TOKEN is required when binding gateway to non-loopback host '{}'",
                self.config.host
            ));
        }

        let listener = TcpListener::bind(addr).await?;
        self.start_with_listener(listener).await
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

fn find_web_dist() -> Option<std::path::PathBuf> {
    // 1. Local ./web/dist
    let local = Path::new("./web/dist");
    if local.exists() && local.is_dir() && local.join("index.html").exists() {
        let global = crate::config::loader::config_dir().join("web/dist");
        let _ = copy_dir_all(local, &global);
        return Some(local.to_path_buf());
    }

    // 2. Global ~/.openz/web/dist
    let global = crate::config::loader::config_dir().join("web/dist");
    if global.exists() && global.is_dir() && global.join("index.html").exists() {
        return Some(global);
    }

    // 3. Alternative legacy path
    let legacy = Path::new("./nanobot/nanobot/web/dist");
    if legacy.exists() && legacy.is_dir() && legacy.join("index.html").exists() {
        return Some(legacy.to_path_buf());
    }

    None
}
