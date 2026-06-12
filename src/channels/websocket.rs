use crate::agent::AgentLoop;
use crate::config::schema::WebSocketChannelConfig;
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State, Path as AxumPath},
    response::IntoResponse,
    routing::get,
    Router, Json,
    http::StatusCode,
};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use serde_json::Value;
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
            agent_loop: self.agent_loop.clone(),
        };
        let mut app = Router::new()
            .route("/ws", get(ws_handler))
            .route("/webhook/sop/trigger/:sop_id", axum::routing::post(trigger_sop_handler))
            .route("/webhook/sop/instances/:instance_id/resume", axum::routing::post(resume_sop_handler))
            .with_state(state);

        let silent = std::env::var("OPENZ_SILENT").is_ok();
        let dist_path = Path::new("./nanobot/nanobot/web/dist");
        if dist_path.exists() {
            if !silent {
                println!("🌐 Serving WebUI static files from {:?}", dist_path);
            }
            app = app.fallback_service(ServeDir::new(dist_path));
        } else {
            let alt_path = Path::new("./web/dist");
            if alt_path.exists() {
                if !silent {
                    println!("🌐 Serving WebUI static files from {:?}", alt_path);
                }
                app = app.fallback_service(ServeDir::new(alt_path));
            } else {
                if !silent {
                    println!("⚠️ WebUI static directory not found. Serving WebSocket API only at ws://{}/ws", addr_str);
                }
            }
        }

        if !silent {
            println!("⚡ OpenZ Gateway running on http://{}", addr);
        }
        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<WsState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: WsState) {
    let client_id = format!("client-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let default_chat_id = uuid::Uuid::new_v4().to_string();

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
            let parsed: Result<Value, _> = serde_json::from_str(&text);
            if let Ok(envelope) = parsed {
                let msg_type = envelope.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let chat_id = envelope.get("chat_id").and_then(|v| v.as_str()).unwrap_or(&default_chat_id).to_string();
                
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
                    }
                    "message" => {
                        let content = envelope.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        
                        let agent = state.agent_loop.clone();
                        let tx_clone = tx.clone();
                        let chat_id_clone = chat_id.clone();
                        let content_str = content.to_string();
                        
                        tokio::spawn(async move {
                            match agent.run(&content_str, &format!("ws:{}", chat_id_clone)).await {
                                Ok(res) => {
                                    let delta_evt = serde_json::json!({
                                        "event": "delta",
                                        "chat_id": chat_id_clone,
                                        "content": res.content
                                    });
                                    if let Ok(evt_str) = serde_json::to_string(&delta_evt) {
                                        let _ = tx_clone.send(Message::Text(evt_str)).await;
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
                    _ => {}
                }
            }
        }
    }
}

async fn trigger_sop_handler(
    State(state): State<WsState>,
    AxumPath(sop_id): AxumPath<String>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let config = state.agent_loop.config.clone();
    match crate::sop::engine::trigger_sop(config, sop_id, payload).await {
        Ok(instance_id) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "triggered",
                "instance_id": instance_id
            })),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        ).into_response(),
    }
}

async fn resume_sop_handler(
    State(state): State<WsState>,
    AxumPath(instance_id): AxumPath<String>,
) -> impl IntoResponse {
    let config = state.agent_loop.config.clone();
    match crate::sop::engine::resume_sop(config, instance_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "resumed"
            })),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        ).into_response(),
    }
}
