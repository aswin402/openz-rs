use super::get_server;
use crate::tools::searchxyz::server::{
    DeepResearchRequest, FetchCacheMode, ReadUrlRequest, SaveMode,
};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub use super::core::search::cooldown::{
    active_search_backend_cooldowns, classify_search_failure, record_search_backend_failure,
    search_failure_cooldown_secs, search_failure_error_value, SearchFailureKind,
};
#[cfg(test)]
pub use super::core::search::cooldown::{
    clear_search_backend_cooldowns_for_tests, COOLDOWN_TEST_LOCK,
};
pub use super::doctor_report::{
    backend_health_label, build_searchxyz_doctor_report, build_web_engine_health_json,
    only_scraper_backends_enabled,
};

fn encode_browser_query(query: &str) -> String {
    percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC)
        .to_string()
        .replace("%20", "+")
}

pub fn browser_search_url(engine: &str, query: &str) -> Result<String> {
    let encoded = encode_browser_query(query);
    match engine {
        "duckduckgo" | "ddg" => Ok(format!("https://duckduckgo.com/html/?q={encoded}")),
        "bing" => Ok(format!("https://www.bing.com/search?q={encoded}")),
        "mojeek" => Ok(format!("https://www.mojeek.com/search?q={encoded}")),
        other => Err(anyhow!(
            "Unsupported browser search engine: {other}. Use duckduckgo, bing, or mojeek."
        )),
    }
}

fn is_http_or_https(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn normalize_browser_result_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    match trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("javascript:")
        || trimmed.starts_with("mailto:")
    {
        true => return None,
        false => {}
    }

    match trimmed
        .strip_prefix("/l/?uddg=")
        .or_else(|| trimmed.strip_prefix("https://duckduckgo.com/l/?uddg="))
    {
        Some(encoded) => {
            let raw_target = encoded.split('&').next().unwrap_or(encoded);
            let decoded = percent_encoding::percent_decode_str(raw_target)
                .decode_utf8_lossy()
                .to_string();
            match is_http_or_https(&decoded) {
                true => return Some(decoded),
                false => {}
            }
        }
        None => {}
    }

    is_http_or_https(trimmed).then(|| trimmed.to_string())
}

fn is_likely_search_engine_internal_url(url: &str) -> bool {
    let lowered = url.to_lowercase();
    [
        "duckduckgo.com",
        "bing.com/search",
        "bing.com/images",
        "go.microsoft.com",
        "microsoft.com/rewards",
        "privacy.microsoft.com",
        "mojeek.com/search",
        "mojeek.com/about",
        "mojeek.com/preferences",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn browser_result_selectors(engine: &str) -> Vec<&'static str> {
    match engine {
        "bing" => vec![
            "li.b_algo h2 a[href]",
            "#b_results h2 a[href]",
            "#b_results a[href]",
            "main a[href]",
        ],
        "mojeek" => vec![
            "ul.results-standard a.title[href]",
            "a.title[href]",
            "main a[href]",
        ],
        _ => vec![
            "a.result__a[href]",
            "a.result-link[href]",
            "a[data-testid='result-title-a'][href]",
            "article a[href]",
            "main a[href]",
        ],
    }
}

fn browser_result_extraction_script(engine: &str, max_results: usize) -> String {
    let selectors =
        serde_json::to_string(&browser_result_selectors(engine)).unwrap_or_else(|_| "[]".into());
    format!(
        r#"(() => {{
  const selectors = {selectors};
  const maxResults = {max_results};
  const cleanText = (value) => (value || "").replace(/\s+/g, " ").trim();
  const collectAnchors = () => {{
    const anchors = [];
    for (const selector of selectors) {{
      anchors.push(...Array.from(document.querySelectorAll(selector)));
    }}
    return anchors.filter((anchor) => anchor && anchor.href && cleanText(anchor.innerText || anchor.textContent));
  }};

  const anchors = collectAnchors();
  const seen = new Set();
  const results = [];
  for (const anchor of anchors) {{
    const url = anchor.href;
    if (seen.has(url)) continue;
    seen.add(url);
    results.push({{
      title: cleanText(anchor.innerText || anchor.textContent),
      url
    }});
    if (results.length >= maxResults) break;
  }}

  return JSON.stringify({{
    results,
    rendered_link_count: anchors.length,
    page_url: location.href,
    page_title: document.title,
    body_hint: cleanText(document.body ? document.body.innerText : "").slice(0, 600)
  }});
}})()"#
    )
}

