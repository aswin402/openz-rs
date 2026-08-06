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
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

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
}

/// Broadcast a JSON event to every connected WebSocket client.
pub fn publish_ws_event(event: serde_json::Value) {
    if let Ok(senders) = crate::channels::get_active_ws_senders().lock() {
        if let Ok(evt_str) = serde_json::to_string(&event) {
            for sender in senders.values() {
                let _ = sender.try_send(Message::Text(evt_str.clone()));
            }
        }
    }
}

/// Extract the chat id from a session key (mapping colons to underscores to match WebUI format).
pub fn ws_chat_id(session_key: &str) -> Option<String> {
    if session_key.is_empty() {
        return None;
    }
    Some(session_key.replace(':', "_"))
}

static WS_APPROVALS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
> = std::sync::OnceLock::new();

/// Register a pending security-approval request for a WebSocket client.
pub fn register_ws_approval(req_id: String, tx: tokio::sync::oneshot::Sender<bool>) {
    let map = WS_APPROVALS.get_or_init(std::sync::Mutex::default);
    if let Ok(mut approvals) = map.lock() {
        approvals.insert(req_id, tx);
    }
}

/// Resolve a pending security-approval request from a client response.
pub fn resolve_ws_approval(req_id: &str, approved: bool) {
    let map = WS_APPROVALS.get_or_init(std::sync::Mutex::default);
    if let Ok(mut approvals) = map.lock() {
        if let Some(tx) = approvals.remove(req_id) {
            let _ = tx.send(approved);
        }
    }
}

impl WsGateway {
    pub fn new(config: WebSocketChannelConfig, agent_loop: AgentLoop) -> Self {
        WsGateway {
            config,
            agent_loop: Arc::new(agent_loop),
        }
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

        let state = WsState {
            config: self.config.clone(),
            live_config: Arc::new(std::sync::RwLock::new(self.agent_loop.config.clone())),
            agent_loop: self.agent_loop.clone(),
        };
        // Restrict CORS to localhost origins only for security
        let cors = CorsLayer::new()
            .allow_origin([
                "http://localhost".parse().unwrap(),
                "http://127.0.0.1".parse().unwrap(),
                "http://localhost:3000".parse().unwrap(),
                "http://127.0.0.1:3000".parse().unwrap(),
                "http://localhost:8765".parse().unwrap(),
                "http://127.0.0.1:8765".parse().unwrap(),
            ])
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
            .layer(cors)
            .with_state(state);

        let silent = std::env::var("OPENZ_SILENT").is_ok();
        if let Some(dist_path) = find_web_dist() {
            if !silent {
                println!("🌐 Serving WebUI static files from {:?}", dist_path);
            }
            let index_file = dist_path.join("index.html");
            let serve_dir = ServeDir::new(&dist_path)
                .fallback(tower_http::services::ServeFile::new(index_file));
            app = app.fallback_service(serve_dir);
        } else {
            if !silent {
                println!(
                    "⚠️ WebUI static directory not found. Serving WebSocket API only at ws://{}/ws",
                    addr_str
                );
            }
        }

        if !silent {
            println!("⚡ OpenZ Gateway running on http://{}", addr);
            if std::env::var("OPENZ_GATEWAY_TOKEN")
                .map(|t| t.is_empty())
                .unwrap_or(true)
            {
                println!("ℹ️  OPENZ_GATEWAY_TOKEN is not set. Gateway is open for local access.");
                println!("   Set OPENZ_GATEWAY_TOKEN to require authentication for remote clients.");
            }
        }
        let mut shutdown_rx = match crate::shutdown::receiver() {
            Some(rx) => rx,
            None => {
                let (_, rx) = tokio::sync::watch::channel(false);
                rx
            }
        };

        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                if *shutdown_rx.borrow() {
                    return;
                }
                let _ = shutdown_rx.changed().await;
            })
            .await?;

        Ok(())
    }
}

const MAX_WS_MESSAGE_SIZE: usize = 16 * 1024 * 1024; // 16 MB
#[allow(dead_code)]
const MAX_ATTACHMENT_BYTES: usize = 15 * 1024 * 1024; // 15 MB each

