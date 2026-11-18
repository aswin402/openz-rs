use crate::tools::arguments::{
    extract_normalized_url_from_aliases, get_bool_arg, get_u64_arg,
};
use crate::tools::searchxyz::core::crawler::robots::RobotsRules;
use crate::tools::searchxyz::core::crawler::{sitemap, Crawler};
use crate::tools::searchxyz::core::extractor::ExtractionPipeline;
use crate::tools::web::validate_url;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use scraper::{Html, Selector};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use url::Url;

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

#[derive(Debug, Clone)]
pub(crate) struct CrawlOptions {
    pub limit: u32,
    pub depth: usize,
    pub respect_robots_txt: bool,
    pub use_sitemap: bool,
    pub extract_markdown: bool,
    pub render_js: bool,
    pub delay_ms: u64,
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
        "next_step": "Retry with a smaller depth/limit, a higher timeout_secs value, or use web_fetch for targeted single-page reads."
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

fn normalize_crawl_key(raw_url: &str) -> Option<String> {
    let mut parsed = Url::parse(raw_url).ok()?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return None,
    }
    parsed.set_fragment(None);
    let mut normalized = parsed.to_string();
    let trim_slash = normalized.ends_with('/') && parsed.path() != "/";
    match trim_slash {
        true => {
            normalized.pop();
        }
        false => {}
    }
    Some(normalized)
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_page_summary_and_links(
    base_url: &Url,
    target_host: &str,
    html_str: &str,
    extractor: &ExtractionPipeline,
    extract_markdown: bool,
) -> (String, String, Option<String>, Vec<String>) {
    let doc = Html::parse_document(html_str);
    let title_selector = Selector::parse("title").ok();
    let body_selector = Selector::parse("body").ok();
    let link_selector = Selector::parse("a[href]").ok();

    let title = title_selector
        .as_ref()
        .and_then(|sel| doc.select(sel).next())
        .map(|el| collapse_whitespace(&el.text().collect::<Vec<_>>().join(" ")))
        .unwrap_or_default();

    let body_raw = body_selector
        .as_ref()
        .and_then(|sel| doc.select(sel).next())
        .map(|el| collapse_whitespace(&el.text().collect::<Vec<_>>().join(" ")))
        .unwrap_or_else(|| collapse_whitespace(html_str));

    let snippet = match body_raw.chars().count() > 300 {
        true => {
            let mut s: String = body_raw.chars().take(300).collect();
            s.push_str("...");
            s
        }
        false => body_raw,
    };

    let markdown = extract_markdown.then(|| {
        extractor
            .extract(base_url.as_str(), html_str, Some("text/html"))
            .map(|c| c.content_markdown)
            .unwrap_or_else(|_| snippet.clone())
    });

    let mut links = Vec::new();
    match link_selector {
        Some(sel) => {
            for el in doc.select(&sel) {
                match el.value().attr("href").and_then(|href| base_url.join(href).ok()) {
                    Some(resolved) => {
                        let same_host = resolved
                            .host_str()
                            .is_some_and(|h| h.eq_ignore_ascii_case(target_host));
                        match same_host {
                            true => match normalize_crawl_key(resolved.as_str()) {
                                Some(clean) => links.push(clean),
                                None => {}
                            },
                            false => {}
                        }
                    }
                    None => {}
                }
            }
        }
        None => {}
    }

    (title, snippet, markdown, links)
}

pub(crate) async fn execute_bfs_crawl(
    start_url: &str,
    crawler: &Crawler,
    extractor: &ExtractionPipeline,
    options: &CrawlOptions,
    pages_sink: Arc<Mutex<Vec<Value>>>,
) -> Result<()> {
    let base_parsed = Url::parse(start_url).map_err(|e| anyhow!("Invalid crawl URL: {e}"))?;
    let target_host = base_parsed
        .host_str()
        .ok_or_else(|| anyhow!("Crawl URL has no host"))?
        .to_ascii_lowercase();

    let robots = match options.respect_robots_txt {
        false => RobotsRules::allow_all(),
        true => {
            let robots_url = format!(
                "{}://{}{}/robots.txt",
                base_parsed.scheme(),
                target_host,
                base_parsed
                    .port()
                    .map(|p| format!(":{p}"))
                    .unwrap_or_default()
            );
            match crawler.fetch_url(&robots_url, false).await {
                Ok(res) => RobotsRules::parse(&res.body),
                Err(_) => RobotsRules::allow_all(),
            }
        }
    };

    let effective_delay_ms = options
        .delay_ms
        .max(robots.crawl_delay_ms.unwrap_or(0));

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    match normalize_crawl_key(start_url) {
        Some(seed) => {
            visited.insert(seed.clone());
            queue.push_back((seed, 0));
        }
        None => return Err(anyhow!("Unsupported crawl URL scheme")),
    }

    match options.use_sitemap {
        true => {
            match sitemap::discover_sitemap_urls(crawler, start_url).await {
                Ok(sitemap_urls) => {
                    for sm_url in sitemap_urls.into_iter().take(options.limit as usize) {
                        match normalize_crawl_key(&sm_url) {
                            Some(clean) => {
                                let same_host = Url::parse(&clean)
                                    .ok()
                                    .and_then(|u| u.host_str().map(|h| h.eq_ignore_ascii_case(&target_host)))
                                    .unwrap_or(false);
                                match same_host && visited.insert(clean.clone()) {
                                    true => queue.push_back((clean, 1)),
                                    false => {}
                                }
                            }
                            None => {}
                        }
                    }
                }
                Err(err) => {
                    tracing::debug!(url = %start_url, error = ?err, "Sitemap discovery returned no seeds; continuing with BFS");
                }
            }
        }
        false => {}
    }

    let mut first_request = true;
    while let Some((current_url, current_depth)) = queue.pop_front() {
        let collected_count = pages_sink.lock().await.len() as u32;
        match collected_count >= options.limit {
            true => break,
            false => {}
        }

        match options.respect_robots_txt && !robots.is_allowed(&current_url) {
            true => {
                tracing::debug!(url = %current_url, "Skipping URL disallowed by robots.txt");
                continue;
            }
            false => {}
        }

        match !first_request && effective_delay_ms > 0 {
            true => tokio::time::sleep(Duration::from_millis(effective_delay_ms)).await,
            false => {}
        }
        first_request = false;

        match crawler.fetch_url(&current_url, options.render_js).await {
            Ok(fetched) => {
                let effective_base = Url::parse(&fetched.final_url)
                    .or_else(|_| Url::parse(&current_url))
                    .unwrap_or_else(|_| base_parsed.clone());

                let (title, snippet, markdown, outgoing_links) = extract_page_summary_and_links(
                    &effective_base,
                    &target_host,
                    &fetched.body,
                    extractor,
                    options.extract_markdown,
                );

                let links_count = outgoing_links.len();
                match current_depth < options.depth {
                    true => {
                        for next_link in outgoing_links {
                            match (!options.respect_robots_txt || robots.is_allowed(&next_link))
                                && visited.insert(next_link.clone())
                            {
                                true => queue.push_back((next_link, current_depth + 1)),
                                false => {}
                            }
                        }
                    }
                    false => {}
                }

                let mut page_json = json!({
                    "url": fetched.final_url,
                    "status_code": 200u16,
                    "title": title,
                    "snippet": snippet,
                    "depth": current_depth,
                    "links_found": links_count,
                });
                match (page_json.as_object_mut(), markdown) {
                    (Some(map), Some(md)) => {
                        map.insert("markdown".to_string(), Value::String(md));
                    }
                    _ => {}
                }

                pages_sink.lock().await.push(page_json);
            }
            Err(err) => {
                tracing::debug!(url = %current_url, error = ?err, "Crawl page fetch failed; continuing");
            }
        }
    }

    Ok(())
}

#[async_trait::async_trait]
impl Tool for CrawlSiteTool {
    fn name(&self) -> &str {
        "crawl_website"
    }

