use super::{get_server, map_mcp_err};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rmcp::handler::server::wrapper::Parameters;
use searchxyz::tools::{
    DeepResearchRequest, ReadUrlRequest, SearchAndReadRequest, SearchWebRequest, SiteMapRequest,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFailureKind {
    RateLimited,
    Blocked,
    Timeout,
    NoUsableResults,
    Other,
}

impl SearchFailureKind {
    fn as_str(self) -> &'static str {
        match self {
            SearchFailureKind::RateLimited => "rate_limited",
            SearchFailureKind::Blocked => "blocked",
            SearchFailureKind::Timeout => "timeout",
            SearchFailureKind::NoUsableResults => "no_usable_results",
            SearchFailureKind::Other => "other",
        }
    }
}

pub fn classify_search_failure(message: &str) -> SearchFailureKind {
    let normalized = message.to_lowercase();
    if normalized.contains("429")
        || normalized.contains("too many requests")
        || normalized.contains("rate limit")
    {
        return SearchFailureKind::RateLimited;
    }
    if normalized.contains("403")
        || normalized.contains("forbidden")
        || normalized.contains("captcha")
        || normalized.contains("blocked")
        || normalized.contains("unusual traffic")
    {
        return SearchFailureKind::Blocked;
    }
    if normalized.contains("timed out") || normalized.contains("timeout") {
        return SearchFailureKind::Timeout;
    }
    if normalized.contains("0 usable results")
        || normalized.contains("all search backends exhausted")
    {
        return SearchFailureKind::NoUsableResults;
    }
    SearchFailureKind::Other
}

pub fn search_failure_cooldown_secs(kind: SearchFailureKind) -> u64 {
    match kind {
        SearchFailureKind::RateLimited => 1800,
        SearchFailureKind::Blocked => 3600,
        SearchFailureKind::Timeout => 300,
        SearchFailureKind::NoUsableResults => 600,
        SearchFailureKind::Other => 120,
    }
}

#[derive(Debug, Clone)]
struct BackendCooldown {
    kind: SearchFailureKind,
    until: Instant,
}

static SEARCH_BACKEND_COOLDOWNS: OnceLock<Mutex<HashMap<String, BackendCooldown>>> =
    OnceLock::new();

fn search_backend_cooldowns() -> &'static Mutex<HashMap<String, BackendCooldown>> {
    SEARCH_BACKEND_COOLDOWNS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn record_search_backend_failure(backends: &[String], kind: SearchFailureKind) {
    if backends.is_empty() {
        return;
    }

    let until = Instant::now() + std::time::Duration::from_secs(search_failure_cooldown_secs(kind));
    let mut cooldowns = search_backend_cooldowns()
        .lock()
        .expect("search backend cooldown lock poisoned");
    for backend in backends {
        if backend.trim().is_empty() {
            continue;
        }
        cooldowns.insert(backend.clone(), BackendCooldown { kind, until });
    }
}

pub fn active_search_backend_cooldowns() -> Vec<Value> {
    let now = Instant::now();
    let mut cooldowns = search_backend_cooldowns()
        .lock()
        .expect("search backend cooldown lock poisoned");
    cooldowns.retain(|_, cooldown| cooldown.until > now);

    let mut active = cooldowns
        .iter()
        .map(|(backend, cooldown)| {
            json!({
                "backend": backend,
                "error_kind": cooldown.kind.as_str(),
                "cooldown_remaining_secs": cooldown.until.saturating_duration_since(now).as_secs().max(1),
            })
        })
        .collect::<Vec<_>>();
    active.sort_by(|a, b| {
        a["backend"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["backend"].as_str().unwrap_or_default())
    });
    active
}

#[cfg(test)]
fn clear_search_backend_cooldowns_for_tests() {
    search_backend_cooldowns()
        .lock()
        .expect("search backend cooldown lock poisoned")
        .clear();
}

