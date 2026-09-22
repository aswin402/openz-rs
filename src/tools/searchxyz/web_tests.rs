use super::*;
use crate::tools::searchxyz::{
    coerce_bool_fields, coerce_numeric_fields, map_field_alias, normalize_query_arg,
    normalize_url_arg,
};

#[test]
fn backend_health_labels_cover_expected_states() {
    let mut config = crate::tools::searchxyz::core::config::Config::default();
    config.search.backends = vec![
        "searxng".to_string(),
        "brave".to_string(),
        "duckduckgo".to_string(),
    ];
    config.searxng.instance_url = "http://searxng.local".to_string();
    config.brave.api_key = None;

    assert_eq!(
        backend_health_label("searxng", &config.search.backends, &config),
        "preferred (configured)"
    );
    assert_eq!(
        backend_health_label("brave", &config.search.backends, &config),
        "missing_key"
    );
    assert_eq!(
        backend_health_label("duckduckgo", &config.search.backends, &config),
        "keyless"
    );
    assert_eq!(
        backend_health_label("google", &config.search.backends, &config),
        "disabled"
    );
}

#[test]
fn only_scraper_backends_detects_keyless_only_order() {
    assert!(only_scraper_backends_enabled(&[
        "duckduckgo".to_string(),
        "google".to_string(),
        "bing".to_string(),
    ]));
    assert!(!only_scraper_backends_enabled(&[
        "searxng".to_string(),
        "duckduckgo".to_string(),
    ]));
}

#[test]
fn browser_search_builds_search_urls() {
    let duckduckgo = browser_search_url("duckduckgo", "rust async").unwrap();
    let bing = browser_search_url("bing", "rust async").unwrap();

    assert!(duckduckgo.starts_with("https://duckduckgo.com/html/"));
    assert!(duckduckgo.contains("q=rust+async"));
    assert!(bing.starts_with("https://www.bing.com/search"));
    assert!(bing.contains("q=rust+async"));
}

#[test]
fn browser_search_extracts_and_dedupes_result_links() {
    let html = r#"
        <a href="https://example.com/a"><h2>A</h2></a>
        <a href="/l/?uddg=https%3A%2F%2Fexample.com%2Fb">B</a>
        <a href="https://example.com/a">Duplicate</a>
        <a href="javascript:void(0)">Ignore</a>
    "#;

    let results = extract_browser_search_results("duckduckgo", html, 10);

    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["url"], "https://example.com/a");
    assert_eq!(results[1]["url"], "https://example.com/b");
}

#[test]
fn browser_search_extracts_rendered_dom_results() {
    let payload = json!({
        "results": [
            { "title": " OpenZ  repository ", "url": "https://github.com/aswin402/openz-rs" },
            { "title": "Bing internal", "url": "https://www.bing.com/search?q=openz" },
            { "title": "Duplicate", "url": "https://github.com/aswin402/openz-rs" },
            { "title": "DuckDuckGo redirect", "url": "https://duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fdocs" }
        ]
    })
    .to_string();

    let results = extract_rendered_browser_search_results("bing", &payload, 10);

    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["title"], "OpenZ repository");
    assert_eq!(results[0]["source"], "browser_search_rendered");
    assert_eq!(results[1]["url"], "https://example.com/docs");
}

#[test]
fn browser_search_extracts_string_wrapped_rendered_payload() {
    let inner = json!({
        "results": [{ "title": "Rust docs", "url": "https://docs.rs/tokio" }]
    })
    .to_string();
    let wrapped = json!(inner).to_string();

    let results = extract_rendered_browser_search_results("duckduckgo", &wrapped, 10);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["url"], "https://docs.rs/tokio");
}

#[test]
fn browser_search_tool_metadata_is_web_safe() {
    let tool = SearchXyzBrowserSearchTool;
    assert_eq!(tool.name(), "searchxyz_browser_search");
    assert_eq!(tool.metadata().domain, "web");
    assert!(tool.description().contains("headless-first"));
    assert!(tool.parameters()["properties"].get("query").is_some());
    assert!(tool.parameters()["properties"].get("engine").is_some());
}

#[test]
fn browser_search_read_options_default_to_links_only() {
    let args = json!({ "query": "rust async", "max_results": 4 });
    assert!(!browser_search_read_top_results_enabled(&args));
    assert_eq!(browser_search_max_pages(&args, 4), 3);

    let tool = SearchXyzBrowserSearchTool;
    let properties = &tool.parameters()["properties"];
    assert!(properties.get("read_top_results").is_some());
    assert!(properties.get("max_pages").is_some());
    assert!(properties.get("save_mode").is_some());
}

#[test]
fn browser_search_read_limits_are_clamped() {
    assert_eq!(browser_search_max_pages(&json!({ "max_pages": 0 }), 10), 1);
    assert_eq!(browser_search_max_pages(&json!({ "max_pages": 99 }), 10), 5);
    assert_eq!(browser_search_max_pages(&json!({}), 2), 2);
}

#[test]
fn search_failure_classification_covers_blocking_and_empty_results() {
    assert_eq!(
        classify_search_failure("HTTP 403 Forbidden"),
        SearchFailureKind::Blocked
    );
    assert_eq!(
        classify_search_failure("HTTP 429 Too Many Requests"),
        SearchFailureKind::RateLimited
    );
    assert_eq!(
        classify_search_failure("All search backends exhausted: 0 usable results"),
        SearchFailureKind::NoUsableResults
    );
    assert_eq!(
        classify_search_failure("request timed out after 30 seconds"),
        SearchFailureKind::Timeout
    );
}

