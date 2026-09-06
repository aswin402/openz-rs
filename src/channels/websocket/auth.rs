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

    is_allowed_origin(origin, config)
}

pub(crate) fn is_allowed_origin(origin: &str, config: &WebSocketChannelConfig) -> bool {
    let port = config.port.to_string();
    let configured_http = ["http://", &config.host, ":", &port].concat();
    let configured_https = ["https://", &config.host, ":", &port].concat();

    origin == configured_http
        || origin == configured_https
        || config.cors_origins.iter().any(|allowed| allowed == origin)
}

pub(crate) fn websocket_cors_origins(config: &WebSocketChannelConfig) -> Vec<HeaderValue> {
    let mut origins = vec![
        HeaderValue::from_static("http://localhost"),
        HeaderValue::from_static("http://127.0.0.1"),
    ];

    for custom in &config.cors_origins {
        if let Ok(hv) = HeaderValue::from_str(custom) {
            if !origins.contains(&hv) {
                origins.push(hv);
            }
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_allowed_origin_with_custom_configured_origins() {
        let mut config = WebSocketChannelConfig::default();
        config.cors_origins.push("https://custom.app.domain".to_string());

        assert!(is_allowed_origin("https://custom.app.domain", &config));
        assert!(is_allowed_origin("http://localhost:3000", &config));
        assert!(!is_allowed_origin("https://evil.site.com", &config));
    }
}