fn search_failure_error_value(
    raw_error: &str,
    kind: SearchFailureKind,
    backends: &[String],
) -> Value {
    json!({
        "status": "search_failed",
        "error_kind": kind.as_str(),
        "retryable": true,
        "cooldown_secs": search_failure_cooldown_secs(kind),
        "affected_backends": backends,
        "raw_error": raw_error,
        "next_step": "Use searchxyz_browser_search for provider-free browser discovery, searchxyz_recall for local indexed content, or provide a direct URL for searchxyz_read_url."
    })
}

fn backend_health_label(
    name: &str,
    configured_order: &[String],
    config: &searchxyz::config::Config,
) -> String {
    let enabled = configured_order.iter().any(|backend| backend == name);
    let preferred = configured_order
        .first()
        .is_some_and(|backend| backend == name);
    if !enabled {
        return "disabled".to_string();
    }

    let base = match name {
        "searxng" => {
            if config.searxng.instance_url.trim().is_empty() {
                "missing_config"
            } else {
                "configured"
            }
        }
        "brave" => {
            if config
                .brave
                .api_key
                .as_ref()
                .is_some_and(|key| !key.trim().is_empty())
            {
                "configured"
            } else {
                "missing_key"
            }
        }
        "duckduckgo" | "google" | "bing" => "keyless",
        _ => "configured",
    };

    if preferred {
        format!("preferred ({base})")
    } else {
        base.to_string()
    }
}

fn only_scraper_backends_enabled(configured_order: &[String]) -> bool {
    !configured_order.is_empty()
        && configured_order
            .iter()
            .all(|backend| matches!(backend.as_str(), "duckduckgo" | "google" | "bing"))
}

