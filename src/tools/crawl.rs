use crate::tools::Tool;
use anyhow::{anyhow, Result};
use scraper::{Html, Selector};
use serde_json::{json, Value};
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

fn get_u64_arg(arguments: &Value, keys: &[&str], default: u64) -> u64 {
    for key in keys {
        if let Some(val) = arguments.get(*key) {
            if let Some(n) = val.as_u64() {
                return n;
            }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.trim().parse::<u64>() {
                    return n;
                }
            }
        }
    }
    default
}

fn get_bool_arg(arguments: &Value, keys: &[&str], default: bool) -> bool {
    for key in keys {
        if let Some(val) = arguments.get(*key) {
            if let Some(b) = val.as_bool() {
                return b;
            }
            if let Some(s) = val.as_str() {
                let trimmed = s.trim().to_lowercase();
                if trimmed == "true" || trimmed == "1" || trimmed == "yes" {
                    return true;
                } else if trimmed == "false" || trimmed == "0" || trimmed == "no" {
                    return false;
                }
            }
        }
    }
    default
}

fn crawl_timeout_secs(arguments: &Value) -> u64 {
    get_u64_arg(arguments, &["timeout_secs", "timeout", "timeoutSecs"], 45).clamp(5, 300)
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

pub fn extract_crawl_url(arguments: &Value) -> Result<String> {
    let raw_url = if let Some(s) = arguments.as_str() {
        s.trim()
    } else {
        arguments
            .get("url")
            .or_else(|| arguments.get("target_url"))
            .or_else(|| arguments.get("targetUrl"))
            .or_else(|| arguments.get("uri"))
            .or_else(|| arguments.get("link"))
            .or_else(|| arguments.get("target"))
            .or_else(|| arguments.get("site"))
            .or_else(|| arguments.get("website"))
            .or_else(|| arguments.get("domain"))
            .or_else(|| arguments.get("host"))
            .or_else(|| arguments.get("start_url"))
            .or_else(|| arguments.get("startUrl"))
            .or_else(|| arguments.get("href"))
            .or_else(|| arguments.get("page"))
            .or_else(|| arguments.get("address"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'url' parameter"))?
            .trim()
    };

    if raw_url.is_empty() {
        return Err(anyhow!("Missing 'url' parameter (received empty string)"));
    }

    Ok(crate::tools::web::normalize_web_url(raw_url))
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
                    "description": "The starting URL of the website to crawl (supports bare domains, site/domain aliases, or direct string)."
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
        let normalized_url = extract_crawl_url(arguments)?;
        let url_str = &normalized_url;

        validate_url(url_str).await?;

        let limit = get_u64_arg(
            arguments,
            &["limit", "max_pages", "maxPages", "max_results", "count"],
            10,
        )
        .min(1000) as u32;
        let depth = get_u64_arg(arguments, &["depth", "max_depth", "maxDepth"], 3).min(10) as usize;
        let respect = get_bool_arg(
            arguments,
            &[
                "respect_robots_txt",
                "respectRobotsTxt",
                "respect_robots",
                "respectRobots",
            ],
            true,
        );
        let delay = get_u64_arg(arguments, &["delay", "delay_ms", "delayMs"], 250).max(50);
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
            let title_selector = Selector::parse("title").ok();
            let body_selector = Selector::parse("body").ok();
            let mut count = 0u32;
            while let Ok(page) = rx.recv().await {
                if count >= limit {
                    break;
                }
                let html_str = page.get_html();
                let (title, snippet) = {
                    let doc = Html::parse_document(&html_str);

                    let title = title_selector
                        .as_ref()
                        .and_then(|sel| doc.select(sel).next())
                        .map(|el| el.text().collect::<Vec<_>>().join(" "))
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                    let body_text = body_selector
                        .as_ref()
                        .and_then(|sel| doc.select(sel).next())
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
#[path = "crawl_tests.rs"]
mod tests;