    fn description(&self) -> &str {
        "Crawl a website starting from a URL (with optional sitemap.xml seeding and robots.txt politeness) and collect structured page information."
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
                    "description": "Maximum number of pages to fetch (default: 10, max: 1000)."
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum link depth to follow (default: 3, max: 10)."
                },
                "respect_robots_txt": {
                    "type": "boolean",
                    "description": "Whether to respect robots.txt Disallow and Crawl-delay rules (default: true)."
                },
                "use_sitemap": {
                    "type": "boolean",
                    "description": "Seed discovery from /sitemap.xml in addition to BFS link traversal (default: true when called via sitemap workflows, false otherwise)."
                },
                "extract_markdown": {
                    "type": "boolean",
                    "description": "Include full readability-extracted Markdown for each crawled page (default: false)."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Render pages through OpenZ's headless-first BrowserBroker (default: false)."
                },
                "delay": {
                    "type": "integer",
                    "description": "Politeness delay between requests in milliseconds (default: 250)."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Global crawl timeout in seconds (default: 45, min: 5, max: 300). Returns partial_success with pages collected so far on timeout."
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
        .clamp(1, 1000) as u32;
        let depth = get_u64_arg(arguments, &["depth", "max_depth", "maxDepth"], 3).min(10) as usize;
        let respect_robots_txt = get_bool_arg(
            arguments,
            &[
                "respect_robots_txt",
                "respectRobotsTxt",
                "respect_robots",
                "respectRobots",
            ],
            true,
        );
        let use_sitemap = get_bool_arg(arguments, &["use_sitemap", "useSitemap"], false);
        let extract_markdown = get_bool_arg(
            arguments,
            &["extract_markdown", "extractMarkdown", "markdown"],
            false,
        );
        let render_js = get_bool_arg(arguments, &["render_js", "renderJs"], false);
        let delay_ms = get_u64_arg(arguments, &["delay", "delay_ms", "delayMs"], 250).max(25);
        let timeout_secs = crawl_timeout_secs(arguments);

        let options = CrawlOptions {
            limit,
            depth,
            respect_robots_txt,
            use_sitemap,
            extract_markdown,
            render_js,
            delay_ms,
        };

        let server = crate::tools::searchxyz::get_server();
        let pages = Arc::new(Mutex::new(Vec::new()));
        let crawl_future = execute_bfs_crawl(
            url_str,
            &server.crawler,
            &server.extractor,
            &options,
            pages.clone(),
        );

        match tokio::time::timeout(Duration::from_secs(timeout_secs), crawl_future).await {
            Err(_) => {
                let results = pages.lock().await.clone();
                Ok(crawl_timeout_response(
                    results,
                    timeout_secs,
                    limit,
                    depth,
                    url_str,
                ))
            }
            Ok(outcome) => {
                let _ = outcome?;
                let results = pages.lock().await.clone();
                match results.is_empty() {
                    true => Err(anyhow!(
                        "Crawl returned no results. The site may be unreachable, disallowed by robots.txt, or blocking automated access."
                    )),
                    false => {
                        for page in &results {
                            let page_url =
                                page.get("url").and_then(Value::as_str).unwrap_or(url_str);
                            let title =
                                page.get("title").and_then(Value::as_str).unwrap_or(page_url);
                            let body = page
                                .get("markdown")
                                .and_then(Value::as_str)
                                .or_else(|| page.get("snippet").and_then(Value::as_str))
                                .unwrap_or("");
                            let _ =
                                crate::tools::shared_memory::archive_research_entry_with_meta(
                                    page_url,
                                    title,
                                    body,
                                    "crawl_site",
                                    Some(page_url),
                                )
                                .await;
                        }
                        Ok(Value::Array(results))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "crawl_tests.rs"]
mod tests;
