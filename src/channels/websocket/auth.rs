//! WebSocket and gateway authentication policy.
//!
//! This module owns origin allowlisting and gateway-token checks. It has no
//! connection or command state, which keeps security policy independently
//! reviewable while preserving the existing gateway behavior.

use crate::config::schema::WebSocketChannelConfig;
use axum::http::{header, HeaderMap, HeaderValue};

pub(crate) fn gateway_token_required_for_host(host: &str) -> bool {
    let normalized = host.trim().trim_start_matches('[').trim_end_matches(']');
    match normalized.eq_ignore_ascii_case("localhost") {
        true => false,
        false => match normalized.parse::<std::net::IpAddr>() {
            Ok(ip) => !ip.is_loopback(),
            Err(_) => true,
        },
    }
}

pub(crate) fn websocket_origin_allowed(
    origin: Option<&str>,
    config: &WebSocketChannelConfig,
    gateway_token_is_configured: bool,
) -> bool {
    match origin {
        Some(origin) => is_allowed_origin(origin, config),
        None => gateway_token_is_configured,
    }
}

pub(crate) fn is_allowed_origin(origin: &str, config: &WebSocketChannelConfig) -> bool {
    let port = config.port.to_string();
    let configured_http = ["http://", &config.host, ":", &port].concat();
    let configured_https = ["https://", &config.host, ":", &port].concat();

    origin == configured_http
        || origin == configured_https
        || config
            .cors_origins
            .iter()
            .any(|allowed| allowed == "*" || allowed == origin)
}

pub(crate) fn websocket_cors_origins(config: &WebSocketChannelConfig) -> Vec<HeaderValue> {
    let mut origins = vec![
        HeaderValue::from_static("http://localhost"),
        HeaderValue::from_static("http://127.0.0.1"),
    ];

    for custom in &config.cors_origins {
        match HeaderValue::from_str(custom) {
            Ok(hv) => match origins.contains(&hv) {
                true => {}
                false => origins.push(hv),
            },
            Err(_) => {}
        }
    }

    for scheme in ["http", "https"] {
        let origin = format!("{scheme}://{}:{}", config.host, config.port);
        match HeaderValue::try_from(origin) {
            Ok(header) => match origins.contains(&header) {
                true => {}
                false => origins.push(header),
            },
            Err(_) => {}
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
    match expected.is_empty() {
        true => true,
        false => {
            let query_matches = query_token
                .map(|tok| crate::channels::secure_compare(tok, &expected))
                .unwrap_or(false);
            let bearer_matches = headers
                .get(header::AUTHORIZATION)
                .and_then(|auth_header| auth_header.to_str().ok())
                .and_then(|auth_str| auth_str.strip_prefix("Bearer "))
                .map(|token| crate::channels::secure_compare(token.trim(), &expected))
                .unwrap_or(false);
            query_matches || bearer_matches
        }
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
