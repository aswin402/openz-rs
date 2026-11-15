use crate::tools::searchxyz::core::search::{
    duckduckgo::DuckDuckGoBackend, exa::ExaBackend, mojeek::MojeekBackend, tavily::TavilyBackend,
    websurfx::WebsurfxBackend, SearchBackend, SearchDispatcher, SearchQuery, SearchResult,
};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use futures_util::future::join_all;
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebSearchPolicy {
    NativeOnly,
    NativeThenBrowser,
    NativeThenExternal,
    ExternalOnly,
}

impl WebSearchPolicy {
    fn parse(value: Option<&str>) -> Self {
        let env_value = std::env::var("OPENZ_WEB_SEARCH_POLICY")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let configured = crate::tools::searchxyz::get_server()
            .config
            .search_policy
            .as_str();
        let raw = value.or(env_value.as_deref()).unwrap_or(configured);
        match raw.trim().to_ascii_lowercase().as_str() {
            "native_then_browser" | "browser_fallback" | "native-browser" | "native_browser" => {
                Self::NativeThenBrowser
            }
            "native_then_external" | "native-first" | "native_first" | "fallback" => {
                Self::NativeThenExternal
            }
            "external_only" | "external-only" => Self::ExternalOnly,
            _ => Self::NativeOnly,
        }
    }

    fn allows_native(self) -> bool {
        matches!(
            self,
            Self::NativeOnly | Self::NativeThenBrowser | Self::NativeThenExternal
        )
    }

    fn allows_browser(self) -> bool {
        matches!(self, Self::NativeOnly | Self::NativeThenBrowser)
    }

    fn allows_external(self) -> bool {
        matches!(self, Self::NativeThenExternal | Self::ExternalOnly)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::NativeOnly => "native_only",
            Self::NativeThenBrowser => "native_then_browser",
            Self::NativeThenExternal => "native_then_external",
            Self::ExternalOnly => "external_only",
        }
    }
}

fn browser_fallback_engines() -> [&'static str; 2] {
    ["duckduckgo", "bing"]
}

fn native_rescue_results(query: &str) -> Vec<Value> {
    let terms = normalized_terms(query);
    let has_rust = terms.iter().any(|term| term == "rust");
    match has_rust.then(|| detect_rust_crate(&terms)).flatten() {
        Some(crate_name) => vec![
            json!({
                "title": format!("{} - Rust crate documentation", crate_name),
                "url": format!("https://docs.rs/{crate_name}"),
                "snippet": format!("Native rescue result: docs.rs documentation for the Rust crate `{crate_name}`."),
                "source": "native_rescue"
            }),
            json!({
                "title": format!("{} - crates.io", crate_name),
                "url": format!("https://crates.io/crates/{crate_name}"),
                "snippet": format!("Native rescue result: crates.io package page for `{crate_name}`."),
                "source": "native_rescue"
            }),
        ],
        None => Vec::new(),
    }
}

fn normalized_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn native_search_results_need_merge_retry(query: &str, results: &[Value]) -> bool {
    let terms = normalized_terms(query)
        .into_iter()
        .filter(|term| term.len() >= 3)
        .collect::<Vec<_>>();
    (terms.len() >= 3 && results.len() >= 3) && {
        let best_coverage = results
            .iter()
            .take(3)
            .map(|result| result_term_coverage(result, &terms))
            .max()
            .unwrap_or(0);
        best_coverage < 2
    }
}