pub async fn build_searchxyz_doctor_report(include_paths: bool) -> Result<String> {
    let server = get_server();
    let config = server.config.as_ref();
    let cache_len = server.cache.lock().await.len();
    let (_, source_count) = server.index.list_documents(None, 1, 0)?;
    let graph = server.graph.lock().await;
    let graph_nodes = graph.nodes.len();
    let graph_edges = graph.edges.len();
    drop(graph);

    let native_policy = std::env::var("OPENZ_WEB_SEARCH_POLICY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "native_only".to_string());

    let known_backends = ["searxng", "brave", "duckduckgo", "google", "bing"];
    let headless_status = if config.headless.enabled {
        match &config.headless.chrome_path {
            Some(path) => format!("enabled ({path})"),
            None => "enabled (auto chrome path)".to_string(),
        }
    } else {
        "disabled".to_string()
    };

    let mut report = String::new();
    report.push_str("# SearchXyz Doctor\n\n");
    report.push_str("## Policy\n");
    report.push_str(&format!("- OpenZ web_search policy: `{}`\n", native_policy));
    report.push_str("- Default behavior: native SearchXyz only unless `search_policy=native_then_external`, `external_only`, or the matching `OPENZ_WEB_SEARCH_POLICY` value is set. The default local stack includes browser discovery.\n\n");

    report.push_str("## Search Backends\n");
    report.push_str(&format!(
        "- Configured order: `{}`\n",
        config.search.backends.join(", ")
    ));
    report.push_str(&format!("- Max results: {}\n", config.search.max_results));
    report.push_str("- Ranking: enabled (dynamic term rarity, coverage, phrase/entity matches, domain bonuses, generic-page and low-quality URL penalties)\n");
    for backend in known_backends {
        report.push_str(&format!(
            "- {}: {}\n",
            backend,
            backend_health_label(backend, &config.search.backends, config)
        ));
    }
    report.push_str("- Labels: preferred, configured, missing_key, disabled, keyless.\n\n");

    report.push_str("## Fetch & Safety\n");
    report.push_str(&format!("- Headless rendering: {}\n", headless_status));
    report.push_str(&format!(
        "- Private network fetches allowed: {}\n",
        config.crawler.allow_private_network
    ));
    report.push_str(&format!(
        "- Request timeout: {}s\n",
        config.crawler.timeout_secs
    ));
    report.push_str(&format!(
        "- Max redirects: {}\n",
        config.crawler.max_redirects
    ));
    report.push_str(&format!(
        "- Max body bytes: {}\n\n",
        config.crawler.max_body_bytes
    ));

    report.push_str("## Local State\n");
    report.push_str(&format!("- Cache entries loaded: {}\n", cache_len));
    report.push_str(&format!("- Indexed sources: {}\n", source_count));
    report.push_str(&format!("- Graph nodes: {}\n", graph_nodes));
    report.push_str(&format!("- Graph edges: {}\n", graph_edges));
    if include_paths {
        report.push_str(&format!(
            "- Index path: `{}`\n",
            config.index.path.display()
        ));
        report.push_str(&format!(
            "- Cache path: `{}`\n",
            config.cache.path.display()
        ));
        report.push_str(&format!(
            "- Graph path: `{}`\n",
            config.index.path.join("graph.json").display()
        ));
    }

    let active_cooldowns = active_search_backend_cooldowns();
    if !active_cooldowns.is_empty() {
        report.push_str("\n## Backend Cooldowns\n");
        for cooldown in active_cooldowns {
            let backend = cooldown["backend"].as_str().unwrap_or("unknown");
            let error_kind = cooldown["error_kind"].as_str().unwrap_or("unknown");
            let remaining = cooldown["cooldown_remaining_secs"].as_u64().unwrap_or(0);
            report.push_str(&format!(
                "- {}: {} ({}s remaining)\n",
                backend, error_kind, remaining
            ));
        }
    }

    report.push_str("\n## Hints\n");
    if !config
        .search
        .backends
        .iter()
        .any(|backend| backend == "searxng")
    {
        report.push_str("- SearXNG is not enabled in backend order. Configure `SEARCHXYZ_SEARXNG_URL` without explicit backend order, or include `searxng` in `SEARCHXYZ_SEARCH_BACKENDS`, for the strongest private/native discovery path.\n");
    }
    if config
        .search
        .backends
        .iter()
        .any(|backend| backend == "brave")
        && config
            .brave
            .api_key
            .as_ref()
            .is_none_or(|key| key.trim().is_empty())
    {
        report.push_str("- Brave is enabled but missing `SEARCHXYZ_BRAVE_API_KEY`; it will be skipped at runtime.\n");
    }
    if only_scraper_backends_enabled(&config.search.backends) {
        report.push_str("- Only keyless scraper backends are enabled; these can be blocked or return empty results under anti-bot pressure. Prefer SearXNG for native discovery.\n");
    }
    if !config.headless.enabled {
        report.push_str("- Enable SearchXyz headless mode when scraper backends are blocked by JS or anti-bot pages.\n");
    }
    report.push_str("- Provider-free browser fallback is automatic in default `web_search`; call `searchxyz_browser_search` directly only for explicit engine/debug control.\n");
    report.push_str("- For one-off fallback to external engines, call `web_search` with `search_policy=native_then_external`.\n");

    Ok(report)
}

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
        other => Err(anyhow!(
            "Unsupported browser search engine: {other}. Use duckduckgo or bing."
        )),
    }
}

fn normalize_browser_result_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("javascript:")
        || trimmed.starts_with("mailto:")
    {
        return None;
    }

    if let Some(encoded) = trimmed
        .strip_prefix("/l/?uddg=")
        .or_else(|| trimmed.strip_prefix("https://duckduckgo.com/l/?uddg="))
    {
        let raw_target = encoded.split('&').next().unwrap_or(encoded);
        let decoded = percent_encoding::percent_decode_str(raw_target)
            .decode_utf8_lossy()
            .to_string();
        if decoded.starts_with("http://") || decoded.starts_with("https://") {
            return Some(decoded);
        }
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }

    None
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
    page_title: document.title
  }});
}})()"#
    )
}

