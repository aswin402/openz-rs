use crate::tools::Tool;
use anyhow::{anyhow, Result};
use scraper::{Html, Selector};
use serde_json::{json, Value};
use spider::website::Website;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::tools::web::validate_url;

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

use crate::tools::arguments::{
    extract_normalized_url_from_aliases, get_bool_arg, get_u64_arg,
};

const CRAWL_URL_ALIASES: &[&str] = &[
    "url",
    "target_url",
    "targetUrl",
    "uri",
    "link",
    "target",
    "site",
    "website",
    "domain",
    "host",
    "start_url",
    "startUrl",
    "href",
    "page",
    "address",
];

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
    let status = match pages_crawled > 0 {
        true => "partial_success",
        false => "timeout",
    };
    json!({
        "status": status,
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
    extract_normalized_url_from_aliases(
        arguments,
        CRAWL_URL_ALIASES,
        "Missing 'url' parameter",
        "Missing 'url' parameter (received empty string)",
    )
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
                match count >= limit {
                    true => break,
                    false => {}
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

                    let snippet = match body_text.chars().count() > 300 {
                        true => {
                            let mut snippet_str: String = body_text.chars().take(300).collect();
                            snippet_str.push_str("...");
                            snippet_str
                        }
                        false => body_text.clone(),
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
        match crawl_timed_out {
            true => {
                handle.abort();
                let results = pages.lock().await.clone();
                Ok(crawl_timeout_response(
                    results,
                    timeout_secs,
                    limit,
                    depth,
                    url_str,
                ))
            }
            false => {
                let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
                let results = pages.lock().await.clone();
                match results.is_empty() {
                    true => Err(anyhow!(
                        "Crawl returned no results. The site may be unreachable or block automated access."
                    )),
                    false => Ok(Value::Array(results)),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "crawl_tests.rs"]
mod tests;