fn result_term_coverage(result: &Value, terms: &[String]) -> usize {
    let haystack = ["title", "url", "snippet"]
        .into_iter()
        .filter_map(|key| result.get(key).and_then(|value| value.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    terms.iter().filter(|term| haystack.contains(*term)).count()
}

fn searchxyz_results_to_json(results: Vec<SearchResult>) -> Vec<Value> {
    results
        .into_iter()
        .map(|r| {
            json!({
                "title": r.title,
                "url": r.url,
                "snippet": r.snippet,
                "source": "searchxyz"
            })
        })
        .collect()
}

fn extract_domain_list(arguments: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|key| arguments.get(*key))
        .map(|val| match val {
            Value::Array(items) => items
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect(),
            Value::String(csv) => csv
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

pub(crate) fn build_search_query(query: &str, arguments: &Value) -> SearchQuery {
    let max_results = crate::tools::arguments::extract_u64_arg(
        arguments,
        &["max_results", "maxResults", "num_results", "count"],
    )
    .map(|v| v.clamp(1, 20) as usize)
    .unwrap_or(10);

    let mut sq = SearchQuery::new(query, max_results);
    sq.include_domains = extract_domain_list(arguments, &["include_domains", "includeDomains"]);
    sq.exclude_domains = extract_domain_list(arguments, &["exclude_domains", "excludeDomains"]);
    sq.merge_backends = crate::tools::arguments::get_bool_arg(
        arguments,
        &["merge_backends", "mergeBackends"],
        false,
    );
    sq
}

fn web_search_should_auto_read_top_results(query: &str, arguments: &Value) -> bool {
    crate::tools::arguments::extract_bool_arg(
        arguments,
        &["read_top_results", "readTopResults", "auto_read", "autoRead"],
    )
    .unwrap_or_else(|| {
        let normalized = query.to_ascii_lowercase();
        [
            "research",
            "summarize",
            "summary",
            "compare",
            "comparison",
            "latest",
            "current",
            "today",
            "pricing",
            "release",
            "changelog",
            "what's new",
            "whats new",
            "market",
            "landscape",
            "deep dive",
        ]
        .iter()
        .any(|needle| normalized.contains(needle))
    })
}

fn web_search_auto_read_max_pages(arguments: &Value, should_read: bool) -> usize {
    match should_read {
        false => 0,
        true => crate::tools::arguments::extract_u64_arg(
            arguments,
            &["max_pages", "maxPages", "limit", "max_results"],
        )
        .map(|value| value.clamp(1, 5) as usize)
        .unwrap_or(3),
    }
}

fn web_search_should_diagnose_on_failure(arguments: &Value) -> bool {
    crate::tools::arguments::get_bool_arg(
        arguments,
        &["diagnose_on_failure", "diagnoseOnFailure"],
        true,
    )
}

pub fn extract_web_search_query(arguments: &Value) -> Result<String> {
    let raw_query = match arguments.as_str() {
        Some(s) => s.trim(),
        None => arguments
            .get("query")
            .or_else(|| arguments.get("q"))
            .or_else(|| arguments.get("search"))
            .or_else(|| arguments.get("prompt"))
            .or_else(|| arguments.get("term"))
            .or_else(|| arguments.get("keywords"))
            .or_else(|| arguments.get("text"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?
            .trim(),
    };

    (!raw_query.is_empty())
        .then_some(())
        .ok_or_else(|| anyhow!("Missing 'query' parameter (received empty string)"))?;

    let domain = arguments
        .get("domain")
        .or_else(|| arguments.get("site"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|d| !d.is_empty());

    match domain.filter(|_| !raw_query.contains("site:")) {
        Some(dom) => Ok(format!("{} site:{}", raw_query, dom)),
        None => Ok(raw_query.to_string()),
    }
}

fn browser_search_value_to_web_search_result(value: Value) -> Value {
    let has_read_results = value
        .get("read_results")
        .and_then(|read_results| read_results.as_array())
        .is_some_and(|read_results| !read_results.is_empty());
    match has_read_results {
        true => value,
        false => Value::Array(
            value
                .get("results")
                .and_then(|results| results.as_array())
                .cloned()
                .unwrap_or_default(),
        ),
    }
}

async fn attach_top_result_reads(results: Vec<Value>, query: &str, arguments: &Value) -> Value {
    let should_read = web_search_should_auto_read_top_results(query, arguments);
    let max_pages = web_search_auto_read_max_pages(arguments, should_read);
    match max_pages == 0 || results.is_empty() {
        true => Value::Array(results),
        false => {
            let server = crate::tools::searchxyz::get_server();
            let candidate_urls: Vec<String> = results
                .iter()
                .filter_map(|item| item.get("url").and_then(|u| u.as_str()))
                .filter(|u| !u.trim().is_empty())
                .take(max_pages)
                .map(ToString::to_string)
                .collect();

            let read_futs = candidate_urls.into_iter().map(|url| async move {
                match server
                    .read_url(crate::tools::searchxyz::server::ReadUrlRequest {
                        url: url.clone(),
                        depth: Some(1),
                        render_js: Some(false),
                        cache_mode: None,
                        save_mode: None,
                        include_diagnostics: Some(false),
                        max_chars: Some(8_000),
                    })
                    .await
                {
                    Ok(markdown) => (!markdown.trim().is_empty()).then(|| {
                        json!({
                            "status": "success",
                            "url": url,
                            "content": markdown,
                        })
                    }),
                    Err(err) => {
                        tracing::debug!(url = %url, error = ?err, "web_search auto-read skipped failed URL");
                        None
                    }
                }
            });

            let read_results: Vec<Value> = join_all(read_futs).await.into_iter().flatten().collect();
            match read_results.is_empty() {
                true => Value::Array(results),
                false => json!({
                    "results": results,
                    "read_count": read_results.len(),
                    "read_results": read_results,
                }),
            }
        }
    }
}

fn web_search_archive_text(search_res: &Value) -> Option<String> {
    let results = search_res
        .as_array()
        .or_else(|| search_res.get("results").and_then(|v| v.as_array()))?;

    (!results.is_empty()).then_some(())?;

    let mut text = results
        .iter()
        .map(|r| {
            format!(
                "Title: {}\nURL: {}\nSnippet: {}\n---",
                r["title"].as_str().unwrap_or_default(),
                r["url"].as_str().unwrap_or_default(),
                r["snippet"].as_str().unwrap_or_default()
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    for read in search_res
        .get("read_results")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .take(3)
    {
        let url = read["url"].as_str().unwrap_or_default();
        let content = read.get("content").unwrap_or(&Value::Null);
        let content_text = content
            .as_str()
            .or_else(|| content.get("content").and_then(|value| value.as_str()))
            .or_else(|| content.get("markdown").and_then(|value| value.as_str()))
            .unwrap_or_default();
        let _ = (!content_text.trim().is_empty()).then(|| {
            text.push_str(&format!(
                "\nRead URL: {}\nContent: {}\n---",
                url,
                content_text.chars().take(2000).collect::<String>()
            ));
        });
    }

    Some(text)
}

fn detect_rust_crate(terms: &[String]) -> Option<String> {
    const KNOWN_CRATES: &[&str] = &[
        "tokio",
        "axum",
        "hyper",
        "tonic",
        "serde",
        "reqwest",
        "clap",
        "tracing",
        "rusqlite",
        "tantivy",
        "crossterm",
        "ratatui",
        "bevy",
    ];

    KNOWN_CRATES
        .iter()
        .find(|crate_name| terms.iter().any(|term| term == **crate_name))
        .map(|crate_name| crate_name.to_string())
}

fn format_native_only_failure(native_error: Option<&str>) -> String {
    let detail = native_error
        .map(|e| format!("Native SearchXyz error: {e}"))
        .unwrap_or_else(|| "Native SearchXyz returned no usable results.".to_string());
    format!(
        "Native web_search policy is active (`search_policy=native_only`); external search backends are disabled. {detail}

Next steps:
- Run `diagnose_system` (alias `searchxyz_doctor`) to inspect native backend health.
- Provider-free browser discovery is attempted automatically before this failure is returned.
- Configure `SEARCHXYZ_SEARXNG_URL` for stronger private/native discovery.
- Diagnostics are appended automatically unless `diagnose_on_failure=false` is set.
- For one-off API/scraper fallback, call `web_search` with `search_policy=native_then_external`."
    )
}

pub struct WebSearchTool {
    client: Client,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        WebSearchTool {
            client: crate::core::http::default_http_client(),
        }
    }

    fn build_external_dispatcher(&self) -> SearchDispatcher {
        let server = crate::tools::searchxyz::get_server();
        let server_cfg = &server.config;
        let timeout_secs = server_cfg.search.timeout_secs;
        let mut backends: Vec<Box<dyn SearchBackend>> = Vec::new();

        let websurfx_url = std::env::var("WEBSURFX_URL")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .or_else(|| {
                server_cfg
                    .websurfx_url
                    .clone()
                    .filter(|u| !u.trim().is_empty())
            });
        match websurfx_url {
            Some(url) => {
                backends.push(Box::new(WebsurfxBackend::new(
                    self.client.clone(),
                    Some(url),
                    timeout_secs,
                )));
            }
            None => {}
        }

        let tavily_key = std::env::var("TAVILY_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| {
                server_cfg
                    .tavily_api_key
                    .clone()
                    .filter(|k| !k.trim().is_empty())
            });
        match tavily_key {
            Some(key) => {
                backends.push(Box::new(TavilyBackend::new(
                    self.client.clone(),
                    Some(key),
                    timeout_secs,
                )));
            }
            None => {}
        }

        let exa_key = std::env::var("EXA_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| {
                server_cfg
                    .exa_api_key
                    .clone()
                    .filter(|k| !k.trim().is_empty())
            });
        match exa_key {
            Some(key) => {
                backends.push(Box::new(ExaBackend::new(
                    self.client.clone(),
                    Some(key),
                    timeout_secs,
                )));
            }
            None => {}
        }

        backends.push(Box::new(
            DuckDuckGoBackend::new(self.client.clone())
                .with_proxies(server.crawler.clients().to_vec())
                .with_headless_enabled(server_cfg.headless.enabled),
        ));
        backends.push(Box::new(
            MojeekBackend::new(self.client.clone())
                .with_proxies(server.crawler.clients().to_vec()),
        ));

        SearchDispatcher::new(backends)
    }
}

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Perform a multi-backend web search (with optional top-result page reading and domain filtering) and return matching titles, URLs, snippets, and extracted content."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query term (supports q/search/prompt aliases or direct string)."
                },
                "domain": {
                    "type": "string",
                    "description": "Optional domain or website to restrict search to (e.g. 'docs.rs', 'github.com'). Automatically appends site:<domain> to query."
                },
                "include_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional allowlist of domains to keep in search results."
                },
                "exclude_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional denylist of domains to exclude from search results."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of search results to return (default: 10, max: 20)."
                },
                "merge_backends": {
                    "type": "boolean",
                    "description": "Query all healthy search backends concurrently and merge results via Reciprocal Rank Fusion (default: false)."
                },
                "search_policy": {
                    "type": "string",
                    "enum": ["native_only", "native_then_browser", "native_then_external", "external_only"],
                    "description": "Search backend policy. Default native_only uses OpenZ's SearchXyz engine, native rescue, and provider-free BrowserBroker discovery; native_then_external allows API/scraper fallback; external_only skips native SearchXyz."
                },
                "diagnose_on_failure": {
                    "type": "boolean",
                    "description": "When search tiers fail, append SearchXyz diagnostics to the error for actionable debugging. Defaults to true; set false to opt out."
                },
                "read_top_results": {
                    "type": "boolean",
                    "description": "Automatically fetch and extract Markdown from the top matching result URLs. Defaults to true for research/latest/compare/summarize queries."
                },
                "max_pages": {
                    "type": "integer",
                    "description": "Maximum top result pages to read when read_top_results is active (default: 3, max: 5)."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = extract_web_search_query(arguments)?;

        let search_res = self.perform_search(&query, arguments).await?;

        match web_search_archive_text(&search_res) {
            Some(results_str) => {
                let _ = crate::tools::shared_memory::archive_research_entry(
                    &query,
                    &results_str,
                    "web_search",
                )
                .await;
            }
            None => {}
        }

        Ok(search_res)
    }
}

impl WebSearchTool {
    async fn try_native_search(
        &self,
        query: &str,
        arguments: &Value,
        policy: WebSearchPolicy,
    ) -> (Option<Vec<Value>>, Option<String>) {
        match policy.allows_native() {
            false => (None, None),
            true => {
                let search_query = build_search_query(query, arguments);
                match crate::tools::searchxyz::get_server()
                    .dispatcher
                    .search(&search_query)
                    .await
                {
                    Ok(results) => {
                        let search_results = searchxyz_results_to_json(results);
                        match search_results.is_empty() {
                            true => (None, None),
                            false => {
                                let should_retry_merge = !search_query.merge_backends
                                    && native_search_results_need_merge_retry(
                                        query,
                                        &search_results,
                                    );
                                match should_retry_merge {
                                    true => {
                                        let mut merged_query = search_query.clone();
                                        merged_query.merge_backends = true;
                                        match crate::tools::searchxyz::get_server()
                                            .dispatcher
                                            .search(&merged_query)
                                            .await
                                        {
                                            Ok(merged_results) => {
                                                let merged_json =
                                                    searchxyz_results_to_json(merged_results);
                                                match merged_json.is_empty() {
                                                    false => {
                                                        tracing::info!(query = %query, "SearchXyz first native results looked weak; returned RRF-merged backend results");
                                                        return (Some(merged_json), None);
                                                    }
                                                    true => {}
                                                }
                                            }
                                            Err(err) => {
                                                tracing::warn!(query = %query, error = ?err, "SearchXyz merge retry failed after weak native results");
                                            }
                                        }
                                    }
                                    false => {}
                                }
                                (Some(search_results), None)
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(policy = policy.as_str(), error = ?e, "SearchXyz native search failed");
                        (None, Some(e.to_string()))
                    }
                }
            }
        }
    }

    async fn perform_search(&self, query: &str, arguments: &Value) -> Result<Value> {
        let policy =
            WebSearchPolicy::parse(arguments.get("search_policy").and_then(|v| v.as_str()));
        let diagnose_on_failure = web_search_should_diagnose_on_failure(arguments);

        let (native_hit, native_error) = self.try_native_search(query, arguments, policy).await;
        match native_hit {
            Some(results) => return Ok(attach_top_result_reads(results, query, arguments).await),
            None => {}
        }

        match policy.allows_native() {
            true => {
                let rescue_results = native_rescue_results(query);
                match rescue_results.is_empty() {
                    false => {
                        return Ok(attach_top_result_reads(rescue_results, query, arguments).await)
                    }
                    true => {}
                }
            }
            false => {}
        }

        match policy.allows_browser() {
            true => {
                let should_read_top_results =
                    web_search_should_auto_read_top_results(query, arguments);
                let max_pages = web_search_auto_read_max_pages(arguments, should_read_top_results);
                let browser_tool = crate::tools::searchxyz::SearchXyzBrowserSearchTool;
                for engine in browser_fallback_engines() {
                    match browser_tool
                        .call(&json!({
                            "query": query,
                            "engine": engine,
                            "max_results": 5,
                            "read_top_results": should_read_top_results,
                            "max_pages": max_pages,
                            "save_mode": "none",
                        }))
                        .await
                    {
                        Ok(value) => {
                            let has_results = value
                                .get("results")
                                .and_then(|v| v.as_array())
                                .is_some_and(|results| !results.is_empty());
                            match has_results {
                                true => return Ok(browser_search_value_to_web_search_result(value)),
                                false => {
                                    tracing::warn!(query = %query, engine = %engine, result = %value, "browser search fallback returned no usable links");
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!(query = %query, engine = %engine, error = ?err, "browser search fallback failed");
                        }
                    }
                }
            }
            false => {}
        }

        match policy.allows_external() {
            true => {}
            false => {
                let mut message = format_native_only_failure(native_error.as_deref());
                match diagnose_on_failure {
                    true => {
                        match crate::tools::searchxyz::doctor_report::build_searchxyz_doctor_report(
                            false,
                        )
                        .await
                        {
                            Ok(report) => {
                                message.push_str("\n\n---\n");
                                message.push_str(&report);
                            }
                            Err(err) => {
                                message.push_str(&format!(
                                    "\n\nSearchXyz doctor failed while building diagnostics: {err}"
                                ));
                            }
                        }
                    }
                    false => {}
                }
                return Err(anyhow!(message));
            }
        }

        let external_dispatcher = self.build_external_dispatcher();
        let search_query = build_search_query(query, arguments);
        let external_results = external_dispatcher
            .search(&search_query)
            .await
            .map_err(|err| anyhow!("All external search backends failed: {err}"))?;

        let mapped = searchxyz_results_to_json(external_results);
        match mapped.is_empty() {
            true => Err(anyhow!(
                "Search returned no results from any available external search engine."
            )),
            false => Ok(attach_top_result_reads(mapped, query, arguments).await),
        }
    }
}

#[cfg(test)]
#[path = "web_search_tests.rs"]
mod tests;
