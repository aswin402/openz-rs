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
