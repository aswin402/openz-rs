use crate::agent::AgentLoop;
use crate::config::schema::WebSocketChannelConfig;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path as AxumPath, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::ServeDir;

pub(crate) mod protocol;
pub(crate) mod auth;
pub(crate) mod approvals;
pub(crate) mod attachments;
pub(crate) mod events;
pub(crate) mod commands;
pub(crate) mod turns;
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
pub(crate) use turns::WsTurnGuard;
pub use events::{
    publish_activity_notice, publish_orchestration_event, publish_ws_event, ws_chat_id,
};
#[cfg(test)]
pub(crate) use events::{
    active_ws_sender_snapshot_for_chat, remove_active_ws_sender,
};
pub(crate) use events::normalize_ws_chat_id;
#[cfg(test)]
pub(crate) use commands::cron::{cron_logs_event, cron_update_event};
#[cfg(test)]
pub(crate) use commands::config::config_update_requires_gateway_token;
#[cfg(test)]
pub(crate) use commands::config::mask_config_secret;
pub(crate) use commands::memory::cognitive_memory_event;
pub(crate) use commands::observability::{logs_event, mcp_servers_event};
pub(crate) use commands::sessions::{
    archive_session_event, delete_session_event, fetch_real_session_history,
    fetch_real_sessions_list, fetch_real_sessions_list_paginated, resolve_session_key,
};
pub(crate) use commands::system::runtime_inventory_event;
pub(crate) use attachments::{
    persist_attachments, ATTACHMENT_ALLOWED_MIME_TYPES, ATTACHMENT_TTL,
    MAX_ATTACHMENT_BYTES, MAX_ATTACHMENT_COUNT, MAX_ATTACHMENT_TOTAL_BYTES,
    MAX_WS_MESSAGE_SIZE,
};
#[cfg(test)]
pub(crate) use attachments::{
    attachment_mime_allowed, attachment_total_within_quota,
};

pub(crate) use auth::{
    gateway_token_configured, gateway_token_required_for_host, is_authorized,
    websocket_cors_origins, websocket_origin_allowed,
};

pub struct WsGateway {
    config: WebSocketChannelConfig,
    agent_loop: Arc<AgentLoop>,
}

#[allow(dead_code)]
#[derive(Clone)]
struct WsState {
    config: WebSocketChannelConfig,
    agent_loop: Arc<AgentLoop>,
    live_config: Arc<std::sync::RwLock<crate::config::schema::Config>>,
    _config_watcher: Arc<Option<crate::config::watcher::ConfigWatcherGuard>>,
}

impl WsState {
    fn current_config(&self) -> crate::config::schema::Config {
        self.live_config
            .read()
            .map(|config| config.clone())
            .unwrap_or_else(|_| self.agent_loop.config.clone())
    }
}

/// Serialize and enqueue one connection-local event while preserving the
/// gateway's existing best-effort send behavior.
async fn send_event(sender: &mpsc::Sender<Message>, event: Value) {
    let Ok(event_str) = serde_json::to_string(&event) else {
        return;
    };
    let _ = sender.send(Message::Text(event_str)).await;
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
        .map(|sender| sender.try_send(Message::Text(event_str)).is_ok())
        .unwrap_or(false)
}

fn security_response_rejected_event(req_id: &str, chat_id: &str) -> serde_json::Value {
    protocol::security_response_rejected(req_id, chat_id)
}

