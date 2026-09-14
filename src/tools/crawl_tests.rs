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
