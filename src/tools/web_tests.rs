use super::*;

#[test]
fn test_html_parsing() {
    let html = r#"
        <html>
            <head>
                <title>Test Page</title>
                <style>body { color: red; }</style>
            </head>
            <body>
                <h1>Hello World</h1>
                <p>This is a <b>test</b> page.</p>
                <script>console.log("ignore me");</script>
            </body>
        </html>
    "#;

    let document = Html::parse_document(html);
    let mut raw_text = String::new();
    walk_nodes(document.tree.root(), &mut raw_text);

    let clean = raw_text.trim();
    assert!(clean.contains("Hello World"));
    assert!(clean.contains("This is a test page."));
    assert!(!clean.contains("body {"));
    assert!(!clean.contains("console.log"));
}

#[test]
fn test_is_safe_ip() {
    use std::net::IpAddr;

    // Safe IPv4
    assert!(is_safe_ip(&"1.1.1.1".parse::<IpAddr>().unwrap()));
    assert!(is_safe_ip(&"8.8.8.8".parse::<IpAddr>().unwrap()));

    // Unsafe IPv4
    assert!(!is_safe_ip(&"127.0.0.1".parse::<IpAddr>().unwrap()));
    assert!(!is_safe_ip(&"10.0.0.1".parse::<IpAddr>().unwrap()));
    assert!(!is_safe_ip(&"192.168.1.1".parse::<IpAddr>().unwrap()));
    assert!(!is_safe_ip(&"172.16.0.1".parse::<IpAddr>().unwrap()));
    assert!(!is_safe_ip(&"169.254.169.254".parse::<IpAddr>().unwrap()));
    assert!(!is_safe_ip(&"0.0.0.0".parse::<IpAddr>().unwrap()));

    // Safe IPv6
    assert!(is_safe_ip(&"2001:db8::1".parse::<IpAddr>().unwrap()));

    // Unsafe IPv6
    assert!(!is_safe_ip(&"::1".parse::<IpAddr>().unwrap()));
    assert!(!is_safe_ip(&"::".parse::<IpAddr>().unwrap()));
    assert!(!is_safe_ip(&"fc00::1".parse::<IpAddr>().unwrap())); // ULA
    assert!(!is_safe_ip(&"fe80::1".parse::<IpAddr>().unwrap())); // Link-Local
    assert!(!is_safe_ip(&"ff02::1".parse::<IpAddr>().unwrap())); // Multicast

    // IPv4-mapped IPv6
    assert!(!is_safe_ip(&"::ffff:127.0.0.1".parse::<IpAddr>().unwrap()));
    assert!(!is_safe_ip(&"::ffff:10.0.0.1".parse::<IpAddr>().unwrap()));
    assert!(is_safe_ip(&"::ffff:8.8.8.8".parse::<IpAddr>().unwrap()));
}

#[test]
fn cache_control_max_age_sets_expiry() {
    let fetched_at = chrono::DateTime::parse_from_rfc3339("2026-07-26T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let expires = compute_expires_at(Some("public, max-age=120"), None, fetched_at).unwrap();
    assert_eq!(expires, fetched_at + chrono::Duration::seconds(120));
}

#[test]
fn cache_control_no_cache_requires_revalidation() {
    let fetched_at = chrono::DateTime::parse_from_rfc3339("2026-07-26T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let expires = compute_expires_at(Some("no-cache"), None, fetched_at).unwrap();
    assert_eq!(expires, fetched_at);
}

#[test]
fn missing_cache_control_uses_last_modified_heuristic() {
    let fetched_at = chrono::DateTime::parse_from_rfc3339("2026-07-26T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let expires =
        compute_expires_at(None, Some("Sat, 25 Jul 2026 00:00:00 GMT"), fetched_at).unwrap();
    assert!(expires > fetched_at);
}

#[test]
fn cache_control_no_store_skips_storage() {
    let fetched_at = chrono::DateTime::parse_from_rfc3339("2026-07-26T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    assert!(compute_expires_at(Some("no-store"), None, fetched_at).is_none());
}

#[test]
fn web_fetch_cache_mode_accepts_aliases() {
    assert_eq!(
        WebFetchCacheMode::from_args(&serde_json::json!({})).unwrap(),
        WebFetchCacheMode::Auto
    );
    assert_eq!(
        WebFetchCacheMode::from_args(&serde_json::json!({ "cache_mode": "refresh" })).unwrap(),
        WebFetchCacheMode::Revalidate
    );
    assert_eq!(
        WebFetchCacheMode::from_args(&serde_json::json!({ "cacheMode": "no-cache" })).unwrap(),
        WebFetchCacheMode::Bypass
    );
}

#[test]
fn web_fetch_detects_js_shell_pages_for_browser_retry() {
    let html = r#"<html><head><title>App</title></head><body><div id="root"></div><script src="/app.js"></script><noscript>You need to enable JavaScript to run this app.</noscript></body></html>"#;
    let text = extract_text_from_html(html);
    assert!(web_fetch_should_retry_browser_render(
        html,
        &text,
        &serde_json::json!({})
    ));
}

#[test]
fn web_fetch_browser_render_uses_broker_order() {
    assert_eq!(
        web_fetch_browser_render_backend_names(),
        ["gsd_browser", "firefox_browser", "obscura_browser"]
    );
}

#[test]
fn web_fetch_browser_retry_can_be_disabled_explicitly() {
    let html =
        r#"<html><body><div id="app"></div><script src="/bundle.js"></script></body></html>"#;
    let text = extract_text_from_html(html);
    assert!(!web_fetch_should_retry_browser_render(
        html,
        &text,
        &serde_json::json!({ "render_js": false })
    ));
    assert!(!web_fetch_should_retry_browser_render(
        html,
        &text,
        &serde_json::json!({ "render_js": "false" })
    ));
    assert!(!web_fetch_should_retry_browser_render(
        html,
        &text,
        &serde_json::json!({ "renderJs": "0" })
    ));
    assert!(web_fetch_should_retry_browser_render(
        html,
        &text,
        &serde_json::json!({ "render_js": "true" })
    ));
}

#[tokio::test]
async fn test_validate_url() {
    assert!(validate_url("http://example.com").await.is_ok());
    assert!(validate_url("https://google.com/search?q=rust")
        .await
        .is_ok());

    assert!(validate_url("ftp://example.com").await.is_err());
    assert!(validate_url("http://127.0.0.1").await.is_err());
    assert!(validate_url("http://localhost").await.is_err());
    assert!(validate_url("http://169.254.169.254").await.is_err());
    assert!(validate_url("http://[::1]").await.is_err());
    assert!(validate_url("http://[fc00::1]").await.is_err());
    assert!(validate_url("http://[::ffff:127.0.0.1]").await.is_err());
}
