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
            if std::env::var("OPENZ_GATEWAY_TOKEN")
                .map(|t| t.is_empty())
                .unwrap_or(true)
            {
                println!("⚠️ WARNING: OPENZ_GATEWAY_TOKEN is not set or is empty. All gateway requests will be rejected for security!");
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

                        let agent = state.agent_loop.clone();
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

                            match agent
                                .run(&content_str, &format!("ws:{}", chat_id_clone))
                                .await
                            {
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
    let config = state.agent_loop.config.clone();
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
    let config = state.agent_loop.config.clone();
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

    let mut config = state.agent_loop.config.clone();
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
            extra: std::collections::HashMap::new(),
        });
        let model_routed = determine_routed_model(&config, "gpt-4o", "Hi there");
        assert_eq!(model_routed, "deepseek/deepseek-chat");
    }

    #[test]
    fn test_is_authorized() {
        use axum::http::HeaderMap;

        // Unset token -> unauthorized
        std::env::remove_var("OPENZ_GATEWAY_TOKEN");
        let headers = HeaderMap::new();
        assert!(!is_authorized(&headers, None));
        assert!(!is_authorized(&headers, Some("test")));

        // Empty token -> unauthorized
        std::env::set_var("OPENZ_GATEWAY_TOKEN", "");
        assert!(!is_authorized(&headers, None));
        assert!(!is_authorized(&headers, Some("")));

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
    if let Ok(expected) = std::env::var("OPENZ_GATEWAY_TOKEN") {
        if expected.is_empty() {
            return false;
        }
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
    }
    false
}