fn rendered_payload_value(payload: &str) -> Option<Value> {
    let trimmed = payload.trim();
    match trimmed.is_empty() {
        true => return None,
        false => {}
    }
    let parsed: Value = serde_json::from_str(trimmed).ok()?;
    match parsed {
        Value::String(inner) => serde_json::from_str(&inner).ok(),
        other => Some(other),
    }
}

pub fn extract_rendered_browser_search_results(
    engine: &str,
    payload: &str,
    max_results: usize,
) -> Vec<Value> {
    let raw_results = match rendered_payload_value(payload)
        .and_then(|value| value.get("results").and_then(|v| v.as_array()).cloned())
    {
        Some(items) => items,
        None => return Vec::new(),
    };
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for result in raw_results {
        let url = match result
            .get("url")
            .and_then(|value| value.as_str())
            .and_then(normalize_browser_result_url)
        {
            Some(u) => u,
            None => continue,
        };
        match is_likely_search_engine_internal_url(&url) || !seen.insert(url.clone()) {
            true => continue,
            false => {}
        }
        let title = result
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        match title.is_empty() {
            true => continue,
            false => {}
        }
        results.push(json!({
            "title": title,
            "url": url,
            "engine": engine,
            "source": "browser_search_rendered",
        }));
        match results.len() >= max_results {
            true => break,
            false => {}
        }
    }

    results
}

fn append_unique_browser_results(
    target: &mut Vec<Value>,
    candidates: Vec<Value>,
    max_results: usize,
) {
    let mut seen = target
        .iter()
        .filter_map(|result| result.get("url").and_then(|value| value.as_str()))
        .map(str::to_string)
        .collect::<std::collections::HashSet<_>>();

    for candidate in candidates {
        let url = match candidate.get("url").and_then(|value| value.as_str()) {
            Some(u) => u,
            None => continue,
        };
        match seen.insert(url.to_string()) {
            false => continue,
            true => {}
        }
        target.push(candidate);
        match target.len() >= max_results {
            true => break,
            false => {}
        }
    }
}