fn rendered_payload_value(payload: &str) -> Option<Value> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed: Value = serde_json::from_str(trimmed).ok()?;
    match parsed {
        Value::String(inner) => serde_json::from_str(&inner).ok(),
        other => Some(other),
    }
}

fn extract_rendered_browser_search_results(
    engine: &str,
    payload: &str,
    max_results: usize,
) -> Vec<Value> {
    let Some(value) = rendered_payload_value(payload) else {
        return Vec::new();
    };
    let Some(raw_results) = value.get("results").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for result in raw_results {
        let Some(raw_url) = result.get("url").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(url) = normalize_browser_result_url(raw_url) else {
            continue;
        };
        if is_likely_search_engine_internal_url(&url) || !seen.insert(url.clone()) {
            continue;
        }
        let title = result
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if title.is_empty() {
            continue;
        }
        results.push(json!({
            "title": title,
            "url": url,
            "engine": engine,
            "source": "browser_search_rendered",
        }));
        if results.len() >= max_results {
            break;
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
        let Some(url) = candidate.get("url").and_then(|value| value.as_str()) else {
            continue;
        };
        if !seen.insert(url.to_string()) {
            continue;
        }
        target.push(candidate);
        if target.len() >= max_results {
            break;
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
        let Some(raw_href) = anchor.value().attr("href") else {
            continue;
        };
        let Some(url) = normalize_browser_result_url(raw_href) else {
            continue;
        };
        if is_likely_search_engine_internal_url(&url) || !seen.insert(url.clone()) {
            continue;
        }
        let title = text_for_anchor(anchor);
        results.push(json!({
            "title": title,
            "url": url,
            "engine": engine,
            "source": "browser_search",
        }));
        if results.len() >= max_results {
            break;
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
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn browser_search_read_top_results_enabled(arguments: &Value) -> bool {
    arguments
        .get("read_top_results")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn browser_search_max_pages(arguments: &Value, max_results: usize) -> usize {
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
    let save_mode = arguments
        .get("save_mode")
        .and_then(|value| value.as_str())
        .unwrap_or("none");
    let reader = SearchXyzReadUrlTool;
    let mut reads = Vec::new();

    for result in results.iter().take(max_pages) {
        let Some(url) = result.get("url").and_then(|value| value.as_str()) else {
            continue;
        };
        match reader
            .call(&json!({
                "url": url,
                "cache_mode": "auto",
                "save_mode": save_mode,
                "max_chars": 6000,
            }))
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

// ── Browser Search Fallback ───────────────────────────────────
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
                    "enum": ["duckduckgo", "bing"],
                    "description": "Browser search engine to use (default: duckduckgo)."
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
                    "description": "After browser discovery, fetch the top result pages through searchxyz_read_url (default: false)."
                },
                "max_pages": {
                    "type": "integer",
                    "description": "Maximum discovered pages to read when read_top_results=true (default: min(max_results, 3), max: 5)."
                },
                "save_mode": {
                    "type": "string",
                    "enum": ["full", "none"],
                    "description": "Whether fetched top-result pages should be persisted into SearchXyz memory (default: none)."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;
        let engine = arguments
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or("duckduckgo");
        let max_results = arguments
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .clamp(1, 20) as usize;
        let timeout_secs = arguments
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

        let (mut results, backend, cleanup, fallbacks_tried, broker_errors) = match broker_eval {
            Ok(value) => {
                let rendered =
                    extract_rendered_browser_search_results(engine, &value.output, max_results);
                (
                    rendered,
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
                    "next_step": "Run inspect_browsers, use manage_tasks action=cleanup, retry with engine=bing, or use searchxyz_recall/direct URL readers."
                }));
            }
        };
        let rendered_result_count = results.len();

        let html = if results.is_empty() {
            match crate::tools::browser_broker::render_with_browser_broker(
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
            }
        } else {
            String::new()
        };

        let static_results = extract_browser_search_results(engine, &html, max_results);
        let static_result_count = static_results.len();
        append_unique_browser_results(&mut results, static_results, max_results);
        if results.is_empty() && html_has_bot_block(&html) {
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
                "next_step": "Retry later, try engine=bing, use a known URL, or build the local SearchXyz index from trusted seed sources."
            }));
        }

        let read_results = if browser_search_read_top_results_enabled(arguments) {
            read_browser_search_results(&results, arguments, max_results).await
        } else {
            Vec::new()
        };
        let read_errors = read_results
            .iter()
            .filter(|entry| entry["status"] == "error")
            .count();
        let status = if results.is_empty() {
            "no_results"
        } else if browser_search_read_top_results_enabled(arguments) && read_errors > 0 {
            "partial_success"
        } else {
            "success"
        };

        Ok(json!({
            "status": status,
            "engine": engine,
            "query": query,
            "result_count": results.len(),
            "rendered_result_count": rendered_result_count,
            "static_result_count": static_result_count,
            "extraction_strategy": if rendered_result_count > 0 { "rendered_dom" } else if static_result_count > 0 { "page_source" } else { "empty" },
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

// ── 0. Doctor ────────────────────────────────────────────────
pub struct SearchXyzDoctorTool;

#[async_trait::async_trait]
impl Tool for SearchXyzDoctorTool {
    fn name(&self) -> &str {
        "searchxyz_doctor"
    }

    fn description(&self) -> &str {
        "Report SearchXyz native search health, configured backends, cache/index paths, and fallback policy without performing a web search."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "include_paths": {
                    "type": "boolean",
                    "description": "Include local cache/index/graph paths in the report (default: true)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let include_paths = arguments
            .get("include_paths")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        Ok(json!(build_searchxyz_doctor_report(include_paths).await?))
    }
}

// ── 1. Search Web ─────────────────────────────────────────────
pub struct SearchXyzSearchWebTool;

#[async_trait::async_trait]
impl Tool for SearchXyzSearchWebTool {
    fn name(&self) -> &str {
        "searchxyz_search_web"
    }

    fn description(&self) -> &str {
        "Search the web using searchxyz. Returns titles, URLs, and snippets. Useful for keyless searches."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query string."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Max results to return (default: 10)."
                },
                "include_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only keep results from these domains."
                },
                "exclude_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Drop results from these domains."
                },
                "merge_backends": {
                    "type": "boolean",
                    "description": "Query all available backends and deduplicate results instead of stopping at the first success."
                },
                "include_diagnostics": {
                    "type": "boolean",
                    "description": "Append compact backend diagnostics to the output."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: SearchWebRequest = serde_json::from_value(arguments.clone())?;
        let server = get_server();
        let configured_backends = server.config.search.backends.clone();
        match server.search_web(Parameters(req)).await {
            Ok(res) => Ok(json!(res)),
            Err(err) => {
                let raw_error = format!("MCP Error {:?}: {}", err.code, err.message);
                let kind = classify_search_failure(&raw_error);
                record_search_backend_failure(&configured_backends, kind);
                Err(anyhow!(
                    "{}",
                    search_failure_error_value(&raw_error, kind, &configured_backends)
                ))
            }
        }
    }
}

// ── 2. Read URL ───────────────────────────────────────────────
pub struct SearchXyzReadUrlTool;

#[async_trait::async_trait]
impl Tool for SearchXyzReadUrlTool {
    fn name(&self) -> &str {
        "searchxyz_read_url"
    }

    fn description(&self) -> &str {
        "Fetch a URL and extract its content as clean Markdown. Also handles PDFs, YouTube transcripts, and Git repos. For a direct URL, use this OR web_fetch, never both in the same turn."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The full URL to fetch."
                },
                "depth": {
                    "type": "integer",
                    "description": "Crawl depth for recursive crawling (default: 1)."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Enable headless JS rendering (default: false)."
                },
                "cache_mode": {
                    "type": "string",
                    "enum": ["auto", "prefer_cache", "revalidate", "bypass"],
                    "description": "Cache behavior. Use revalidate/check-again or bypass for live refresh tasks."
                },
                "save_mode": {
                    "type": "string",
                    "enum": ["full", "none"],
                    "description": "Whether to persist fetched content into SearchXyz memory. Use none for one-off checks."
                },
                "include_diagnostics": {
                    "type": "boolean",
                    "description": "Append compact diagnostics to the output."
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Optional output character budget. Truncates large Markdown responses with metadata when exceeded."
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: ReadUrlRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server()
            .read_url(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 3. Search and Read ─────────────────────────────────────────
pub struct SearchXyzSearchAndReadTool;

#[async_trait::async_trait]
impl Tool for SearchXyzSearchAndReadTool {
    fn name(&self) -> &str {
        "searchxyz_search_and_read"
    }

    fn description(&self) -> &str {
        "Search the web AND crawl the top results in a single call. Returns full Markdown for each result page."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query string."
                },
                "max_pages": {
                    "type": "integer",
                    "description": "How many top results to crawl (default: 3)."
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
                    "description": "Query all available backends and deduplicate results before crawling."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Enable JS rendering (default: false)."
                },
                "cache_mode": {
                    "type": "string",
                    "enum": ["auto", "prefer_cache", "revalidate", "bypass"],
                    "description": "Cache behavior. Use revalidate/check-again or bypass for live refresh tasks."
                },
                "save_mode": {
                    "type": "string",
                    "enum": ["full", "none"],
                    "description": "Whether to persist fetched content into SearchXyz memory. Use none for one-off checks."
                },
                "include_diagnostics": {
                    "type": "boolean",
                    "description": "Append compact diagnostics to the output."
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Optional output character budget. Truncates large Markdown responses with metadata when exceeded."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: SearchAndReadRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server()
            .search_and_read(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 6. Deep Research ──────────────────────────────────────────
pub struct SearchXyzDeepResearchTool;

#[async_trait::async_trait]
impl Tool for SearchXyzDeepResearchTool {
    fn name(&self) -> &str {
        "searchxyz_deep_research"
    }

    fn description(&self) -> &str {
        "Perform iterative multi-query web crawls and compile a deep research markdown report."
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
                    "description": "Number of expanded sub-queries to execute (default: 3)."
                },
                "max_pages_per_query": {
                    "type": "integer",
                    "description": "How many top pages to crawl per sub-query (default: 2)."
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
                    "description": "Query all available backends and deduplicate results before crawling."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Enable headless JS rendering (default: false)."
                },
                "cache_mode": {
                    "type": "string",
                    "enum": ["auto", "prefer_cache", "revalidate", "bypass"],
                    "description": "Cache behavior. Use revalidate/check-again or bypass for live refresh tasks."
                },
                "save_mode": {
                    "type": "string",
                    "enum": ["full", "none"],
                    "description": "Whether to persist fetched content into SearchXyz memory. Use none for one-off checks."
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
        let req: DeepResearchRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server()
            .deep_research(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 8. Sitemap ────────────────────────────────────────────────
pub struct SearchXyzSiteMapTool;

#[async_trait::async_trait]
impl Tool for SearchXyzSiteMapTool {
    fn name(&self) -> &str {
        "searchxyz_site_map"
    }

    fn description(&self) -> &str {
        "Discover sitemap URLs or map domain structure via fast recursive link crawling."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Domain root to map."
                },
                "use_sitemap": {
                    "type": "boolean",
                    "description": "Try parsing sitemap.xml (default: true)."
                },
                "crawl_links": {
                    "type": "boolean",
                    "description": "Fallback to internal link spiders (default: true)."
                },
                "max_links": {
                    "type": "integer",
                    "description": "Max discovered links to return (default: 100)."
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: SiteMapRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server()
            .site_map(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_health_labels_cover_expected_states() {
        let mut config = searchxyz::config::Config::default();
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
}