#[test]
fn search_failure_cooldowns_are_bounded_by_kind() {
    assert_eq!(
        search_failure_cooldown_secs(SearchFailureKind::RateLimited),
        1800
    );
    assert_eq!(
        search_failure_cooldown_secs(SearchFailureKind::Blocked),
        3600
    );
    assert_eq!(
        search_failure_cooldown_secs(SearchFailureKind::Timeout),
        300
    );
    assert_eq!(
        search_failure_cooldown_secs(SearchFailureKind::NoUsableResults),
        600
    );
}

#[test]
fn search_failure_payload_is_machine_readable() {
    let payload = search_failure_error_value(
        "native search failed",
        SearchFailureKind::RateLimited,
        &["duckduckgo".to_string(), "google".to_string()],
    );
    assert_eq!(payload["status"], "search_failed");
    assert_eq!(payload["error_kind"], "rate_limited");
    assert_eq!(payload["retryable"], true);
    assert!(payload["next_step"]
        .as_str()
        .unwrap()
        .contains("searchxyz_browser_search"));
}

#[test]
fn search_backend_cooldown_records_active_backends() {
    clear_search_backend_cooldowns_for_tests();
    record_search_backend_failure(
        &["duckduckgo".to_string(), "google".to_string()],
        SearchFailureKind::Blocked,
    );
    let active = active_search_backend_cooldowns();
    assert_eq!(active.len(), 2);
    assert!(active.iter().any(|entry| entry["backend"] == "duckduckgo"));
    assert!(active.iter().any(|entry| entry["error_kind"] == "blocked"));
    assert!(active
        .iter()
        .all(|entry| entry["cooldown_remaining_secs"].as_u64().unwrap() > 0));
    clear_search_backend_cooldowns_for_tests();
}

#[tokio::test]
async fn searchxyz_doctor_reports_active_cooldowns() -> Result<()> {
    clear_search_backend_cooldowns_for_tests();
    record_search_backend_failure(&["duckduckgo".to_string()], SearchFailureKind::RateLimited);
    let report = build_searchxyz_doctor_report(false).await?;
    assert!(report.contains("## Backend Cooldowns"));
    assert!(report.contains("duckduckgo"));
    assert!(report.contains("rate_limited"));
    clear_search_backend_cooldowns_for_tests();
    Ok(())
}

#[tokio::test]
async fn searchxyz_doctor_mentions_automatic_browser_fallback() -> Result<()> {
    let report = build_searchxyz_doctor_report(false).await?;
    assert!(report.contains("searchxyz_browser_search"));
    assert!(report.contains("Provider-free browser fallback is automatic"));
    Ok(())
}

#[tokio::test]
async fn searchxyz_doctor_accepts_direct_string_and_aliases() -> Result<()> {
    let tool = SearchXyzDoctorTool;
    let res = tool.call(&json!("false")).await?;
    let text = res.as_str().expect("string report");
    assert!(!text.contains("Index path:"));

    let res_alias = tool.call(&json!({ "paths": false })).await?;
    let text_alias = res_alias.as_str().expect("string report");
    assert!(!text_alias.contains("Index path:"));
    Ok(())
}

#[test]
fn searchxyz_web_args_normalization_and_aliases() {
    let raw = json!({
        "q": "rust async framework",
        "limit": "8",
        "merge": "1",
        "diagnostics": "true"
    });
    let mut normalized = normalize_query_arg(&raw);
    map_field_alias(&mut normalized, "max_results", &["limit", "count", "maxResults", "max_pages"]);
    map_field_alias(&mut normalized, "merge_backends", &["mergeBackends", "merge"]);
    map_field_alias(&mut normalized, "include_diagnostics", &["includeDiagnostics", "diagnostics"]);
    coerce_numeric_fields(&mut normalized, &["max_results"]);
    coerce_bool_fields(&mut normalized, &["merge_backends", "include_diagnostics"]);

    assert_eq!(normalized["query"], "rust async framework");
    assert_eq!(normalized["max_results"], json!(8));
    assert_eq!(normalized["merge_backends"], json!(true));
    assert_eq!(normalized["include_diagnostics"], json!(true));
}

#[test]
fn searchxyz_read_url_args_normalization_and_aliases() {
    let raw = json!("docs.rs/tokio");
    let mut normalized = normalize_url_arg(&raw);
    map_field_alias(&mut normalized, "depth", &["max_depth", "crawl_depth", "maxDepth"]);
    map_field_alias(&mut normalized, "max_chars", &["maxChars", "limit", "max_length", "maxLength"]);
    map_field_alias(&mut normalized, "render_js", &["renderJs", "js"]);
    coerce_numeric_fields(&mut normalized, &["depth", "max_chars"]);
    coerce_bool_fields(&mut normalized, &["render_js"]);

    assert_eq!(normalized["url"], "https://docs.rs/tokio");
}

#[test]
fn searchxyz_site_map_args_normalization_and_aliases() {
    let raw = json!({
        "link": "example.com",
        "sitemap": "0",
        "crawl": "yes",
        "limit": "25"
    });
    let mut normalized = normalize_url_arg(&raw);
    map_field_alias(&mut normalized, "max_links", &["limit", "max_results", "maxLinks", "count"]);
    map_field_alias(&mut normalized, "use_sitemap", &["useSitemap", "sitemap"]);
    map_field_alias(&mut normalized, "crawl_links", &["crawlLinks", "crawl"]);
    coerce_numeric_fields(&mut normalized, &["max_links"]);
    coerce_bool_fields(&mut normalized, &["use_sitemap", "crawl_links"]);

    assert_eq!(normalized["url"], "https://example.com");
    assert_eq!(normalized["max_links"], json!(25));
    assert_eq!(normalized["use_sitemap"], json!(false));
    assert_eq!(normalized["crawl_links"], json!(true));
}