/// Persist base64 attachment payloads (sent by the WebUI) to
/// `<config_dir>/attachments/` and return a list of markdown reference lines
/// that get prepended to the outgoing message content before it reaches the
/// agent loop. Image mime types become `![](file://…)` links so the provider
/// layer (via `parse_multimodal_content`) turns them into vision image parts;
/// everything else becomes a `📎 [name](file://…)` link the agent can read
/// with its file/document tools.
#[allow(dead_code)]
async fn persist_attachments(attachments: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    let Some(arr) = attachments.as_array() else {
        return refs;
    };
    if arr.is_empty() {
        return refs;
    }
    let attach_dir = crate::config::loader::config_dir().join("attachments");
    if tokio::fs::create_dir_all(&attach_dir).await.is_err() {
        return refs;
    }
    use base64::{engine::general_purpose, Engine as _};
    for att in arr.iter().take(8) {
        let Some(data_b64) = att.get("data").and_then(|v| v.as_str()) else {
            continue;
        };
        let Ok(bytes) = general_purpose::STANDARD.decode(data_b64) else {
            continue;
        };
        if bytes.is_empty() || bytes.len() > MAX_ATTACHMENT_BYTES {
            continue;
        }
        let mime = att
            .get("mime")
            .and_then(|v| v.as_str())
            .unwrap_or("application/octet-stream")
            .to_string();
        let raw_name = att.get("name").and_then(|v| v.as_str()).unwrap_or("attachment");
        // Sanitize the filename: keep safe characters, strip path separators.
        let clean_name: String = raw_name
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' '))
            .take(64)
            .collect::<String>()
            .trim()
            .to_string();
        let clean_name = if clean_name.is_empty() {
            "attachment".to_string()
        } else {
            clean_name
        };
        let file_name = format!("{}_{}", &uuid::Uuid::new_v4().to_string()[..8], clean_name);
        let path = attach_dir.join(&file_name);
        if tokio::fs::write(&path, &bytes).await.is_err() {
            continue;
        }
        let link = format!("file://{}", path.to_string_lossy());
        if mime.starts_with("image/") {
            refs.push(format!("![]({link})"));
        } else {
            refs.push(format!("📎 [{}]({link})", clean_name));
        }
    }
    refs
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(state): State<WsState>,
) -> impl IntoResponse {
    let query_token = params.get("token").map(|s| s.as_str());
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

    struct WsSenderGuard(String);
    impl Drop for WsSenderGuard {
        fn drop(&mut self) {
            if let Ok(mut senders) = crate::channels::get_active_ws_senders().lock() {
                senders.remove(&self.0);
            }
        }
    }
    let _guard = WsSenderGuard(client_id.clone());

    // Send ready event
    let ready_evt = serde_json::json!({
        "event": "ready",
        "chat_id": default_chat_id,
        "client_id": client_id
    });
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
                let chat_id = envelope
                    .get("chat_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&default_chat_id)
                    .to_string();

                match msg_type {
                    "new_chat" => {
                        let new_id = uuid::Uuid::new_v4().to_string();
                        let attached_evt = serde_json::json!({
                            "event": "attached",
                            "chat_id": new_id
                        });
                        if let Ok(evt_str) = serde_json::to_string(&attached_evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "attach" => {
                        let attached_evt = serde_json::json!({
                            "event": "attached",
                            "chat_id": chat_id
                        });
                        if let Ok(evt_str) = serde_json::to_string(&attached_evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                        let history_evt = fetch_real_session_history(&state.agent_loop.session_manager, &chat_id).await;
                        if let Ok(evt_str) = serde_json::to_string(&history_evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "list_sessions" => {
                        let evt = fetch_real_sessions_list(&state.agent_loop.session_manager).await;
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "load_history" => {
                        let evt = fetch_real_session_history(&state.agent_loop.session_manager, &chat_id).await;
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "get_cognitive_memory" => {
                        let evt = fetch_real_cognitive_memory().await;
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "get_mcp_servers" => {
                        let live = match state.live_config.read() {
                            Ok(g) => g.clone(),
                            Err(_) => state.agent_loop.config.clone(),
                        };
                        let evt = fetch_real_mcp_servers(&live).await;
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "get_logs" => {
                        let evt = fetch_real_logs().await;
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "message" => {
                        let content = envelope
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        if crate::channels::is_stop_command(content) {
                            crate::shutdown::trigger_cli_cancel();
                            let stopped_evt = serde_json::json!({
                                "event": "stopped",
                                "chat_id": chat_id,
                                "detail": "Stop requested. Active OpenZ turn interrupted."
                            });
                            if let Ok(evt_str) = serde_json::to_string(&stopped_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
                            continue;
                        }

                        if let Some(response_text) = crate::channels::session_command_text_response(
                            &state.agent_loop.session_manager,
                            &chat_id,
                            content,
                        )
                        .await
                        {
                            let delta_evt = serde_json::json!({
                                "event": "delta",
                                "chat_id": chat_id,
                                "content": response_text
                            });
                            if let Ok(evt_str) = serde_json::to_string(&delta_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
                            let turn_end_evt = serde_json::json!({
                                "event": "turn_end",
                                "chat_id": chat_id
                            });
                            if let Ok(evt_str) = serde_json::to_string(&turn_end_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
                            continue;
                        }

                        if let Some(response_text) =
                            crate::channels::model_switch_text_response(content)
                        {
                            let delta_evt = serde_json::json!({
                                "event": "delta",
                                "chat_id": chat_id,
                                "content": response_text
                            });
                            if let Ok(evt_str) = serde_json::to_string(&delta_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
                            let turn_end_evt = serde_json::json!({
                                "event": "turn_end",
                                "chat_id": chat_id
                            });
                            if let Ok(evt_str) = serde_json::to_string(&turn_end_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
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
                            let delta_evt = serde_json::json!({
                                "event": "delta",
                                "chat_id": chat_id,
                                "content": response
                            });
                            if let Ok(evt_str) = serde_json::to_string(&delta_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
                            let turn_end_evt = serde_json::json!({
                                "event": "turn_end",
                                "chat_id": chat_id
                            });
                            if let Ok(evt_str) = serde_json::to_string(&turn_end_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
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
                            let delta_evt = serde_json::json!({
                                "event": "delta",
                                "chat_id": chat_id,
                                "content": response
                            });
                            if let Ok(evt_str) = serde_json::to_string(&delta_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
                            let turn_end_evt = serde_json::json!({
                                "event": "turn_end",
                                "chat_id": chat_id
                            });
                            if let Ok(evt_str) = serde_json::to_string(&turn_end_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
                            continue;
                        }
                        if let Some(_stripped) = content.trim().strip_prefix("/device") {
                            let response = "Device clipboard and app suggestions are currently managed locally. To audit device details, use the CLI `openz agent`.".to_string();
                            let delta_evt = serde_json::json!({
                                "event": "delta",
                                "chat_id": chat_id,
                                "content": response
                            });
                            if let Ok(evt_str) = serde_json::to_string(&delta_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
                            let turn_end_evt = serde_json::json!({
                                "event": "turn_end",
                                "chat_id": chat_id
                            });
                            if let Ok(evt_str) = serde_json::to_string(&turn_end_evt) {
                                let _ = tx.send(Message::Text(evt_str)).await;
                            }
                            continue;
                        }
                        let state_clone = state.clone();
                        let tx_clone = tx.clone();
                        let chat_id_clone = chat_id.clone();
                        let content_str = content.to_string();
                        let sem_clone = semaphore.clone();

                        tokio::spawn(async move {
                            let _permit = match sem_clone.try_acquire() {
                                Ok(p) => p,
                                Err(_) => {
                                    let err_evt = serde_json::json!({
                                        "event": "error",
                                        "chat_id": chat_id_clone,
                                        "detail": "Rate limit exceeded: Only one message can be processed at a time."
                                    });
                                    if let Ok(evt_str) = serde_json::to_string(&err_evt) {
                                        let _ = tx_clone.send(Message::Text(evt_str)).await;
                                    }
                                    return;
                                }
                            };

                            let config = match state_clone.live_config.read() {
                                Ok(g) => g.clone(),
                                Err(_) => state_clone.agent_loop.config.clone(),
                            };
                            let agent_loop = match crate::cli::build_agent_loop(config).await {
                                Ok(al) => al,
                                Err(e) => {
                                    let err_evt = serde_json::json!({
                                        "event": "error",
                                        "chat_id": chat_id_clone,
                                        "detail": format!("Failed to build agent loop: {}", e)
                                    });
                                    if let Ok(evt_str) = serde_json::to_string(&err_evt) {
                                        let _ = tx_clone.send(Message::Text(evt_str)).await;
                                    }
                                    return;
                                }
                            };

                            let session_key = resolve_session_key(&agent_loop.session_manager, &chat_id_clone);

                            match agent_loop
                                .run(&content_str, &session_key)
                                .await
                            {
                                Ok(res) => {
                                    // Streaming deltas are emitted live from the agent loop
                                    // (event "delta"); only send a full-content delta when
                                    // the turn did not stream.
                                    if !res.streamed {
                                        let delta_evt = serde_json::json!({
                                            "event": "delta",
                                            "chat_id": chat_id_clone,
                                            "content": res.content
                                        });
                                        if let Ok(evt_str) = serde_json::to_string(&delta_evt) {
                                            let _ = tx_clone.send(Message::Text(evt_str)).await;
                                        }
                                    }

                                    let turn_end_evt = serde_json::json!({
                                        "event": "turn_end",
                                        "chat_id": chat_id_clone
                                    });
                                    if let Ok(evt_str) = serde_json::to_string(&turn_end_evt) {
                                        let _ = tx_clone.send(Message::Text(evt_str)).await;
                                    }
                                }
                                Err(e) => {
                                    let err_evt = serde_json::json!({
                                        "event": "error",
                                        "chat_id": chat_id_clone,
                                        "detail": e.to_string()
                                    });
                                    if let Ok(evt_str) = serde_json::to_string(&err_evt) {
                                        let _ = tx_clone.send(Message::Text(evt_str)).await;
                                    }
                                }
                            }
                        });
                    }
                    "security_response" => {
                        let req_id = envelope
                            .get("req_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let approved = envelope
                            .get("approved")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        crate::channels::websocket::resolve_ws_approval(&req_id, approved);
                    }
                    "get_models" => {
                        let config = match state.live_config.read() {
                            Ok(g) => g.clone(),
                            Err(_) => state.agent_loop.config.clone(),
                        };
                        let mut providers = Vec::new();
                        for opt in crate::channels::configured_provider_model_options(&config) {
                            providers.push(serde_json::json!({
                                "name": opt.name,
                                "display": opt.display,
                                "models": opt.models,
                            }));
                        }
                        let active_provider = config.agents.defaults.provider.clone();
                        let active_model = config.agents.defaults.model.clone();
                        let evt = serde_json::json!({
                            "event": "models_list",
                            "providers": providers,
                            "active_provider": active_provider,
                            "active_model": active_model,
                        });
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "get_config" => {
                        let config = match state.live_config.read() {
                            Ok(g) => g.clone(),
                            Err(_) => state.agent_loop.config.clone(),
                        };
                        let skills = crate::agent::skills::load_skills()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|s| serde_json::json!({ "name": s.name, "content": s.content }))
                            .collect::<Vec<_>>();
                        let d = &config.agents.defaults;
                        let defaults = serde_json::json!({
                            "model": d.model,
                            "provider": d.provider,
                            "temperature": d.temperature,
                            "max_tokens": d.max_tokens,
                            "streaming": d.streaming,
                            "caveman_mode": d.caveman_mode,
                            "security_mode": d.security_mode,
                            "workspace": d.workspace,
                            "bot_name": d.bot_name,
                            "max_messages": d.max_messages,
                            "max_tool_iterations": d.max_tool_iterations,
                            "tool_timeout_secs": d.tool_timeout_secs,
                            "enable_sandbox": d.enable_sandbox,
                            "context_limit": d.context_limit,
                            "tool_output_limit": d.tool_output_limit,
                            "tui_thought_display": d.tui_thought_display,
                        });
                        let mcp_resp = fetch_real_mcp_servers(&config).await;
                        let mcp_servers = mcp_resp["servers"].clone();
                        let subagents = crate::subagents::load_profiles()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|p| {
                                let model_str = p.model.clone().unwrap_or_else(|| "auto".to_string());
                                let parts: Vec<&str> = model_str.split('/').collect();
                                let (provider, model) = if parts.len() > 1 {
                                    (parts[0].to_string(), parts[1..].join("/"))
                                } else {
                                    ("auto".to_string(), model_str)
                                };
                                serde_json::json!({
                                    "name": p.name,
                                    "description": p.description,
                                    "systemPrompt": p.system_prompt,
                                    "model": model,
                                    "provider": provider,
                                })
                            })
                            .collect::<Vec<_>>();
                        // Expose providers with masked api_keys
                        let mut providers_config = serde_json::Map::new();
                        let p = &config.providers;
                        
                        let mask_key = |key: &Option<String>| {
                            key.as_ref().map(|k| if k.is_empty() { "" } else { "••••••••" })
                        };
                        
                        let map_provider = |c: &Option<crate::config::schema::ProviderConfig>| {
                            c.as_ref().map(|cfg| serde_json::json!({
                                "api_key": mask_key(&cfg.api_key),
                                "api_base": cfg.api_base,
                                "default_model": cfg.default_model
                            }))
                        };

                        providers_config.insert("openai".to_string(), serde_json::to_value(map_provider(&p.openai)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("anthropic".to_string(), serde_json::to_value(map_provider(&p.anthropic)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("openrouter".to_string(), serde_json::to_value(map_provider(&p.openrouter)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("deepseek".to_string(), serde_json::to_value(map_provider(&p.deepseek)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("groq".to_string(), serde_json::to_value(map_provider(&p.groq)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("ollama".to_string(), serde_json::to_value(map_provider(&p.ollama)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("minimax".to_string(), serde_json::to_value(map_provider(&p.minimax)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("mistral".to_string(), serde_json::to_value(map_provider(&p.mistral)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("z_ai".to_string(), serde_json::to_value(map_provider(&p.z_ai)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("nvidia".to_string(), serde_json::to_value(map_provider(&p.nvidia)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("opencode_zen".to_string(), serde_json::to_value(map_provider(&p.opencode_zen)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("cerebras".to_string(), serde_json::to_value(map_provider(&p.cerebras)).unwrap_or(serde_json::Value::Null));
                        providers_config.insert("google_ai_studio".to_string(), serde_json::to_value(map_provider(&p.google_ai_studio)).unwrap_or(serde_json::Value::Null));

                        for (key, cfg) in &p.others {
                            providers_config.insert(
                                key.clone(),
                                serde_json::json!({
                                    "api_key": mask_key(&cfg.api_key),
                                    "api_base": cfg.api_base,
                                    "default_model": cfg.default_model
                                })
                            );
                        }

                        // Expose channel configurations
                        let mut channels_config = serde_json::Map::new();
                        let ch = &config.channels;
                        
                        let map_tg = |c: &Option<crate::config::schema::TelegramChannelConfig>| {
                            c.as_ref().map(|cfg| serde_json::json!({
                                "enabled": cfg.enabled,
                                "bot_token": if cfg.bot_token.is_empty() { "" } else { "••••••••" }
                            }))
                        };
                        let map_dc = |c: &Option<crate::config::schema::DiscordChannelConfig>| {
                            c.as_ref().map(|cfg| serde_json::json!({
                                "enabled": cfg.enabled,
                                "bot_token": if cfg.bot_token.is_empty() { "" } else { "••••••••" }
                            }))
                        };
                        let map_wa = |c: &Option<crate::config::schema::WhatsAppChannelConfig>| {
                            c.as_ref().map(|cfg| serde_json::json!({
                                "enabled": cfg.enabled,
                                "api_key": if cfg.api_key.is_empty() { "" } else { "••••••••" },
                                "phone_number_id": cfg.phone_number_id,
                                "webhook_port": cfg.webhook_port,
                                "verify_token": cfg.verify_token
                            }))
                        };

                        channels_config.insert("telegram".to_string(), serde_json::to_value(map_tg(&ch.telegram)).unwrap_or(serde_json::Value::Null));
                        channels_config.insert("discord".to_string(), serde_json::to_value(map_dc(&ch.discord)).unwrap_or(serde_json::Value::Null));
                        channels_config.insert("whatsapp".to_string(), serde_json::to_value(map_wa(&ch.whatsapp)).unwrap_or(serde_json::Value::Null));

                        let evt = serde_json::json!({
                            "event": "config_data",
                            "defaults": defaults,
                            "skills": skills,
                            "mcp_servers": mcp_servers,
                            "subagents": subagents,
                            "providers": providers_config,
                            "channels": channels_config,
                            "version": env!("CARGO_PKG_VERSION"),
                        });
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "set_config" => {
                        let mut config = match state.live_config.read() {
                            Ok(guard) => guard.clone(),
                            Err(_) => state.agent_loop.config.clone(),
                        };
                        if let Some(defaults) = envelope.get("defaults") {
                            let d = &mut config.agents.defaults;
                            if let Some(v) = defaults.get("model").and_then(|v| v.as_str()) {
                                d.model = v.to_string();
                            }
                            if let Some(v) = defaults.get("provider").and_then(|v| v.as_str()) {
                                d.provider = v.to_string();
                            }
                            if let Some(v) = defaults.get("temperature").and_then(|v| v.as_f64()) {
                                d.temperature = v as f32;
                            }
                            if let Some(v) = defaults.get("max_tokens").and_then(|v| v.as_u64()) {
                                d.max_tokens = v as usize;
                            }
                            if let Some(v) = defaults.get("streaming").and_then(|v| v.as_bool()) {
                                d.streaming = v;
                            }
                            if let Some(v) = defaults.get("caveman_mode").and_then(|v| v.as_bool()) {
                                d.caveman_mode = v;
                            }
                            if let Some(v) = defaults.get("security_mode").and_then(|v| v.as_str()) {
                                d.security_mode = v.to_string();
                            }
                            if let Some(v) = defaults.get("bot_name").and_then(|v| v.as_str()) {
                                d.bot_name = v.to_string();
                            }
                            if let Some(v) = defaults.get("max_messages").and_then(|v| v.as_u64()) {
                                d.max_messages = v as usize;
                            }
                            if let Some(v) = defaults.get("max_tool_iterations").and_then(|v| v.as_u64()) {
                                d.max_tool_iterations = v as usize;
                            }
                            if let Some(v) = defaults.get("tool_timeout_secs").and_then(|v| v.as_u64()) {
                                d.tool_timeout_secs = v as u64;
                            }
                        }

                        // 2. Providers updates
                        if let Some(providers_val) = envelope.get("providers") {
                            if let Some(obj) = providers_val.as_object() {
                                let p = &mut config.providers;
                                let mut update_provider = |field: &mut Option<crate::config::schema::ProviderConfig>, data: &serde_json::Value| {
                                    if let Some(data_obj) = data.as_object() {
                                        let mut cfg = field.clone().unwrap_or_default();
                                        if let Some(key) = data_obj.get("api_key").and_then(|v| v.as_str()) {
                                            if key != "••••••••" {
                                                cfg.api_key = if key.is_empty() { None } else { Some(key.to_string()) };
                                            }
                                        }
                                        if let Some(base) = data_obj.get("api_base") {
                                            cfg.api_base = base.as_str().map(|s| s.to_string());
                                        }
                                        if let Some(m) = data_obj.get("default_model") {
                                            cfg.default_model = m.as_str().map(|s| s.to_string());
                                        }
                                        *field = Some(cfg);
                                    }
                                };

                                if let Some(val) = obj.get("openai") { update_provider(&mut p.openai, val); }
                                if let Some(val) = obj.get("anthropic") { update_provider(&mut p.anthropic, val); }
                                if let Some(val) = obj.get("openrouter") { update_provider(&mut p.openrouter, val); }
                                if let Some(val) = obj.get("deepseek") { update_provider(&mut p.deepseek, val); }
                                if let Some(val) = obj.get("groq") { update_provider(&mut p.groq, val); }
                                if let Some(val) = obj.get("ollama") { update_provider(&mut p.ollama, val); }
                                if let Some(val) = obj.get("minimax") { update_provider(&mut p.minimax, val); }
                                if let Some(val) = obj.get("mistral") { update_provider(&mut p.mistral, val); }
                                if let Some(val) = obj.get("z_ai") { update_provider(&mut p.z_ai, val); }
                                if let Some(val) = obj.get("nvidia") { update_provider(&mut p.nvidia, val); }
                                if let Some(val) = obj.get("opencode_zen") { update_provider(&mut p.opencode_zen, val); }
                                if let Some(val) = obj.get("cerebras") { update_provider(&mut p.cerebras, val); }
                                if let Some(val) = obj.get("google_ai_studio") { update_provider(&mut p.google_ai_studio, val); }

                                for (key, val) in obj {
                                    let is_builtin = matches!(
                                        key.as_str(),
                                        "openai" | "anthropic" | "openrouter" | "deepseek" | "groq" | "ollama" |
                                        "minimax" | "mistral" | "z_ai" | "nvidia" | "opencode_zen" | "cerebras" |
                                        "google_ai_studio"
                                    );
                                    if !is_builtin {
                                        if let Some(data_obj) = val.as_object() {
                                            let mut cfg = p.others.get(key).cloned().unwrap_or_default();
                                            if let Some(k) = data_obj.get("api_key").and_then(|v| v.as_str()) {
                                                if k != "••••••••" {
                                                    cfg.api_key = if k.is_empty() { None } else { Some(k.to_string()) };
                                                }
                                            }
                                            if let Some(base) = data_obj.get("api_base") {
                                                cfg.api_base = base.as_str().map(|s| s.to_string());
                                            }
                                            if let Some(m) = data_obj.get("default_model") {
                                                cfg.default_model = m.as_str().map(|s| s.to_string());
                                            }
                                            p.others.insert(key.clone(), cfg);
                                        }
                                    }
                                }
                            }
                        }

                        // 3. Channels updates
                        if let Some(channels_val) = envelope.get("channels") {
                            if let Some(obj) = channels_val.as_object() {
                                let ch = &mut config.channels;
                                
                                if let Some(val) = obj.get("telegram") {
                                    if let Some(data_obj) = val.as_object() {
                                        let mut cfg = ch.telegram.clone().unwrap_or_default();
                                        if let Some(enabled) = data_obj.get("enabled").and_then(|v| v.as_bool()) {
                                            cfg.enabled = enabled;
                                        }
                                        if let Some(token) = data_obj.get("bot_token").and_then(|v| v.as_str()) {
                                            if token != "••••••••" {
                                                cfg.bot_token = token.to_string();
                                            }
                                        }
                                        ch.telegram = Some(cfg);
                                    }
                                }

                                if let Some(val) = obj.get("discord") {
                                    if let Some(data_obj) = val.as_object() {
                                        let mut cfg = ch.discord.clone().unwrap_or_default();
                                        if let Some(enabled) = data_obj.get("enabled").and_then(|v| v.as_bool()) {
                                            cfg.enabled = enabled;
                                        }
                                        if let Some(token) = data_obj.get("bot_token").and_then(|v| v.as_str()) {
                                            if token != "••••••••" {
                                                cfg.bot_token = token.to_string();
                                            }
                                        }
                                        ch.discord = Some(cfg);
                                    }
                                }

                                if let Some(val) = obj.get("whatsapp") {
                                    if let Some(data_obj) = val.as_object() {
                                        let mut cfg = ch.whatsapp.clone().unwrap_or_default();
                                        if let Some(enabled) = data_obj.get("enabled").and_then(|v| v.as_bool()) {
                                            cfg.enabled = enabled;
                                        }
                                        if let Some(key) = data_obj.get("api_key").and_then(|v| v.as_str()) {
                                            if key != "••••••••" {
                                                cfg.api_key = key.to_string();
                                            }
                                        }
                                        if let Some(num_id) = data_obj.get("phone_number_id").and_then(|v| v.as_str()) {
                                            cfg.phone_number_id = num_id.to_string();
                                        }
                                        if let Some(port) = data_obj.get("webhook_port").and_then(|v| v.as_u64()) {
                                            cfg.webhook_port = port as u16;
                                        }
                                        if let Some(verify) = data_obj.get("verify_token").and_then(|v| v.as_str()) {
                                            cfg.verify_token = verify.to_string();
                                        }
                                        ch.whatsapp = Some(cfg);
                                    }
                                }
                            }
                        }

                        let _ = crate::config::loader::save_config(&config);
                        if let Ok(mut live) = state.live_config.write() {
                            *live = config.clone();
                        }
                        let d = &config.agents.defaults;
                        let evt = serde_json::json!({
                            "event": "config_updated",
                            "defaults": {
                                "model": d.model,
                                "provider": d.provider,
                                "temperature": d.temperature,
                                "max_tokens": d.max_tokens,
                                "streaming": d.streaming,
                                "caveman_mode": d.caveman_mode,
                                "security_mode": d.security_mode,
                                "workspace": d.workspace,
                                "bot_name": d.bot_name,
                                "max_messages": d.max_messages,
                                "max_tool_iterations": d.max_tool_iterations,
                                "tool_timeout_secs": d.tool_timeout_secs,
                                "enable_sandbox": d.enable_sandbox,
                            }
                        });
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "get_slash_commands" => {
                        let commands = crate::channels::cli::render::SLASH_COMMANDS
                            .iter()
                            .map(|(cmd, desc)| serde_json::json!({ "cmd": cmd, "desc": desc }))
                            .collect::<Vec<_>>();
                        let evt = serde_json::json!({
                            "event": "slash_commands",
                            "commands": commands,
                        });
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "get_status" => {
                        let (loaded, failed, total) = crate::channels::cli::mcp::get_mcp_stats();
                        let evt = serde_json::json!({
                            "event": "status",
                            "version": env!("CARGO_PKG_VERSION"),
                            "mcp": {
                                "loaded": loaded,
                                "failed": failed,
                                "total": total,
                            },
                        });
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "get_servers" => {
                        let config = match state.live_config.read() {
                            Ok(g) => g.clone(),
                            Err(_) => state.agent_loop.config.clone(),
                        };
                        let evt = fetch_real_servers(&config).await;
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    "stop_server" => {
                        let target = envelope
                            .get("target")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let result = match crate::shutdown::stop_registered_child(target) {
                            Ok(count) => format!("Stopped {count} server(s) successfully."),
                            Err(e) => format!("Failed to stop server: {e}"),
                        };
                        let evt = serde_json::json!({
                            "event": "server_stopped",
                            "target": target,
                            "result": result,
                        });
                        if let Ok(evt_str) = serde_json::to_string(&evt) {
                            let _ = tx.send(Message::Text(evt_str)).await;
                        }
                    }
                    _ => {}
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
    let config = match state.live_config.read() {
        Ok(g) => g.clone(),
        Err(_) => state.agent_loop.config.clone(),
    };
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
    let config = match state.live_config.read() {
        Ok(g) => g.clone(),
        Err(_) => state.agent_loop.config.clone(),
    };
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
        let has_key = |prov: &str| -> bool {
            match prov {
                "deepseek" => {
                    config
                        .providers
                        .deepseek
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("DEEPSEEK_API_KEY").is_ok()
                }
                "groq" => {
                    config
                        .providers
                        .groq
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("GROQ_API_KEY").is_ok()
                }
                "openrouter" => {
                    config
                        .providers
                        .openrouter
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("OPENROUTER_API_KEY").is_ok()
                }
                "openai" => {
                    config
                        .providers
                        .openai
                        .as_ref()
                        .and_then(|p| p.api_key.as_ref())
                        .is_some()
                        || std::env::var("OPENAI_API_KEY").is_ok()
                }
                _ => false,
            }
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

    let mut config = match state.live_config.read() {
        Ok(g) => g.clone(),
        Err(_) => state.agent_loop.config.clone(),
    };
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

    match agent_loop.run(&last_user_content, &session_key).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_chat_uses_shared_stop_command_detection() {
        assert!(crate::channels::is_stop_command("/stop"));
        assert!(!crate::channels::is_stop_command("/stopwatch"));
    }

    #[test]
    fn test_normalize_model_name() {
        assert_eq!(normalize_model_name("gpt-4o"), "openai/gpt-4o");
        assert_eq!(
            normalize_model_name("claude-3-5-sonnet"),
            "anthropic/claude-3-5-sonnet"
        );
        assert_eq!(
            normalize_model_name("deepseek-chat"),
            "deepseek/deepseek-chat"
        );
        assert_eq!(normalize_model_name("custom/my-model"), "custom/my-model");
    }

    #[test]
    fn test_determine_routed_model_complex() {
        let mut config = crate::config::schema::Config::default();
        config.agents.defaults.model = "anthropic/claude-3-5-sonnet".to_string();

        // Complex prompts should use requested or default premium
        let model =
            determine_routed_model(&config, "gpt-4o", "Please fix this error in my rust code");
        assert_eq!(model, "gpt-4o");

        let model_fallback = determine_routed_model(
            &config,
            "some-random-model",
            "Please design a new database schema for a blog",
        );
        assert_eq!(model_fallback, "anthropic/claude-3-5-sonnet");
    }

    #[test]
    fn test_determine_routed_model_simple_fallback() {
        let mut config = crate::config::schema::Config::default();
        config.agents.defaults.model = "anthropic/claude-3-5-sonnet".to_string();

        // Simple prompt with env vars -> routes to cheapest available provider
        let _model = determine_routed_model(&config, "gpt-4o", "Hello!");

        // Simple prompt, deepseek key set -> should route to deepseek-chat
        config.providers.deepseek = Some(crate::config::schema::ProviderConfig {
            api_key: Some("test-key".to_string()),
            api_base: None,
            default_model: None,
            extra: std::collections::HashMap::new(),
        });
        let model_routed = determine_routed_model(&config, "gpt-4o", "Hi there");
        assert_eq!(model_routed, "deepseek/deepseek-chat");
    }

    #[test]
    fn test_is_authorized() {
        use axum::http::HeaderMap;

        // Unset token -> open access (allow all)
        std::env::remove_var("OPENZ_GATEWAY_TOKEN");
        let headers = HeaderMap::new();
        assert!(is_authorized(&headers, None));
        assert!(is_authorized(&headers, Some("test")));

        // Empty token -> open access (allow all)
        std::env::set_var("OPENZ_GATEWAY_TOKEN", "");
        assert!(is_authorized(&headers, None));
        assert!(is_authorized(&headers, Some("")));

        // Set token -> verify query token and header
        std::env::set_var("OPENZ_GATEWAY_TOKEN", "super-secret-token");
        assert!(!is_authorized(&headers, None));
        assert!(!is_authorized(&headers, Some("wrong-token")));
        assert!(is_authorized(&headers, Some("super-secret-token")));

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer super-secret-token"),
        );
        assert!(is_authorized(&headers, None));

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer wrong-token"),
        );
        assert!(!is_authorized(&headers, None));

        // Clean up
        std::env::remove_var("OPENZ_GATEWAY_TOKEN");
    }
}
use super::secure_compare;

fn is_authorized(headers: &axum::http::HeaderMap, query_token: Option<&str>) -> bool {
    let expected = std::env::var("OPENZ_GATEWAY_TOKEN").unwrap_or_default();
    // When no token is configured, allow all connections (open local access).
    if expected.is_empty() {
        return true;
    }
    // Token is configured — enforce it via query param or Authorization header.
    if let Some(tok) = query_token {
        if secure_compare(tok, &expected) {
            return true;
        }
    }
    if let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if secure_compare(token.trim(), &expected) {
                    return true;
                }
            }
        }
    }
    false
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

async fn fetch_real_sessions_list(session_mgr: &crate::session::SessionManager) -> serde_json::Value {
    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&session_mgr.dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let key = stem.to_string();
                    if let Ok(session) = session_mgr.load(&key) {
                        let title = session
                            .messages
                            .iter()
                            .find(|m| m.role == "user")
                            .map(|m| {
                                let c = m.content.trim();
                                if c.len() > 36 {
                                    format!("{}...", &c[..36])
                                } else {
                                    c.to_string()
                                }
                            })
                            .unwrap_or_else(|| key.clone());

                        let msg_count = session.messages.len();
                        let last_msg_at = path
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::now())
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();

                        sessions.push(serde_json::json!({
                            "id": key,
                            "title": title,
                            "createdAt": last_msg_at,
                            "lastMessageAt": last_msg_at,
                            "messageCount": msg_count
                        }));
                    }
                }
            }
        }
    }
    sessions.sort_by(|a, b| {
        let ta = a["lastMessageAt"].as_u64().unwrap_or(0);
        let tb = b["lastMessageAt"].as_u64().unwrap_or(0);
        tb.cmp(&ta)
    });
    serde_json::json!({
        "event": "sessions_list",
        "sessions": sessions
    })
}

fn resolve_session_key(session_mgr: &crate::session::SessionManager, chat_id: &str) -> String {
    if chat_id.contains(':') {
        return chat_id.to_string();
    }
    let safe_key = chat_id.replace(":", "_").replace("/", "_").replace("\\", "_");
    let path = session_mgr.dir.join(format!("{}.json", safe_key));
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(key) = val.get("key").and_then(|k| k.as_str()) {
                    return key.to_string();
                }
            }
        }
        return chat_id.to_string();
    }
    if chat_id.starts_with("cli_") {
        return format!("cli:{}", &chat_id[4..]);
    }
    if chat_id.starts_with("subagent_") {
        return format!("subagent:{}", &chat_id[9..]);
    }
    if chat_id.starts_with("telegram_") {
        return format!("telegram:{}", &chat_id[9..]);
    }
    if chat_id.starts_with("ws_") {
        return format!("ws:{}", &chat_id[3..]);
    }
    format!("ws:{}", chat_id)
}

async fn fetch_real_session_history(
    session_mgr: &crate::session::SessionManager,
    chat_id: &str,
) -> serde_json::Value {
    let mut messages = Vec::new();
    let load_key = resolve_session_key(session_mgr, chat_id);
    if let Ok(session) = session_mgr.load(&load_key) {
        for (idx, msg) in session.messages.iter().enumerate() {
            let ts = msg
                .timestamp
                .as_deref()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|dt| dt.timestamp_millis())
                .unwrap_or_else(|| (idx as i64) * 1000);

            messages.push(serde_json::json!({
                "id": format!("msg-{}-{}", idx, ts),
                "role": msg.role,
                "content": msg.content,
                "timestamp": ts,
                "extra": msg.extra,
            }));
        }
    }
    serde_json::json!({
        "event": "session_history",
        "chat_id": chat_id,
        "messages": messages
    })
}

async fn fetch_real_cognitive_memory() -> serde_json::Value {
    let mut entities_count = 0i64;
    let mut relations_count = 0i64;
    let mut facts_count = 0i64;
    let mut working_memory_keys: Vec<String> = Vec::new();
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut edges: Vec<serde_json::Value> = Vec::new();
    let mut facts: Vec<serde_json::Value> = Vec::new();

    // graph_memory.db uses graph_nodes and graph_edges
    let graph_db = crate::config::loader::runtime_db_path("graph_memory.db");
    if graph_db.exists() {
        if let Ok(conn) = rusqlite::Connection::open(&graph_db) {
            let _ = conn.query_row("SELECT COUNT(*) FROM graph_nodes", [], |r| r.get(0)).map(|c: i64| entities_count = c);
            let _ = conn.query_row("SELECT COUNT(*) FROM graph_edges", [], |r| r.get(0)).map(|c: i64| relations_count = c);

            // Fetch nodes
            if let Ok(mut stmt) = conn.prepare("SELECT name, entity_type, observations FROM graph_nodes LIMIT 100") {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok(serde_json::json!({
                        "name": r.get::<_, String>(0)?,
                        "entity_type": r.get::<_, String>(1)?,
                        "observations": r.get::<_, String>(2)?,
                    }))
                }) {
                    nodes = rows.filter_map(|r| r.ok()).collect();
                }
            }

            // Fetch edges
            if let Ok(mut stmt) = conn.prepare("SELECT from_name, to_name, relation_type FROM graph_edges LIMIT 200") {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok(serde_json::json!({
                        "from_name": r.get::<_, String>(0)?,
                        "to_name": r.get::<_, String>(1)?,
                        "relation_type": r.get::<_, String>(2)?,
                    }))
                }) {
                    edges = rows.filter_map(|r| r.ok()).collect();
                }
            }
        }
    }

    // memory.db uses cognitive_memory table for stored facts/memories
    let memory_db = crate::config::loader::runtime_db_path("memory.db");
    if memory_db.exists() {
        if let Ok(conn) = rusqlite::Connection::open(&memory_db) {
            let _ = conn.query_row("SELECT COUNT(*) FROM cognitive_memory", [], |r| r.get(0)).map(|c: i64| facts_count = c);
            // Fetch working memory keys from interaction_history or skills if available
            if let Ok(mut stmt) = conn.prepare("SELECT name FROM skills LIMIT 10") {
                if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                    working_memory_keys = rows.filter_map(|r| r.ok()).collect();
                }
            }

            // Fetch facts
            if let Ok(mut stmt) = conn.prepare("SELECT text, timestamp, tags, importance FROM cognitive_memory LIMIT 100") {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok(serde_json::json!({
                        "text": r.get::<_, String>(0)?,
                        "timestamp": r.get::<_, String>(1)?,
                        "tags": r.get::<_, String>(2)?,
                        "importance": r.get::<_, f64>(3)?,
                    }))
                }) {
                    facts = rows.filter_map(|r| r.ok()).collect();
                }
            }
        }
    }

    if working_memory_keys.is_empty() {
        working_memory_keys = vec![
            "active_workspace".to_string(),
            "session_scope".to_string(),
            "security_level".to_string(),
            "caveman_mode".to_string(),
        ];
    }

    serde_json::json!({
        "event": "cognitive_memory",
        "stats": {
            "entitiesCount": entities_count,
            "relationsCount": relations_count,
            "factsCount": facts_count,
            "workingMemoryKeys": working_memory_keys
        },
        "nodes": nodes,
        "edges": edges,
        "facts": facts
    })
}

async fn fetch_real_mcp_servers(config: &crate::config::schema::Config) -> serde_json::Value {
    let (loaded, failed, _total) = crate::channels::cli::mcp::get_mcp_stats();
    let mcp_done = crate::channels::cli::mcp::is_mcp_done();
    let mut servers = Vec::new();
    for (name, server_cfg) in &config.mcp_servers {
        let status = if !server_cfg.enabled {
            "disabled"
        } else if !mcp_done {
            "starting"
        } else {
            "connected"
        };
        let tools_count = if server_cfg.enabled {
            crate::tools::mcp::spawned_tools_count(&server_cfg.command, &server_cfg.args).await
        } else {
            0
        };
        servers.push(serde_json::json!({
            "name": name,
            "command": server_cfg.command,
            "status": status,
            "enabled": server_cfg.enabled,
            "args": server_cfg.args,
            "toolsCount": tools_count,
        }));
    }
    serde_json::json!({
        "event": "mcp_servers",
        "servers": servers,
        "stats": {
            "loaded": loaded,
            "failed": failed,
            "total": failed + loaded,
        }
    })
}

async fn fetch_real_logs() -> serde_json::Value {
    let mut log_entries = Vec::new();
    let log_path = crate::logs::default_log_path();
    if let Ok(content) = std::fs::read_to_string(&log_path) {
        let lines: Vec<&str> = content.lines().rev().take(100).collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.trim().is_empty() { continue; }
            let level = if line.contains("ERROR") { "ERROR" } else if line.contains("WARN") { "WARN" } else { "INFO" };
            let ts = if line.len() >= 19 && line.as_bytes()[10] == b'T' {
                line[11..19].to_string()
            } else {
                chrono::Local::now().format("%H:%M:%S").to_string()
            };
            log_entries.push(serde_json::json!({
                "id": format!("log-{}", idx),
                "timestamp": ts,
                "level": level,
                "target": "openz::gateway",
                "message": line.to_string()
            }));
        }
    }
    serde_json::json!({
        "event": "logs_data",
        "logs": log_entries
    })
}

async fn fetch_real_servers(config: &crate::config::schema::Config) -> serde_json::Value {
    let servers = crate::shutdown::list_registered_children();
    let mut list = Vec::new();
    for s in servers {
        list.push(serde_json::json!({
            "id": s.id,
            "pid": s.pid,
            "kind": s.kind,
            "command": s.command,
        }));
    }
    let mut channels = Vec::new();
    let tg = config.channels.telegram.as_ref();
    let dc = config.channels.discord.as_ref();
    let wa = config.channels.whatsapp.as_ref();

    channels.push(serde_json::json!({
        "name": "telegram",
        "enabled": tg.map(|c| c.enabled).unwrap_or(false),
        "status": if tg.map(|c| c.enabled).unwrap_or(false) { "configured" } else { "disabled" },
        "token_configured": tg.map(|c| !c.bot_token.is_empty()).unwrap_or(false),
    }));
    channels.push(serde_json::json!({
        "name": "discord",
        "enabled": dc.map(|c| c.enabled).unwrap_or(false),
        "status": if dc.map(|c| c.enabled).unwrap_or(false) { "configured" } else { "disabled" },
        "token_configured": dc.map(|c| !c.bot_token.is_empty()).unwrap_or(false),
    }));
    channels.push(serde_json::json!({
        "name": "whatsapp",
        "enabled": wa.map(|c| c.enabled).unwrap_or(false),
        "status": if wa.map(|c| c.enabled).unwrap_or(false) { "configured" } else { "disabled" },
        "token_configured": wa.map(|c| !c.api_key.is_empty()).unwrap_or(false),
    }));
    serde_json::json!({
        "event": "servers_list",
        "servers": list,
        "channels": channels
    })
}