fn webui_capabilities(config: &crate::config::schema::Config) -> serde_json::Value {
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
        let cors = CorsLayer::new()
            .allow_origin(AllowOrigin::list(websocket_cors_origins(&self.config)))
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

async fn hono_log_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = redact_query_token(req.uri().query().unwrap_or(""));
    let full_path = if query.is_empty() {
        path.clone()
    } else {
        format!("{}?{}", path, query)
    };

    let method_str = method.as_str();
    let method_colored = match method_str {
        "GET" => "\x1b[1;36mGET\x1b[0m",       // Cyan
        "POST" => "\x1b[1;35mPOST\x1b[0m",     // Magenta
        "PUT" => "\x1b[1;33mPUT\x1b[0m",       // Yellow
        "DELETE" => "\x1b[1;31mDELETE\x1b[0m", // Red
        _ => "\x1b[1;32mGET\x1b[0m",
    };

    let silent = std::env::var("OPENZ_SILENT").is_ok();
    if !silent {
        println!(
            "  \x1b[1;30m-->\x1b[0m {} \x1b[37m{}\x1b[0m",
            method_colored, full_path
        );
    }

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status().as_u16();

    let status_colored = if (200..300).contains(&status) {
        format!("\x1b[1;32m{}\x1b[0m", status) // Green
    } else if (300..400).contains(&status) {
        format!("\x1b[1;33m{}\x1b[0m", status) // Yellow
    } else {
        format!("\x1b[1;31m{}\x1b[0m", status) // Red
    };

    let duration_str = if duration.as_secs() > 0 {
        format!("{:.2}s", duration.as_secs_f64())
    } else if duration.as_millis() > 0 {
        format!("{}ms", duration.as_millis())
    } else {
        format!("{}μs", duration.as_micros())
    };

    if !silent {
        println!(
            "  \x1b[1;30m<--\x1b[0m {} \x1b[37m{}\x1b[0m {} \x1b[1;30m({})\x1b[0m",
            method_colored, path, status_colored, duration_str
        );
    }

    response
}

