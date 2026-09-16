//! HTTP REST endpoints and logging middleware for the OpenZ Gateway.

use super::auth::is_authorized;
use super::WsState;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

pub(crate) async fn hono_log_middleware(
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

pub(crate) fn redact_query_token(query: &str) -> String {
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

pub(crate) async fn trigger_sop_handler(
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

pub(crate) async fn resume_sop_handler(
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

pub(crate) fn determine_routed_model(
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

pub(crate) async fn openai_chat_completions(
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

#[cfg(test)]
#[path = "handlers_tests.rs"]
mod tests;
