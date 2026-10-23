//! HTTP REST endpoints and logging middleware for the OpenZ Gateway.

use super::auth::is_authorized;
use super::WsState;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

const MODEL_PREFIX_ROUTING_TABLE: &[(&str, &str)] = &[
    ("gpt-", "openai"),
    ("o1", "openai"),
    ("o3-", "openai"),
    ("claude-", "anthropic"),
    ("deepseek-", "deepseek"),
];

const COMPLEX_PROMPT_KEYWORDS: &[&str] = &[
    "fix",
    "bug",
    "error",
    "implement",
    "refactor",
    "design",
    "build",
    "create",
    "write",
    "code",
    "architect",
    "schema",
    "test",
];

const SIMPLE_FALLBACK_PROVIDER_MODELS: &[(&str, &str)] = &[
    ("deepseek", "deepseek/deepseek-chat"),
    ("groq", "groq/llama-3.3-70b-specdec"),
    ("openai", "openai/gpt-4o-mini"),
    ("openrouter", "openrouter/google/gemini-2.5-flash-lite"),
];

pub(crate) async fn hono_log_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = redact_query_token(req.uri().query().unwrap_or(""));
    let full_path = match query.is_empty() {
        true => path.clone(),
        false => format!("{}?{}", path, query),
    };

    let method_str = method.as_str();
    let method_colored = match method_str {
        "GET" => "\x1b[1;36mGET\x1b[0m",
        "POST" => "\x1b[1;35mPOST\x1b[0m",
        "PUT" => "\x1b[1;33mPUT\x1b[0m",
        "DELETE" => "\x1b[1;31mDELETE\x1b[0m",
        _ => "\x1b[1;32mGET\x1b[0m",
    };

    let silent = std::env::var("OPENZ_SILENT").is_ok();
    match silent {
        false => println!(
            "  \x1b[1;30m-->\x1b[0m {} \x1b[37m{}\x1b[0m",
            method_colored, full_path
        ),
        true => {}
    }

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status().as_u16();

    let status_colored = match status {
        200..=299 => format!("\x1b[1;32m{}\x1b[0m", status),
        300..=399 => format!("\x1b[1;33m{}\x1b[0m", status),
        _ => format!("\x1b[1;31m{}\x1b[0m", status),
    };

    let duration_str = match (duration.as_secs() > 0, duration.as_millis() > 0) {
        (true, _) => format!("{:.2}s", duration.as_secs_f64()),
        (false, true) => format!("{}ms", duration.as_millis()),
        (false, false) => format!("{}μs", duration.as_micros()),
    };

    match silent {
        false => println!(
            "  \x1b[1;30m<--\x1b[0m {} \x1b[37m{}\x1b[0m {} \x1b[1;30m({})\x1b[0m",
            method_colored, path, status_colored, duration_str
        ),
        true => {}
    }

    response
}

pub(crate) fn redact_query_token(query: &str) -> String {
    query
        .split('&')
        .map(|part| match part.split_once('=') {
            Some((key, _)) => match key.eq_ignore_ascii_case("token") {
                true => format!("{key}=<redacted>"),
                false => part.to_string(),
            },
            None => part.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

pub(crate) async fn trigger_sop_handler(
    State(state): State<WsState>,
    headers: axum::http::HeaderMap,
    AxumPath(sop_id): AxumPath<String>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    match is_authorized(&headers, None) {
        false => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Unauthorized" })),
            )
                .into_response();
        }
        true => {}
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

pub(crate) async fn resume_sop_handler(
    State(state): State<WsState>,
    headers: axum::http::HeaderMap,
    AxumPath(instance_id): AxumPath<String>,
) -> impl IntoResponse {
    match is_authorized(&headers, None) {
        false => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Unauthorized" })),
            )
                .into_response();
        }
        true => {}
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
pub(crate) struct OpenAiChatCompletionRequest {
    pub(crate) model: String,
    pub(crate) messages: Vec<OpenAiMessage>,
    #[allow(dead_code)]
    pub(crate) stream: Option<bool>,
    pub(crate) user: Option<String>,
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct OpenAiMessage {
    pub(crate) role: String,
    pub(crate) content: serde_json::Value,
}

pub(crate) fn normalize_model_name(model: &str) -> String {
    let lower = model.to_lowercase();
    match lower.contains('/') {
        true => model.to_string(),
        false => MODEL_PREFIX_ROUTING_TABLE
            .iter()
            .find(|(prefix, _)| lower.starts_with(prefix))
            .map(|(_, provider)| format!("{provider}/{model}"))
            .unwrap_or_else(|| model.to_string()),
    }
}

pub(crate) fn determine_routed_model(
    config: &crate::config::schema::Config,
    request_model: &str,
    prompt: &str,
) -> String {
    let prompt_lower = prompt.to_lowercase();
    let is_complex = prompt.len() > 300
        || COMPLEX_PROMPT_KEYWORDS
            .iter()
            .any(|keyword| prompt_lower.contains(keyword));

    match is_complex {
        true => {
            let explicit_premium = request_model.contains('/')
                || request_model.starts_with("gpt-")
                || request_model.starts_with("claude-");
            match explicit_premium {
                true => request_model.to_string(),
                false => config.agents.defaults.model.clone(),
            }
        }
        false => SIMPLE_FALLBACK_PROVIDER_MODELS
            .iter()
            .find(|(provider, _)| {
                let (api_key, _) = config.resolve_provider_config(provider);
                !api_key.trim().is_empty()
            })
            .map(|(_, model)| (*model).to_string())
            .unwrap_or_else(|| request_model.to_string()),
    }
}

fn extract_openai_message_content(content: &serde_json::Value) -> String {
    match (content.as_str(), content.as_array()) {
        (Some(s), _) => s.to_string(),
        (None, Some(arr)) => arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        (None, None) => content.to_string(),
    }
}

pub(crate) async fn openai_chat_completions(
    State(state): State<WsState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<OpenAiChatCompletionRequest>,
) -> impl IntoResponse {
    match is_authorized(&headers, None) {
        false => {
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
        true => {}
    }

    let last_user_content = payload
        .messages
        .iter()
        .rfind(|m| m.role == "user")
        .map(|m| extract_openai_message_content(&m.content))
        .unwrap_or_default();

    let config = state.current_config();
    let req_model = normalize_model_name(&payload.model);
    let routed_model = determine_routed_model(&config, &req_model, &last_user_content);

    let session_key = payload
        .user
        .unwrap_or_else(|| "openai_proxy_default".to_string());

    let mut overrides = serde_json::Map::new();
    overrides.insert(
        "model".to_string(),
        serde_json::Value::String(routed_model.clone()),
    );
    let _ = crate::agent::agent_loop::save_session_overrides(
        &state.agent_loop.session_manager,
        &session_key,
        &overrides,
    )
    .await;

    let workspace = crate::config::loader::workspace_for_agent_turn(&config);
    let run_result = crate::config::loader::ACTIVE_WORKSPACE
        .scope(workspace, async {
            state
                .agent_loop
                .run(&last_user_content, &session_key)
                .await
        })
        .await;

    match run_result {
        Ok(res) => {
            let created = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let response = serde_json::json!({
                "id": format!("chatcmpl-{}", crate::core::short_uuid()),
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
#[path = "handlers_tests.rs"]
mod tests;