fn redact_query_token(query: &str) -> String {
    query
        .split('&')
        .map(|part| {
            let Some((key, _value)) = part.split_once('=') else {
                return part.to_string();
            };
            if key.eq_ignore_ascii_case("token") {
                format!("{key}=<redacted>")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

const MAX_WS_REQUEST_ID_LEN: usize = 128;

fn ws_request_id(envelope: &Value) -> Option<String> {
    let request_id = envelope.get("request_id").and_then(Value::as_str)?.trim();
    if request_id.is_empty() || request_id.len() > MAX_WS_REQUEST_ID_LEN {
        return None;
    }
    Some(request_id.to_string())
}

fn is_known_ws_command(command: &str) -> bool {
    commands::WsCommand::parse(command).is_some()
}

fn command_ack_event(
    request_id: &str,
    command: &str,
    status: &str,
    detail: Option<&str>,
) -> Value {
    protocol::command_ack(request_id, command, status, detail)
}


async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(state): State<WsState>,
) -> impl IntoResponse {
    let query_token = params.get("token").map(|s| s.as_str());
    let token_configured = gateway_token_configured();
    if !websocket_origin_allowed(
        headers.get(axum::http::header::ORIGIN).and_then(|value| value.to_str().ok()),
        &state.config,
        token_configured,
    ) {
        return (StatusCode::FORBIDDEN, "Untrusted WebSocket origin").into_response();
    }
    if !is_authorized(&headers, query_token) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    ws.max_message_size(MAX_WS_MESSAGE_SIZE)
        .max_frame_size(MAX_WS_MESSAGE_SIZE)
        .on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: WsState) {
    let client_id = format!("client-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let default_chat_id = uuid::Uuid::new_v4().to_string();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(1));

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    // Spawn dedicated write loop
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Register sender for the global notification broker
    if let Ok(mut senders) = crate::channels::get_active_ws_senders().lock() {
        senders.insert(client_id.clone(), tx.clone());
    }
    if let Ok(mut client_chats) = crate::channels::get_active_ws_client_chats().lock() {
        client_chats.insert(client_id.clone(), default_chat_id.clone());
    }

    struct WsSenderGuard(String);
    impl Drop for WsSenderGuard {
        fn drop(&mut self) {
            cancel_ws_turns_for_client(&self.0);
            cancel_ws_approvals_for_client(&self.0);
            if let Ok(mut senders) = crate::channels::get_active_ws_senders().lock() {
                senders.remove(&self.0);
            }
            if let Ok(mut client_chats) = crate::channels::get_active_ws_client_chats().lock() {
                client_chats.remove(&self.0);
            }
        }
    }
    let _guard = WsSenderGuard(client_id.clone());

    // Send ready event
    let ready_evt = protocol::ready(&default_chat_id, &client_id);
    if let Ok(ready_str) = serde_json::to_string(&ready_evt) {
        let _ = tx.send(Message::Text(ready_str)).await;
    }

    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            if text.len() > MAX_WS_MESSAGE_SIZE {
                continue;
            }
            let parsed: Result<Value, _> = serde_json::from_str(&text);
            if let Ok(envelope) = parsed {
                let msg_type = envelope.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(request_id) = ws_request_id(&envelope) {
                    let (status, detail) = if is_known_ws_command(msg_type) {
                        ("accepted", None)
                    } else {
                        ("rejected", Some("Unknown WebSocket command."))
                    };
                    let ack = command_ack_event(&request_id, msg_type, status, detail);
                    send_event(&tx, ack).await;
                }
                let chat_id = envelope
                    .get("chat_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&default_chat_id)
                    .to_string();
                if msg_type != "security_response" {
                    if let Ok(mut client_chats) = crate::channels::get_active_ws_client_chats().lock() {
                        client_chats.insert(client_id.clone(), chat_id.clone());
                    }
                }

                match msg_type {
                    "new_chat" => {
                        let new_id = uuid::Uuid::new_v4().to_string();
                        let attached_evt = protocol::attached(&new_id);
                        send_event(&tx, attached_evt).await;
                    }
                    "attach" => {
                        let attached_evt = protocol::attached(&chat_id);
                        send_event(&tx, attached_evt).await;
                        let history_evt =
                            fetch_real_session_history(&state.agent_loop.session_manager, &chat_id)
                                .await;
                        send_event(&tx, history_evt).await;
                    }
                    "list_sessions" => {
                        let offset = envelope.get("offset").and_then(Value::as_u64).map(|v| v as usize);
                        let limit = envelope.get("limit").and_then(Value::as_u64).map(|v| v as usize);
                        let evt = match (offset, limit) {
                            (Some(offset), Some(limit)) => {
                                fetch_real_sessions_list_paginated(
                                    &state.agent_loop.session_manager,
                                    offset,
                                    limit,
                                )
                                .await
                            }
                            _ => fetch_real_sessions_list(&state.agent_loop.session_manager).await,
                        };
                        send_event(&tx, evt).await;
                    }
                    "load_history" => {
                        let evt =
                            fetch_real_session_history(&state.agent_loop.session_manager, &chat_id)
                                .await;
                        send_event(&tx, evt).await;
                    }
                    "archive_session" | "delete_session" => {
                        let evt = if msg_type == "archive_session" {
                            archive_session_event(&state.agent_loop.session_manager, &envelope)
                                .await
                        } else {
                            delete_session_event(&state.agent_loop.session_manager, &envelope).await
                        };
                        send_event(&tx, evt).await;

                        let sessions_evt =
                            fetch_real_sessions_list(&state.agent_loop.session_manager).await;
                        send_event(&tx, sessions_evt).await;

                        let config = state.current_config();
                        let inventory_evt =
                            runtime_inventory_event(&config, Some(&state.agent_loop.tools));
                        send_event(&tx, inventory_evt).await;
                    }
                    "get_cognitive_memory" => {
                        let evt = cognitive_memory_event().await;
                        send_event(&tx, evt).await;
                    }
                    "get_mcp_servers" => {
                        let live = state.current_config();
                        let evt = mcp_servers_event(&live).await;
                        send_event(&tx, evt).await;
                    }
                    "get_logs" => {
                        let evt = logs_event().await;
                        send_event(&tx, evt).await;
                    }
                    "message" => {
                        let content = envelope
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let attachment_refs = persist_attachments(
                            envelope.get("attachments").unwrap_or(&Value::Null),
                        )
                        .await;
                        let content = if attachment_refs.is_empty() {
                            content
                        } else {
                            format!("{}\n\n{}", attachment_refs.join("\n"), content)
                        };

                        if crate::channels::is_stop_command(&content) {
                            let requested_turn_id = envelope
                                .get("turn_id")
                                .and_then(|value| value.as_str())
                                .map(str::trim)
                                .filter(|value| !value.is_empty());
                            let normalized_chat_id = normalize_ws_chat_id(&chat_id);
                            let stopped_turn_id = cancel_ws_turn_for_client_chat(
                                &client_id,
                                &normalized_chat_id,
                                requested_turn_id,
                            );
                            let had_active_turn = stopped_turn_id.is_some();
                            let stopped_evt = protocol::stopped(
                                chat_id.clone(),
                                stopped_turn_id,
                                if had_active_turn { "cancelled" } else { "idle" },
                                if had_active_turn {
                                    "Stop requested. Active OpenZ turn interrupted."
                                } else {
                                    "No active OpenZ turn for this client and chat."
                                },
                            );
                            send_event(&tx, stopped_evt).await;
                            continue;
                        }

                        if let Some(response_text) = crate::channels::session_command_text_response(
                            &state.agent_loop.session_manager,
                            &chat_id,
                            &content,
                        )
                        .await
                        {
                            let delta_evt =
                                protocol::delta_without_turn(chat_id.clone(), response_text);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                            continue;
                        }

                        if let Some(response_text) =
                            crate::channels::model_switch_text_response(&content)
                        {
                            let delta_evt =
                                protocol::delta_without_turn(chat_id.clone(), response_text);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                        }
                        if content.trim() == "/servers" {
                            let servers = crate::shutdown::list_registered_children();
                            let response = if servers.is_empty() {
                                "No OpenZ-launched background servers running.".to_string()
                            } else {
                                let mut res = "OpenZ background servers:\n".to_string();
                                for server in servers {
                                    res.push_str(&format!(
                                        "  #{} pid={} {} - {}\n",
                                        server.id, server.pid, server.kind, server.command
                                    ));
                                }
                                res.push_str("Use `/stop-server <id>` or `/stop-server all`.");
                                res
                            };
                            let delta_evt = protocol::delta_without_turn(chat_id.clone(), response);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                            continue;
                        }
                        if let Some(stripped) = content.trim().strip_prefix("/stop-server") {
                            let target = stripped.trim();
                            let response = if target.is_empty() {
                                "Usage: /stop-server <id|all>".to_string()
                            } else {
                                match crate::shutdown::stop_registered_child(target) {
                                    Ok(0) => "No matching background server found.".to_string(),
                                    Ok(count) => format!("✓ Stopped {count} background server(s)."),
                                    Err(e) => format!("✕ Failed to stop server: {e}"),
                                }
                            };
                            let delta_evt = protocol::delta_without_turn(chat_id.clone(), response);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                            continue;
                        }
                        if let Some(_stripped) = content.trim().strip_prefix("/device") {
                            let response = "Device clipboard and app suggestions are currently managed locally. To audit device details, use the CLI `openz agent`.".to_string();
                            let delta_evt = protocol::delta_without_turn(chat_id.clone(), response);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                            continue;
                        }
                        let state_clone = state.clone();
                        let tx_clone = tx.clone();
                        let chat_id_clone = chat_id.clone();
                        let content_str = content.to_string();
                        let sem_clone = semaphore.clone();
                        let msg_model = envelope
                            .get("model")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let msg_provider = envelope
                            .get("provider")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        tracing::info!(
                            "WS message received: chat_id='{}', msg_model={:?}, msg_provider={:?}",
                            chat_id_clone,
                            msg_model,
                            msg_provider
                        );

                        let turn_id = format!(
                            "turn-{}",
                            &uuid::Uuid::new_v4().to_string()[..12],
                        );
                        let turn_token = crate::tools::subagent::CancellationToken::new();
                        let normalized_chat_id = normalize_ws_chat_id(&chat_id_clone);
                        register_ws_turn(
                            turn_id.clone(),
                            client_id.clone(),
                            normalized_chat_id.clone(),
                            turn_token.clone(),
                        );
                        let turn_started_evt = protocol::turn_started(
                            chat_id_clone.clone(),
                            turn_id.clone(),
                        );
                        send_event(&tx, turn_started_evt).await;

                        let approval_context = WsApprovalContext {
                            client_id: client_id.clone(),
                            chat_id: normalized_chat_id,
                        };
                        let turn_context = crate::agent::agent_loop::TurnCancellationContext {
                            turn_id: turn_id.clone(),
                            token: turn_token,
                        };
                        let turn_id_for_guard = turn_id.clone();
                        let turn_id_for_events = turn_id.clone();
                        tokio::spawn(async move {
                            let _turn_guard = WsTurnGuard(turn_id_for_guard);
                            crate::agent::style::spinner::IS_WEBSOCKET.scope(true, async move {
                                crate::agent::agent_loop::with_turn_cancellation_context(
                                    turn_context,
                                    async move {
                                        with_ws_approval_context(approval_context, async move {
                                let _permit = match sem_clone.try_acquire() {
                                    Ok(p) => p,
                                    Err(_) => {
                                        let err_evt = protocol::error_for_turn(
                                            chat_id_clone.clone(),
                                            turn_id_for_events.clone(),
                                            "Rate limit exceeded: Only one message can be processed at a time.",
                                        );
                                        send_event(&tx_clone, err_evt).await;
                                        let turn_end_evt = protocol::turn_end(
                                            chat_id_clone.clone(),
                                            Some(turn_id_for_events.clone()),
                                        );
                                        send_event(&tx_clone, turn_end_evt).await;
                                        return;
                                    }
                                };

                                let mut config = state_clone.current_config();
                                // Apply model/provider overrides from the message payload
                                if let Some(model) = msg_model {
                                    config.agents.defaults.model = model;
                                }
                                if let Some(provider) = msg_provider {
                                    config.agents.defaults.provider = provider;
                                }
                                let agent_loop = match crate::cli::build_agent_loop(config).await {
                                    Ok(al) => al,
                                    Err(e) => {
                                        let err_evt = protocol::error_for_turn(
                                            chat_id_clone.clone(),
                                            turn_id_for_events.clone(),
                                            format!("Failed to build agent loop: {}", e),
                                        );
                                        send_event(&tx_clone, err_evt).await;
                                        let turn_end_evt = protocol::turn_end(
                                            chat_id_clone.clone(),
                                            Some(turn_id_for_events.clone()),
                                        );
                                        send_event(&tx_clone, turn_end_evt).await;
                                        return;
                                    }
                                };

                                let session_key = resolve_session_key(&agent_loop.session_manager, &chat_id_clone);
                                let workspace = crate::config::loader::workspace_for_agent_turn(&agent_loop.config);
                                let run_result = crate::config::loader::ACTIVE_WORKSPACE
                                    .scope(workspace, async { agent_loop.run(&content_str, &session_key).await })
                                    .await;

                                match run_result {
                                    Ok(res) => {
                                        // Streaming deltas are emitted live from the agent loop
                                        // (event "delta"); only send a full-content delta when
                                        // the turn did not stream.
                                        if !res.streamed {
                                            let delta_evt = protocol::delta(
                                                chat_id_clone.clone(),
                                                Some(turn_id_for_events.clone()),
                                                res.content,
                                            );
                                            send_event(&tx_clone, delta_evt).await;
                                        }

                                        let turn_end_evt = protocol::turn_end(
                                            chat_id_clone.clone(),
                                            Some(turn_id_for_events.clone()),
                                        );
                                        send_event(&tx_clone, turn_end_evt).await;
                                    }
                                    Err(e) => {
                                        let err_evt = protocol::error_for_turn(
                                            chat_id_clone.clone(),
                                            turn_id_for_events.clone(),
                                            e.to_string(),
                                        );
                                        send_event(&tx_clone, err_evt).await;
                                        let turn_end_evt = protocol::turn_end(
                                            chat_id_clone.clone(),
                                            Some(turn_id_for_events.clone()),
                                        );
                                        send_event(&tx_clone, turn_end_evt).await;
                                    }
                                }
                                        })
                                        .await
                                    },
                                )
                                .await
                            })
                            .await;
                        });
                    }
                    _ => {
                        commands::handle_post_message_command(
                            msg_type,
                            &envelope,
                            &chat_id,
                            &client_id,
                            &state,
                            &tx,
                        )
                        .await;
                    }
                }
            }
        }
    }

    // Clean up sender on disconnect
    if let Ok(mut senders) = crate::channels::get_active_ws_senders().lock() {
        senders.remove(&client_id);
    }
}

async fn trigger_sop_handler(
    State(state): State<WsState>,
    headers: axum::http::HeaderMap,
    AxumPath(sop_id): AxumPath<String>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !is_authorized(&headers, None) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }
    let config = state.current_config();
    match crate::sop::engine::trigger_sop(config, sop_id, payload).await {
        Ok(instance_id) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "triggered",
                "instance_id": instance_id
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )
            .into_response(),
    }
}

async fn resume_sop_handler(
    State(state): State<WsState>,
    headers: axum::http::HeaderMap,
    AxumPath(instance_id): AxumPath<String>,
) -> impl IntoResponse {
    if !is_authorized(&headers, None) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }
    let config = state.current_config();
    match crate::sop::engine::resume_sop(config, instance_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "resumed"
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct OpenAiChatCompletionRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[allow(dead_code)]
    stream: Option<bool>,
    user: Option<String>,
}

#[derive(serde::Deserialize, Clone)]
struct OpenAiMessage {
    role: String,
    content: serde_json::Value,
}

fn normalize_model_name(model: &str) -> String {
    let lower = model.to_lowercase();
    if lower.contains('/') {
        model.to_string()
    } else if lower.starts_with("gpt-") || lower.starts_with("o1") || lower.starts_with("o3-") {
        format!("openai/{}", model)
    } else if lower.starts_with("claude-") {
        format!("anthropic/{}", model)
    } else if lower.starts_with("deepseek-") {
        format!("deepseek/{}", model)
    } else {
        model.to_string()
    }
}

fn determine_routed_model(
    config: &crate::config::schema::Config,
    request_model: &str,
    prompt: &str,
) -> String {
    let prompt_lower = prompt.to_lowercase();
    let is_complex = prompt_lower.contains("fix")
        || prompt_lower.contains("bug")
        || prompt_lower.contains("error")
        || prompt_lower.contains("implement")
        || prompt_lower.contains("refactor")
        || prompt_lower.contains("design")
        || prompt_lower.contains("build")
        || prompt_lower.contains("create")
        || prompt_lower.contains("write")
        || prompt_lower.contains("code")
        || prompt_lower.contains("architect")
        || prompt_lower.contains("schema")
        || prompt_lower.contains("test")
        || prompt.len() > 300;

    if is_complex {
        if request_model.contains('/')
            || request_model.starts_with("gpt-")
            || request_model.starts_with("claude-")
        {
            request_model.to_string()
        } else {
            config.agents.defaults.model.clone()
        }
    } else {
        let has_key = |prov: &str| {
            let (api_key, _) = config.resolve_provider_config(prov);
            !api_key.trim().is_empty()
        };

        if has_key("deepseek") {
            "deepseek/deepseek-chat".to_string()
        } else if has_key("groq") {
            "groq/llama-3.3-70b-specdec".to_string()
        } else if has_key("openai") {
            "openai/gpt-4o-mini".to_string()
        } else if has_key("openrouter") {
            "openrouter/google/gemini-2.5-flash-lite".to_string()
        } else {
            request_model.to_string()
        }
    }
}

async fn openai_chat_completions(
    State(state): State<WsState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<OpenAiChatCompletionRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers, None) {
        let err_json = serde_json::json!({
            "error": {
                "message": "Unauthorized: Invalid or missing gateway token.",
                "type": "auth_error",
                "param": null,
                "code": "unauthorized"
            }
        });
        return (StatusCode::UNAUTHORIZED, Json(err_json)).into_response();
    }
    let last_user_content = payload
        .messages
        .iter()
        .rfind(|m| m.role == "user")
        .map(|m| {
            if let Some(s) = m.content.as_str() {
                s.to_string()
            } else if let Some(arr) = m.content.as_array() {
                let mut text = String::new();
                for item in arr {
                    if let Some(txt) = item.get("text").and_then(|v| v.as_str()) {
                        text.push_str(txt);
                    }
                }
                text
            } else {
                m.content.to_string()
            }
        })
        .unwrap_or_default();

    let mut config = state.current_config();
    let req_model = normalize_model_name(&payload.model);
    let routed_model = determine_routed_model(&config, &req_model, &last_user_content);

    config.agents.defaults.model = routed_model.clone();

    let agent_loop = match crate::cli::build_agent_loop(config).await {
        Ok(al) => al,
        Err(e) => {
            let err_json = serde_json::json!({
                "error": {
                    "message": format!("Failed to build agent loop: {}", e),
                    "type": "api_error",
                    "param": null,
                    "code": null
                }
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err_json)).into_response();
        }
    };

    let session_key = payload
        .user
        .unwrap_or_else(|| "openai_proxy_default".to_string());

    let workspace = crate::config::loader::workspace_for_agent_turn(&agent_loop.config);
    let run_result = crate::config::loader::ACTIVE_WORKSPACE
        .scope(workspace, async {
            agent_loop.run(&last_user_content, &session_key).await
        })
        .await;

    match run_result {
        Ok(res) => {
            let created = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let response = serde_json::json!({
                "id": format!("chatcmpl-{}", &uuid::Uuid::new_v4().to_string()[..8]),
                "object": "chat.completion",
                "created": created,
                "model": routed_model,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": res.content,
                    },
                    "finish_reason": "stop"
                }],
                "choices_count": 1,
                "usage": {
                    "prompt_tokens": 0,
                    "completion_tokens": 0,
                    "total_tokens": 0
                }
            });
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let err_json = serde_json::json!({
                "error": {
                    "message": e.to_string(),
                    "type": "api_error",
                    "param": null,
                    "code": null
                }
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err_json)).into_response()
        }
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
