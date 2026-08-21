use crate::tools::Tool;
use anyhow::{Result, anyhow};
use scraper::{Html, Selector};
use serde_json::{Value, json};
use spider::website::Website;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::tools::web::is_safe_ip;

/// Validate URL to prevent SSRF — resolves DNS to catch rebinding attacks.
async fn validate_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|e| anyhow!("Invalid URL: {}", e))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("URL has no host"))?
        .to_lowercase();

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(anyhow!(
            "SSRF blocked: only http/https URLs are allowed (got '{}')",
            parsed.scheme()
        ));
    }

    if host == "169.254.169.254" || host == "metadata.google.internal" {
        return Err(anyhow!(
            "SSRF blocked: cloud metadata endpoints are not allowed"
        ));
    }

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if !is_safe_ip(&ip) {
            return Err(anyhow!(
                "SSRF blocked: private/reserved IP addresses are not allowed"
            ));
        }
    }

    // DNS resolution check — prevents rebinding attacks
    let resolved: Vec<_> = match tokio::net::lookup_host(format!("{}:0", host)).await {
        Ok(iter) => iter.map(|addr| addr.ip()).collect(),
        Err(_) => Vec::new(),
    };

    for ip in &resolved {
        if !is_safe_ip(ip) {
            return Err(anyhow!(
                "SSRF blocked: hostname '{}' resolved to private/reserved IP {}",
                host,
                ip
            ));
        }
    }

    if resolved.is_empty() {
        return Err(anyhow!(
            "SSRF blocked: hostname '{}' could not be resolved",
            host
        ));
    }

    Ok(())
}

pub struct CrawlSiteTool;

impl Default for CrawlSiteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CrawlSiteTool {
    pub fn new() -> Self {
        CrawlSiteTool
    }
}

fn crawl_timeout_secs(arguments: &Value) -> u64 {
    arguments
        .get("timeout_secs")
        .or_else(|| arguments.get("timeout"))
        .and_then(|v| v.as_u64())
        .unwrap_or(45)
        .clamp(5, 300)
}

fn crawl_timeout_response(
    pages: Vec<Value>,
    timeout_secs: u64,
    limit: u32,
    depth: usize,
    url: &str,
) -> Value {
    let pages_crawled = pages.len();
    json!({
        "status": if pages_crawled > 0 { "partial_success" } else { "timeout" },
        "error_kind": "timeout",
        "retryable": true,
        "stopped_reason": "global_timeout",
        "timeout_secs": timeout_secs,
        "pages_crawled": pages_crawled,
        "limit": limit,
        "depth": depth,
        "start_url": url,
        "pages": pages,
        "next_step": "Retry with a smaller depth/limit, a higher timeout_secs value, or use searchxyz_site_map/searchxyz_read_url for targeted reads."
    })
}

#[async_trait::async_trait]
impl Tool for CrawlSiteTool {
    fn name(&self) -> &str {
        "crawl_website"
    }

    fn description(&self) -> &str {
        "Crawl a website starting from a URL and collect structured page information (URL, status, title, snippet/content)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The starting URL of the website to crawl."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of pages to fetch (default: 10)."
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum link depth to follow (default: 3)."
                },
                "respect_robots_txt": {
                    "type": "boolean",
                    "description": "Whether to respect robots.txt rules (default: true)."
                },
                "delay": {
                    "type": "integer",
                    "description": "Politeness delay between requests in milliseconds (default: 250)."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Global crawl timeout in seconds (default: 45, min: 5, max: 300). Returns partial_success with pages collected so far instead of hanging until the outer tool timeout."
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let url_str = arguments
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'url' parameter"))?;

        validate_url(url_str).await?;

        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(1000) as u32;
        let depth = arguments
            .get("depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(3)
            .min(10) as usize;
        let respect = arguments
            .get("respect_robots_txt")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let delay = arguments
            .get("delay")
            .and_then(|v| v.as_u64())
            .unwrap_or(250)
            .max(50);
        let timeout_secs = crawl_timeout_secs(arguments);

        let mut website = Website::new(url_str)
            .with_limit(limit)
            .with_depth(depth)
            .with_delay(delay)
            .with_respect_robots_txt(respect)
            .build()?;

        let mut rx = website.subscribe((limit.max(16)) as usize);

        let pages = Arc::new(Mutex::new(Vec::new()));
        let pages_clone = pages.clone();

        let handle = tokio::spawn(async move {
            let title_selector = Selector::parse("title").unwrap();
            let body_selector = Selector::parse("body").unwrap();
            let mut count = 0u32;
            while let Ok(page) = rx.recv().await {
                if count >= limit {
                    break;
                }
                let html_str = page.get_html();
                let (title, snippet) = {
                    let doc = Html::parse_document(&html_str);

                    let title = doc
                        .select(&title_selector)
                        .next()
                        .map(|el| el.text().collect::<Vec<_>>().join(" "))
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                    let body_text = doc
                        .select(&body_selector)
                        .next()
                        .map(|el| el.text().collect::<Vec<_>>().join(" "))
                        .unwrap_or_else(|| html_str.clone());

                    let snippet = if body_text.chars().count() > 300 {
                        let mut snippet_str: String = body_text.chars().take(300).collect();
                        snippet_str.push_str("...");
                        snippet_str
                    } else {
                        body_text.clone()
                    };
                    (title, snippet)
                };

                let status_u16 = page.status_code.as_u16();

                pages_clone.lock().await.push(json!({
                    "url": page.get_url(),
                    "status_code": status_u16,
                    "title": title,
                    "snippet": snippet.trim().replace('\n', " ").replace(r"\s+", " ")
                }));
                count += 1;
            }
        });

        let crawl_timed_out =
            tokio::time::timeout(Duration::from_secs(timeout_secs), website.crawl())
                .await
                .is_err();
        if crawl_timed_out {
            handle.abort();
            let results = pages.lock().await.clone();
            return Ok(crawl_timeout_response(
                results,
                timeout_secs,
                limit,
                depth,
                url_str,
            ));
        }
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;

        let results = pages.lock().await.clone();
        if results.is_empty() {
            Err(anyhow!(
                "Crawl returned no results. The site may be unreachable or block automated access."
            ))
        } else {
            Ok(Value::Array(results))
        }
    }
}

#[cfg(test)]
mod tests {
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
}
