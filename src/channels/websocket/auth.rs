//! WebSocket and gateway authentication policy.
//!
//! This module owns origin allowlisting and gateway-token checks. It has no
//! connection or command state, which keeps security policy independently
//! reviewable while preserving the existing gateway behavior.

use crate::config::schema::WebSocketChannelConfig;
use axum::http::{header, HeaderMap, HeaderValue};

pub(crate) fn gateway_token_required_for_host(host: &str) -> bool {
    let normalized = host.trim().trim_start_matches('[').trim_end_matches(']');
    if normalized.eq_ignore_ascii_case("localhost") {
        return false;
    }
    match normalized.parse::<std::net::IpAddr>() {
        Ok(ip) => !ip.is_loopback(),
        Err(_) => true,
    }
}

pub(crate) fn websocket_origin_allowed(
    origin: Option<&str>,
    config: &WebSocketChannelConfig,
    gateway_token_is_configured: bool,
) -> bool {
    let Some(origin) = origin else {
        return gateway_token_is_configured;
    };

    let port = config.port.to_string();
    let configured_http = ["http://", &config.host, ":", &port].concat();
    let configured_https = ["https://", &config.host, ":", &port].concat();
    let local_origins = [
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://localhost:8765",
        "http://127.0.0.1:8765",
    ];

    origin == configured_http
        || origin == configured_https
        || local_origins.contains(&origin)
}

pub(crate) fn websocket_cors_origins(config: &WebSocketChannelConfig) -> Vec<HeaderValue> {
    let mut origins = vec![
        HeaderValue::from_static("http://localhost"),
        HeaderValue::from_static("http://127.0.0.1"),
        HeaderValue::from_static("http://localhost:3000"),
        HeaderValue::from_static("http://127.0.0.1:3000"),
        HeaderValue::from_static("http://localhost:5173"),
        HeaderValue::from_static("http://127.0.0.1:5173"),
        HeaderValue::from_static("http://localhost:8765"),
        HeaderValue::from_static("http://127.0.0.1:8765"),
    ];

    for scheme in ["http", "https"] {
        let origin = format!("{scheme}://{}:{}", config.host, config.port);
        if let Ok(header) = HeaderValue::try_from(origin) {
            if !origins.contains(&header) {
                origins.push(header);
            }
        }
    }

    origins
}

pub(crate) fn gateway_token_configured() -> bool {
    std::env::var("OPENZ_GATEWAY_TOKEN")
        .map(|token| !token.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn is_authorized(headers: &HeaderMap, query_token: Option<&str>) -> bool {
    let expected = std::env::var("OPENZ_GATEWAY_TOKEN").unwrap_or_default();
    // When no token is configured, allow all connections (open local access).
    if expected.is_empty() {
        return true;
    }
    // Token is configured — enforce it via query param or Authorization header.
    if let Some(tok) = query_token {
        if crate::channels::secure_compare(tok, &expected) {
            return true;
        }
    }
    if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if crate::channels::secure_compare(token.trim(), &expected) {
                    return true;
                }
            }
        }
    }
    false
}