fn text_for_anchor(anchor: scraper::element_ref::ElementRef<'_>) -> String {
    anchor
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn extract_browser_search_results(engine: &str, html: &str, max_results: usize) -> Vec<Value> {
    let doc = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("a[href]").expect("valid anchor selector");
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for anchor in doc.select(&selector) {
        let url = match anchor
            .value()
            .attr("href")
            .and_then(normalize_browser_result_url)
        {
            Some(u) => u,
            None => continue,
        };
        match is_likely_search_engine_internal_url(&url) || !seen.insert(url.clone()) {
            true => continue,
            false => {}
        }
        let title = text_for_anchor(anchor);
        results.push(json!({
            "title": title,
            "url": url,
            "engine": engine,
            "source": "browser_search",
        }));
        match results.len() >= max_results {
            true => break,
            false => {}
        }
    }

    results
}

fn html_has_bot_block(html: &str) -> bool {
    let lowered = html.to_lowercase();
    [
        "captcha",
        "unusual traffic",
        "verify you are human",
        "checking your browser",
        "access denied",
        "temporarily blocked",
        "bots use duckduckgo",
        "please complete the following challenge",
        "anomaly-modal",
        "challenge-form",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn rendered_payload_has_bot_block(payload: &str) -> bool {
    rendered_payload_value(payload).is_some_and(|val| {
        let title = val.get("page_title").and_then(Value::as_str).unwrap_or("");
        let hint = val.get("body_hint").and_then(Value::as_str).unwrap_or("");
        html_has_bot_block(title) || html_has_bot_block(hint)
    })
}

pub(crate) fn filter_relevant_browser_results(query: &str, results: Vec<Value>) -> (Vec<Value>, usize) {
    let stop_words = [
        "the", "and", "for", "with", "from", "what", "how", "why", "who", "are", "this", "that",
    ];
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .map(|tok| tok.trim().to_ascii_lowercase())
        .filter(|tok| tok.chars().count() >= 3 && !stop_words.contains(&tok.as_str()))
        .collect();

    match terms.is_empty() || results.is_empty() {
        true => (results, 0),
        false => {
            let total = results.len();
            let filtered: Vec<Value> = results
                .into_iter()
                .filter(|item| {
                    let title = item
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let url = item
                        .get("url")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    terms
                        .iter()
                        .any(|term| title.contains(term) || url.contains(term))
                })
                .collect();
            let dropped = total.saturating_sub(filtered.len());
            (filtered, dropped)
        }
    }
}

pub(crate) fn browser_search_read_top_results_enabled(arguments: &Value) -> bool {
    arguments
        .get("read_top_results")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

pub(crate) fn browser_search_max_pages(arguments: &Value, max_results: usize) -> usize {
    let default_pages = max_results.clamp(1, 3);
    arguments
        .get("max_pages")
        .and_then(|value| value.as_u64())
        .map(|value| value.clamp(1, 5) as usize)
        .unwrap_or(default_pages)
}

async fn read_browser_search_results(
    results: &[Value],
    arguments: &Value,
    max_results: usize,
) -> Vec<Value> {
    let max_pages = browser_search_max_pages(arguments, max_results);
    let save_mode = match arguments
        .get("save_mode")
        .and_then(|value| value.as_str())
        .unwrap_or("none")
    {
        "full" => SaveMode::Full,
        _ => SaveMode::None,
    };
    let server = get_server();
    let mut reads = Vec::new();

    for result in results.iter().take(max_pages) {
        let url = match result.get("url").and_then(|value| value.as_str()) {
            Some(u) => u,
            None => continue,
        };
        match server
            .read_url(ReadUrlRequest {
                url: url.to_string(),
                depth: Some(1),
                render_js: Some(false),
                cache_mode: Some(FetchCacheMode::Auto),
                save_mode: Some(save_mode),
                include_diagnostics: Some(false),
                max_chars: Some(6000),
            })
            .await
        {
            Ok(content) => reads.push(json!({
                "status": "success",
                "url": url,
                "content": content,
            })),
            Err(err) => reads.push(json!({
                "status": "error",
                "url": url,
                "error": err.to_string(),
            })),
        }
    }

    reads
}

fn apply_common_crawl_aliases(normalized: &mut Value) {
    super::map_field_alias(
        normalized,
        "max_chars",
        &["maxChars", "limit_chars", "max_length", "maxLength"],
    );
    super::map_field_alias(normalized, "merge_backends", &["mergeBackends", "merge"]);
    super::map_field_alias(normalized, "render_js", &["renderJs", "js"]);
    super::map_field_alias(
        normalized,
        "include_diagnostics",
        &["includeDiagnostics", "diagnostics"],
    );
    super::map_field_alias(normalized, "cache_mode", &["cacheMode"]);
    super::map_field_alias(normalized, "save_mode", &["saveMode"]);
    super::map_field_alias(
        normalized,
        "include_domains",
        &["includeDomains", "domains"],
    );
    super::map_field_alias(normalized, "exclude_domains", &["excludeDomains"]);
}

// ── 1. Browser Search Fallback ────────────────────────────────
pub struct SearchXyzBrowserSearchTool;

#[async_trait::async_trait]
impl Tool for SearchXyzBrowserSearchTool {
    fn name(&self) -> &str {
        "searchxyz_browser_search"
    }

    fn description(&self) -> &str {
        "Search the web through the headless-first browser broker when keyless/API search backends are blocked. Tries Obscura/Chrome CDP and Firefox headless before any GUI fallback; no Brave or SearXNG dependency required."
    }

    fn metadata(&self) -> crate::tools::ToolMetadata {
        let mut m = crate::tools::ToolMetadata::infer(self.name());
        m.domain = "web";
        m.risk = crate::tools::ToolRisk::Medium;
        m.uses_network = true;
        m.spawns_process = true;
        m
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string."
                },
                "engine": {
                    "type": "string",
                    "enum": ["duckduckgo", "bing", "mojeek"],
                    "description": "Browser search engine to use (default: duckduckgo; mojeek is bot-friendly)."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum result links to return (default: 5, max: 20)."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Maximum time to wait for rendered result anchors before falling back to page source (default: 8, max: 60)."
                },
                "read_top_results": {
                    "type": "boolean",
                    "description": "After browser discovery, fetch the top result pages into Markdown (default: false)."
                },
                "max_pages": {
                    "type": "integer",
                    "description": "Maximum discovered pages to read when read_top_results=true (default: min(max_results, 3), max: 5)."
                },
                "save_mode": {
                    "type": "string",
                    "enum": ["full", "none"],
                    "description": "Whether fetched top-result pages should be persisted into unified SQLite research memory (default: none)."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let mut normalized = super::normalize_query_arg(arguments);
        super::map_field_alias(
            &mut normalized,
            "max_results",
            &["limit", "count", "maxResults"],
        );
        super::map_field_alias(&mut normalized, "timeout_secs", &["timeout", "timeoutSecs"]);
        super::map_field_alias(
            &mut normalized,
            "read_top_results",
            &["readTopResults", "read_results", "fetch_top_results"],
        );
        super::map_field_alias(&mut normalized, "max_pages", &["maxPages", "pages"]);
        super::map_field_alias(&mut normalized, "save_mode", &["saveMode"]);
        super::coerce_bool_fields(&mut normalized, &["read_top_results"]);
        super::coerce_numeric_fields(
            &mut normalized,
            &["max_results", "timeout_secs", "max_pages"],
        );
        let query = normalized
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;
        let engine = normalized
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or("duckduckgo");
        let max_results = normalized
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .clamp(1, 20) as usize;
        let timeout_secs = normalized
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(8)
            .clamp(1, 60);

        let search_url = browser_search_url(engine, query)?;
        let render_script = browser_result_extraction_script(engine, max_results);
        let broker_eval = crate::tools::browser_broker::eval_with_browser_broker(
            &search_url,
            &render_script,
            timeout_secs,
        )
        .await;

        let (mut results, rendered_bot_blocked, backend, cleanup, fallbacks_tried, broker_errors) =
            match broker_eval {
                Ok(value) => {
                    let bot_blocked = rendered_payload_has_bot_block(&value.output);
                    let rendered =
                        extract_rendered_browser_search_results(engine, &value.output, max_results);
                    (
                        rendered,
                        bot_blocked,
                        Some(value.backend.as_str().to_string()),
                        Some(value.cleanup),
                        value
                            .fallbacks_tried
                            .iter()
                            .map(|backend| backend.as_str().to_string())
                            .collect::<Vec<_>>(),
                        value.errors,
                    )
                }
                Err(err) => {
                    return Ok(json!({
                        "status": "browser_error",
                        "error_kind": "browser_broker_failed",
                        "retryable": true,
                        "engine": engine,
                        "query": query,
                        "error": err.to_string(),
                        "fallbacks_tried": ["obscura_headless", "firefox_headless", "gsd_chrome_gui"],
                        "next_step": "Run inspect_browsers, use manage_tasks action=cleanup, retry with engine=mojeek, or use native web_search / web_fetch."
                    }));
                }
            };
        let rendered_result_count = results.len();

        let html = match results.is_empty() && !rendered_bot_blocked {
            true => match crate::tools::browser_broker::render_with_browser_broker(
                &search_url,
                timeout_secs,
            )
            .await
            {
                Ok(value) => value.output,
                Err(err) => {
                    tracing::warn!(query = %query, engine = %engine, error = ?err, "browser broker page-source fallback failed");
                    String::new()
                }
            },
            false => String::new(),
        };

        let static_results = extract_browser_search_results(engine, &html, max_results);
        let static_result_count = static_results.len();
        append_unique_browser_results(&mut results, static_results, max_results);
        match results.is_empty() && (rendered_bot_blocked || html_has_bot_block(&html)) {
            true => {
                return Ok(json!({
                    "status": "blocked",
                    "error_kind": "captcha_or_bot_block",
                    "retryable": true,
                    "engine": engine,
                    "query": query,
                    "result_count": 0,
                    "results": [],
                    "backend": backend,
                    "cleanup": cleanup,
                    "fallbacks_tried": fallbacks_tried,
                    "broker_errors": broker_errors,
                    "next_step": "Use native web_search, retry with engine=mojeek, use web_fetch on a known URL, or query search_research."
                }));
            }
            false => {}
        }

        let (filtered_results, dropped_irrelevant) =
            filter_relevant_browser_results(query, results);
        match filtered_results.is_empty() && dropped_irrelevant > 0 {
            true => {
                return Ok(json!({
                    "status": "degraded",
                    "error_kind": "irrelevant_serp",
                    "retryable": true,
                    "engine": engine,
                    "query": query,
                    "result_count": 0,
                    "dropped_irrelevant_count": dropped_irrelevant,
                    "results": [],
                    "backend": backend,
                    "cleanup": cleanup,
                    "fallbacks_tried": fallbacks_tried,
                    "broker_errors": broker_errors,
                    "next_step": "Browser SERP returned results unrelated to query tokens; use native web_search, retry with engine=mojeek, or use web_fetch on a known URL."
                }));
            }
            false => {}
        }
        let results = filtered_results;

        let read_top_enabled = browser_search_read_top_results_enabled(&normalized);
        let read_results = match read_top_enabled {
            true => read_browser_search_results(&results, &normalized, max_results).await,
            false => Vec::new(),
        };
        let read_errors = read_results
            .iter()
            .filter(|entry| entry["status"] == "error")
            .count();
        let status = match (results.is_empty(), read_top_enabled && read_errors > 0) {
            (true, _) => "no_results",
            (false, true) => "partial_success",
            (false, false) => "success",
        };
        let extraction_strategy = match (rendered_result_count > 0, static_result_count > 0) {
            (true, _) => "rendered_dom",
            (false, true) => "page_source",
            (false, false) => "empty",
        };

        Ok(json!({
            "status": status,
            "engine": engine,
            "query": query,
            "result_count": results.len(),
            "rendered_result_count": rendered_result_count,
            "static_result_count": static_result_count,
            "extraction_strategy": extraction_strategy,
            "backend": backend,
            "cleanup": cleanup,
            "fallbacks_tried": fallbacks_tried,
            "broker_errors": broker_errors,
            "results": results,
            "read_count": read_results.len(),
            "read_results": read_results,
        }))
    }
}

// ── 2. Deep Research ──────────────────────────────────────────
pub struct SearchXyzDeepResearchTool;

#[async_trait::async_trait]
impl Tool for SearchXyzDeepResearchTool {
    fn name(&self) -> &str {
        "searchxyz_deep_research"
    }

    fn description(&self) -> &str {
        "Perform iterative multi-query web crawls, claim synthesis, and hybrid SQLite indexing to compile a deep research Markdown dossier."
    }

    fn metadata(&self) -> crate::tools::ToolMetadata {
        let mut m = crate::tools::ToolMetadata::infer(self.name());
        m.domain = "web";
        m.risk = crate::tools::ToolRisk::Medium;
        m.uses_network = true;
        m.writes_disk = true;
        m
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The research query or topic."
                },
                "breadth": {
                    "type": "integer",
                    "description": "Number of expanded sub-queries to execute (default: 3, max: 5)."
                },
                "max_pages_per_query": {
                    "type": "integer",
                    "description": "How many top pages to crawl per sub-query (default: 2, max: 4)."
                },
                "include_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only keep search results from these domains before crawling."
                },
                "exclude_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Drop search results from these domains before crawling."
                },
                "merge_backends": {
                    "type": "boolean",
                    "description": "Query all available backends concurrently and deduplicate results before crawling."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Enable headless JS rendering via OpenZ BrowserBroker (default: false)."
                },
                "cache_mode": {
                    "type": "string",
                    "enum": ["auto", "prefer_cache", "revalidate", "bypass"],
                    "description": "HTTP cache behavior backed by unified SQLite web_fetch_cache."
                },
                "save_mode": {
                    "type": "string",
                    "enum": ["full", "none"],
                    "description": "Whether to persist fetched content into unified SQLite memory.db and graph_memory.db."
                },
                "include_diagnostics": {
                    "type": "boolean",
                    "description": "Append compact diagnostics to the output."
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Optional output character budget. Truncates large research reports with metadata when exceeded."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let mut normalized = super::normalize_query_arg(arguments);
        super::map_field_alias(
            &mut normalized,
            "breadth",
            &["depth", "max_depth", "max_breadth", "maxBreadth"],
        );
        super::map_field_alias(
            &mut normalized,
            "max_pages_per_query",
            &["maxPagesPerQuery", "max_pages", "pages_per_query", "limit"],
        );
        apply_common_crawl_aliases(&mut normalized);
        super::coerce_numeric_fields(
            &mut normalized,
            &["breadth", "max_pages_per_query", "max_chars"],
        );
        super::coerce_bool_fields(
            &mut normalized,
            &["merge_backends", "render_js", "include_diagnostics"],
        );
        let req: DeepResearchRequest = serde_json::from_value(normalized)?;
        let res = get_server().deep_research(req).await?;
        Ok(json!(res))
    }
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
