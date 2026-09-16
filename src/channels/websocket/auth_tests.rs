use super::*;

#[test]
fn test_is_allowed_origin_with_custom_configured_origins() {
    let mut config = WebSocketChannelConfig::default();
    config.cors_origins.push("https://custom.app.domain".to_string());

    assert!(is_allowed_origin("https://custom.app.domain", &config));
    assert!(is_allowed_origin("http://localhost:3000", &config));
    assert!(!is_allowed_origin("https://evil.site.com", &config));

    // Test wildcard support
    let mut wildcard_config = WebSocketChannelConfig::default();
    wildcard_config.cors_origins.push("*".to_string());
    assert!(is_allowed_origin("https://any-domain.example.com", &wildcard_config));
}

#[test]
fn gateway_token_required_for_host_rejects_public_binds() {
    assert!(!gateway_token_required_for_host("127.0.0.1"));
    assert!(!gateway_token_required_for_host("localhost"));
    assert!(!gateway_token_required_for_host("::1"));
    assert!(gateway_token_required_for_host("0.0.0.0"));
    assert!(gateway_token_required_for_host("192.168.1.10"));
}

#[test]
fn websocket_origin_validation_rejects_untrusted_browser_origins() {
    let config = WebSocketChannelConfig {
        enabled: true,
        host: "127.0.0.1".to_string(),
        port: 8765,
        start_on_boot: false,
        start_on_tui: false,
        ..Default::default()
    };

    assert!(websocket_origin_allowed(Some("http://127.0.0.1:8765"), &config, false));
    assert!(websocket_origin_allowed(Some("http://localhost:5173"), &config, false));
    assert!(!websocket_origin_allowed(Some("https://evil.example"), &config, false));
    assert!(!websocket_origin_allowed(None, &config, false));
    assert!(websocket_origin_allowed(None, &config, true));
}

#[test]
fn websocket_cors_origins_include_configured_and_vite_origins() {
    let config = WebSocketChannelConfig {
        enabled: true,
        host: "192.168.1.20".to_string(),
        port: 9000,
        start_on_boot: false,
        start_on_tui: false,
        ..Default::default()
    };

    let origins = websocket_cors_origins(&config);
    assert!(origins.iter().any(|origin| origin == "http://192.168.1.20:9000"));
    assert!(origins.iter().any(|origin| origin == "https://192.168.1.20:9000"));
    assert!(origins.iter().any(|origin| origin == "http://localhost:5173"));
    assert!(origins.iter().any(|origin| origin == "http://127.0.0.1:5173"));
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
