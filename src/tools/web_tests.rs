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
        ["obscura_headless", "firefox_headless", "gsd_chrome_gui"]
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
    assert!(validate_url("http://1.1.1.1").await.is_ok());
    assert!(validate_url("https://[2001:db8::1]/search?q=rust")
        .await
        .is_ok());

    match validate_url("http://example.com").await {
        Ok(ip) => assert!(is_safe_ip(&ip)),
        Err(err) => assert!(err.to_string().contains("DNS lookup failed")),
    }

    assert!(validate_url("ftp://example.com").await.is_err());
    assert!(validate_url("http://127.0.0.1").await.is_err());
    assert!(validate_url("http://localhost").await.is_err());
    assert!(validate_url("http://169.254.169.254").await.is_err());
    assert!(validate_url("http://[::1]").await.is_err());
    assert!(validate_url("http://[fc00::1]").await.is_err());
    assert!(validate_url("http://[::ffff:127.0.0.1]").await.is_err());
    assert!(validate_url("http://[::ffff:169.254.169.254]").await.is_err());
}

#[tokio::test]
async fn test_web_fetch_offline_cache_hit_and_markdown_format() {
    let url = "https://cached-webfetch.example/guide";
    let html = r#"<html><head><title>OpenZ Guide</title></head><body><main><h1>Unified Engine</h1><p>Fast SQLite cache with <a href="https://docs.rs">Docs</a>.</p></main></body></html>"#;
    let headers = reqwest::header::HeaderMap::new();
    save_http_cache(url, url, html, "text/html", &headers, 200, 3600).unwrap();

    let tool = WebFetchTool::new();
    let plain = tool
        .call(&serde_json::json!({ "url": url, "format": "text" }))
        .await
        .unwrap();
    let plain_str = plain.as_str().unwrap_or_default();
    assert!(plain_str.contains("Unified Engine"));
    assert!(plain_str.contains("Fast SQLite cache"));

    let md = tool
        .call(&serde_json::json!({ "url": url, "format": "markdown" }))
        .await
        .unwrap();
    let md_str = md.as_str().unwrap_or_default();
    assert!(md_str.contains("Unified Engine"));
    assert!(md_str.contains("https://docs.rs"));
}

#[test]
fn test_normalize_web_url() {
    assert_eq!(normalize_web_url("example.com"), "https://example.com");
    assert_eq!(normalize_web_url("docs.rs/tokio"), "https://docs.rs/tokio");
    assert_eq!(
        normalize_web_url("http://already.http.com"),
        "http://already.http.com"
    );
    assert_eq!(
        normalize_web_url("https://already.https.com"),
        "https://already.https.com"
    );
    assert_eq!(normalize_web_url("//cdn.example.org"), "https://cdn.example.org");
    assert_eq!(
        normalize_web_url("<https://wrapped.example.com>"),
        "https://wrapped.example.com"
    );
    assert_eq!(
        normalize_web_url("\"https://quoted.example.com\""),
        "https://quoted.example.com"
    );
    assert_eq!(
        normalize_web_url("'https://singlequote.example.com'"),
        "https://singlequote.example.com"
    );
    assert_eq!(
        normalize_web_url("  crates.io/crates/serde  "),
        "https://crates.io/crates/serde"
    );
}

#[test]
fn test_parse_max_length() {
    assert_eq!(parse_max_length(&serde_json::json!({})), None);
    assert_eq!(
        parse_max_length(&serde_json::json!({ "max_length": 500 })),
        Some(500)
    );
    assert_eq!(
        parse_max_length(&serde_json::json!({ "limit": "250" })),
        Some(250)
    );
    assert_eq!(
        parse_max_length(&serde_json::json!({ "maxChars": "120" })),
        Some(120)
    );
    assert_eq!(
        parse_max_length(&serde_json::json!({ "maxLength": 75 })),
        Some(75)
    );
}

#[test]
fn test_truncate_web_fetch_output() {
    let full = "abcdefghijklmnopqrstuvwxyz";
    assert_eq!(truncate_web_fetch_output(full.to_string(), None), full);
    assert_eq!(
        truncate_web_fetch_output(full.to_string(), Some(50)),
        full
    );
    let truncated = truncate_web_fetch_output(full.to_string(), Some(10));
    assert!(truncated.starts_with("abcdefghij"));
    assert!(truncated.contains("[Content truncated at 10 chars; total length: 26 chars]"));
}

#[test]
fn test_web_fetch_parameters_schema_includes_max_length() {
    let tool = WebFetchTool::new();
    let params = tool.parameters();
    let props = params["properties"].as_object().expect("properties object");
    assert!(props.contains_key("max_length"));
    assert!(props.contains_key("url"));
    assert!(props.contains_key("urls"));
}

#[tokio::test]
async fn test_web_fetch_batch_urls_concurrent_offline_cache() {
    let url_a = "https://cached-batch.example/quote-a";
    let url_b = "https://cached-batch.example/quote-b";
    let headers = reqwest::header::HeaderMap::new();
    save_http_cache(
        url_a,
        url_a,
        r#"{"symbol":"AMD","price":185.5}"#,
        "application/json",
        &headers,
        200,
        3600,
    )
    .unwrap();
    save_http_cache(
        url_b,
        url_b,
        "<html><head><title>AMD Overview</title></head><body><p>Market Cap 300B</p></body></html>",
        "text/html",
        &headers,
        200,
        3600,
    )
    .unwrap();

    let tool = WebFetchTool::new();
    let res = tool
        .call(&serde_json::json!({
            "urls": [url_a, url_b, url_a]
        }))
        .await
        .unwrap();

    assert_eq!(res["status"], "success");
    assert_eq!(res["fetched"], 2);
    assert_eq!(res["failed"], 0);
    let results = res["results"].as_array().expect("results array");
    assert_eq!(results.len(), 2);
    assert!(results[0]["content"]
        .as_str()
        .unwrap_or_default()
        .contains("185.5"));
    assert_eq!(results[1]["title"], "AMD Overview");
    assert!(results[1]["content"]
        .as_str()
        .unwrap_or_default()
        .contains("Market Cap 300B"));
}

#[test]
fn test_extract_web_fetch_url_aliases() {
    assert_eq!(
        extract_web_fetch_url(&serde_json::json!("example.com")).unwrap(),
        "https://example.com"
    );
    assert_eq!(
        extract_web_fetch_url(&serde_json::json!({ "site": "crates.io" })).unwrap(),
        "https://crates.io"
    );
    assert_eq!(
        extract_web_fetch_url(&serde_json::json!({ "website": "https://rust-lang.org" })).unwrap(),
        "https://rust-lang.org"
    );
    assert_eq!(
        extract_web_fetch_url(&serde_json::json!({ "domain": "docs.rs" })).unwrap(),
        "https://docs.rs"
    );
    assert_eq!(
        extract_web_fetch_url(&serde_json::json!({ "host": "github.com" })).unwrap(),
        "https://github.com"
    );
    assert_eq!(
        extract_web_fetch_url(&serde_json::json!({ "href": "https://news.ycombinator.com" })).unwrap(),
        "https://news.ycombinator.com"
    );
    assert_eq!(
        extract_web_fetch_url(&serde_json::json!({ "page": "https://blog.rust-lang.org" })).unwrap(),
        "https://blog.rust-lang.org"
    );
    assert_eq!(
        extract_web_fetch_url(&serde_json::json!({ "path": "https://tokio.rs" })).unwrap(),
        "https://tokio.rs"
    );
    assert!(extract_web_fetch_url(&serde_json::json!({})).is_err());
    assert!(extract_web_fetch_url(&serde_json::json!("")).is_err());
}


