use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use spider::website::Website;
use scraper::{Html, Selector};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Validate that an IP address is safe (not private, loopback, or reserved).
fn is_safe_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            !(v4.is_private() || v4.is_loopback() || v4.is_link_local()
                || v4.is_unspecified() || v4.is_broadcast())
        }
        std::net::IpAddr::V6(v6) => {
            !(v6.is_loopback() || v6.is_unspecified() || v6.is_multicast())
        }
    }
}

/// Validate URL to prevent SSRF — resolves DNS to catch rebinding attacks.
fn validate_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|e| anyhow!("Invalid URL: {}", e))?;
    let host = parsed.host_str().ok_or_else(|| anyhow!("URL has no host"))?.to_lowercase();

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(anyhow!("SSRF blocked: only http/https URLs are allowed (got '{}')", parsed.scheme()));
    }

    if host == "169.254.169.254" || host == "metadata.google.internal" {
        return Err(anyhow!("SSRF blocked: cloud metadata endpoints are not allowed"));
    }

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if !is_safe_ip(&ip) {
            return Err(anyhow!("SSRF blocked: private/reserved IP addresses are not allowed"));
        }
    }

    // DNS resolution check — prevents rebinding attacks
    use std::net::ToSocketAddrs;
    let resolved: Vec<_> = format!("{}:0", host)
        .to_socket_addrs()
        .map(|iter| iter.map(|addr| addr.ip()).collect())
        .unwrap_or_default();

    for ip in &resolved {
        if !is_safe_ip(ip) {
            return Err(anyhow!("SSRF blocked: hostname '{}' resolved to private/reserved IP {}", host, ip));
        }
    }

    if resolved.is_empty() {
        return Err(anyhow!("SSRF blocked: hostname '{}' could not be resolved", host));
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
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let url_str = arguments.get("url").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'url' parameter"))?;

        validate_url(url_str)?;
        
        let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(10).min(1000) as u32;
        let depth = arguments.get("depth").and_then(|v| v.as_u64()).unwrap_or(3).min(10) as usize;
        let respect = arguments.get("respect_robots_txt").and_then(|v| v.as_bool()).unwrap_or(true);
        let delay = arguments.get("delay").and_then(|v| v.as_u64()).unwrap_or(250).max(50);

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
                    
                    let title = doc.select(&title_selector)
                        .next()
                        .map(|el| el.text().collect::<Vec<_>>().join(" "))
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    
                    let body_text = doc.select(&body_selector)
                        .next()
                        .map(|el| el.text().collect::<Vec<_>>().join(" "))
                        .unwrap_or_else(|| html_str.clone());
                    
                    let snippet = if body_text.len() > 300 {
                        let mut snippet_str = body_text[..300].to_string();
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

        website.crawl().await;
        let _ = handle.await;

        let results = pages.lock().await.clone();
        Ok(Value::Array(results))
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
}
