use super::*;

#[tokio::test]
async fn test_crawl_site_tool_metadata() -> Result<()> {
    let tool = CrawlSiteTool::new();
    assert_eq!(tool.name(), "crawl_website");
    let params = tool.parameters();
    assert!(params.get("properties").is_some());
    Ok(())
}

#[test]
fn crawl_timeout_defaults_and_clamps() {
    assert_eq!(crawl_timeout_secs(&json!({})), 45);
    assert_eq!(crawl_timeout_secs(&json!({ "timeout_secs": 1 })), 5);
    assert_eq!(crawl_timeout_secs(&json!({ "timeout_secs": 999 })), 300);
    assert_eq!(crawl_timeout_secs(&json!({ "timeout": 12 })), 12);
    assert_eq!(crawl_timeout_secs(&json!({ "timeout_secs": "15" })), 15);
    assert_eq!(crawl_timeout_secs(&json!({ "timeoutSecs": "25" })), 25);
}

#[test]
fn crawl_arg_coercion_supports_strings_and_aliases() {
    let args = json!({
        "limit": "4",
        "maxDepth": "2",
        "delayMs": "300",
        "respectRobotsTxt": "false"
    });
    assert_eq!(get_u64_arg(&args, &["limit", "max_pages", "maxPages"], 10), 4);
    assert_eq!(get_u64_arg(&args, &["depth", "max_depth", "maxDepth"], 3), 2);
    assert_eq!(get_u64_arg(&args, &["delay", "delay_ms", "delayMs"], 250), 300);
    assert!(!get_bool_arg(&args, &["respect_robots_txt", "respectRobotsTxt", "respect_robots"], true));

    let truthy_args = json!({ "respect_robots": "true" });
    assert!(get_bool_arg(&truthy_args, &["respect_robots_txt", "respectRobotsTxt", "respect_robots"], false));
}

#[test]
fn crawl_schema_exposes_global_timeout() {
    let tool = CrawlSiteTool::new();
    let params = tool.parameters();
    let props = params["properties"].as_object().expect("properties object");
    assert!(props.contains_key("timeout_secs"));
}

#[test]
fn crawl_timeout_response_preserves_partial_pages() {
    let pages = vec![json!({ "url": "https://example.com", "title": "Example" })];
    let response = crawl_timeout_response(pages, 5, 3, 1, "https://example.com");
    assert_eq!(response["status"], "partial_success");
    assert_eq!(response["error_kind"], "timeout");
    assert_eq!(response["pages_crawled"], 1);
    assert_eq!(response["pages"][0]["title"], "Example");
}

#[test]
fn test_extract_crawl_url_supports_aliases_and_schemeless() {
    assert_eq!(
        extract_crawl_url(&json!("example.com")).unwrap(),
        "https://example.com"
    );
    assert_eq!(
        extract_crawl_url(&json!({ "site": "docs.rs" })).unwrap(),
        "https://docs.rs"
    );
    assert_eq!(
        extract_crawl_url(&json!({ "start_url": "https://crates.io" })).unwrap(),
        "https://crates.io"
    );
    assert_eq!(
        extract_crawl_url(&json!({ "domain": "my-site.org" })).unwrap(),
        "https://my-site.org"
    );
    assert_eq!(
        extract_crawl_url(&json!({ "targetUrl": "http://insecure.org" })).unwrap(),
        "http://insecure.org"
    );
    assert!(extract_crawl_url(&json!({})).is_err());
    assert!(extract_crawl_url(&json!("")).is_err());
}

#[test]
fn test_crawl_arg_coercion_extra_aliases() {
    let args1 = json!({ "max_results": "15" });
    assert_eq!(
        get_u64_arg(&args1, &["limit", "max_pages", "maxPages", "max_results", "count"], 10),
        15
    );
    let args2 = json!({ "count": 25 });
    assert_eq!(
        get_u64_arg(&args2, &["limit", "max_pages", "maxPages", "max_results", "count"], 10),
        25
    );
}

