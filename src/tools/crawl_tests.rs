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
        extract_crawl_url(&json!({ "website": "https://rust-lang.org" })).unwrap(),
        "https://rust-lang.org"
    );
    assert_eq!(
        extract_crawl_url(&json!({ "host": "github.com" })).unwrap(),
        "https://github.com"
    );
    assert_eq!(
        extract_crawl_url(&json!({ "href": "https://crates.io" })).unwrap(),
        "https://crates.io"
    );
    assert_eq!(
        extract_crawl_url(&json!({ "page": "https://docs.rs" })).unwrap(),
        "https://docs.rs"
    );
    assert_eq!(
        extract_crawl_url(&json!({ "address": "127.0.0.1" })).unwrap(),
        "https://127.0.0.1"
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


#[tokio::test]
async fn execute_bfs_crawl_respects_robots_txt_and_follows_internal_links() {
    use crate::tools::searchxyz::core::config::{
        CrawlerConfig, ExtractorConfig, HeadlessConfig, ProxyConfig,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _)) => {
                    tokio::spawn(async move {
                        let mut buf = [0_u8; 2048];
                        let n = stream.read(&mut buf).await.unwrap_or(0);
                        let req = String::from_utf8_lossy(&buf[..n]);
                        let first_line = req.lines().next().unwrap_or("");
                        let path = first_line.split_whitespace().nth(1).unwrap_or("/");

                        let (ct, body) = match path {
                            "/robots.txt" => ("text/plain", "User-agent: *\nDisallow: /private\n"),
                            "/" => (
                                "text/html",
                                "<html><head><title>Home</title></head><body><p>Welcome home</p><a href=\"/docs\">Docs</a><a href=\"/private\">Secret</a></body></html>",
                            ),
                            "/docs" => (
                                "text/html",
                                "<html><head><title>Docs</title></head><body><p>Documentation body</p></body></html>",
                            ),
                            "/private" => (
                                "text/html",
                                "<html><head><title>Private</title></head><body><p>Should never be crawled</p></body></html>",
                            ),
                            _ => ("text/html", "<html><body>404</body></html>"),
                        };
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: {ct}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                    });
                }
                Err(_) => return,
            }
        }
    });

    let mut crawler_cfg = CrawlerConfig::default();
    crawler_cfg.allow_private_network = true;
    crawler_cfg.rate_limit_per_sec = 50;
    let crawler = Crawler::new(
        crawler_cfg,
        HeadlessConfig::default(),
        ProxyConfig::default(),
    );
    let extractor = ExtractionPipeline::new(ExtractorConfig::default());
    let sink = Arc::new(Mutex::new(Vec::new()));
    let options = CrawlOptions {
        limit: 10,
        depth: 2,
        respect_robots_txt: true,
        use_sitemap: false,
        extract_markdown: true,
        render_js: false,
        delay_ms: 0,
    };

    let start = format!("http://{addr}/");
    execute_bfs_crawl(&start, &crawler, &extractor, &options, sink.clone())
        .await
        .unwrap();

    let crawled = sink.lock().await;
    assert_eq!(crawled.len(), 2);
    let urls: Vec<&str> = crawled
        .iter()
        .filter_map(|p| p["url"].as_str())
        .collect();
    assert!(urls.iter().any(|u| u.ends_with('/')));
    assert!(urls.iter().any(|u| u.ends_with("/docs")));
    assert!(!urls.iter().any(|u| u.contains("/private")));
    assert!(crawled[0].get("markdown").is_some());
}
