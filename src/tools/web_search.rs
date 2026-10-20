use crate::tools::Tool;
use anyhow::{anyhow, Result};
use reqwest::Client;
use scraper::{Html, Selector};
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
        let env_value = std::env::var("OPENZ_WEB_SEARCH_POLICY").ok();
        let raw = value.or(env_value.as_deref()).unwrap_or("native_only");
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

fn searchxyz_results_to_json(results: Vec<crate::tools::searchxyz::core::search::SearchResult>) -> Vec<Value> {
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

fn web_search_should_auto_read_top_results(query: &str, arguments: &Value) -> bool {
    crate::tools::arguments::extract_bool_arg(
        arguments,
        &["read_top_results", "readTopResults"],
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
- Run `searchxyz_doctor` to inspect native backend health.
- Provider-free browser discovery is attempted automatically before this failure is returned.
- Configure `SEARCHXYZ_SEARXNG_URL` for stronger private/native discovery.
- Diagnostics are appended automatically unless `diagnose_on_failure=false` is set.
- For one-off API/scraper fallback, call `web_search` with `search_policy=native_then_external`."
    )
}

fn extract_external_json_results(resp_json: &Value, snippet_key: &str) -> Option<Vec<Value>> {
    let results = resp_json.get("results")?.as_array()?;
    let mapped: Vec<Value> = results
        .iter()
        .map(|r| {
            let title = r
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let url = r
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let snippet = r
                .get(snippet_key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            json!({
                "title": title,
                "url": url,
                "snippet": snippet
            })
        })
        .collect();
    Some(mapped)
}

fn normalize_ddg_href(href: String) -> String {
    let decoded = match href.find("uddg=") {
        Some(pos) => percent_encoding::percent_decode_str(&href[pos + 5..])
            .decode_utf8_lossy()
            .into_owned(),
        None => match href.starts_with("//") {
            true => format!("https:{}", href),
            false => href,
        },
    };
    match decoded.find("&rut=") {
        Some(pos) => decoded[..pos].to_string(),
        None => decoded,
    }
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
            client: Client::builder()
                .use_rustls_tls()
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Perform a web search query and return a list of matching page titles, URLs, and snippets."
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
                "search_policy": {
                    "type": "string",
                    "enum": ["native_only", "native_then_browser", "native_then_external", "external_only"],
                    "description": "Search backend policy. Default native_only uses the local stack: SearchXyz native discovery, native rescue, then browser discovery without Brave/SearXNG; native_then_external allows API/scraper fallback; external_only skips SearchXyz. Can also be set with OPENZ_WEB_SEARCH_POLICY."
                },
                "diagnose_on_failure": {
                    "type": "boolean",
                    "description": "When SearchXyz plus browser fallback fail, append searchxyz_doctor output to the error for actionable debugging. Defaults to true; set false to opt out."
                },
                "read_top_results": {
                    "type": "boolean",
                    "description": "Override automatic research-style page reading. By default, research/latest/compare/summarize queries read top browser-discovered pages."
                },
                "max_pages": {
                    "type": "integer",
                    "description": "Maximum browser-discovered result pages to read for research-style queries (default: 3, max: 5)."
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
    async fn try_native_search(&self, query: &str, policy: WebSearchPolicy) -> (Option<Value>, Option<String>) {
        match policy.allows_native() {
            false => (None, None),
            true => {
                let search_query =
                    crate::tools::searchxyz::core::search::SearchQuery::new(query, 10);
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
                                match native_search_results_need_merge_retry(query, &search_results)
                                {
                                    true => {
                                        let mut merged_query =
                                            crate::tools::searchxyz::core::search::SearchQuery::new(
                                                query, 10,
                                            );
                                        merged_query.merge_backends = true;
                                        match crate::tools::searchxyz::get_server()
                                            .dispatcher
                                            .search(&merged_query)
                                            .await
                                        {
                                            Ok(merged_results) => {
                                                let merged_results =
                                                    searchxyz_results_to_json(merged_results);
                                                match merged_results.is_empty() {
                                                    false => {
                                                        tracing::info!(query = %query, "SearchXyz first native results looked weak; returned merged backend results");
                                                        return (
                                                            Some(Value::Array(merged_results)),
                                                            None,
                                                        );
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
                                (Some(Value::Array(search_results)), None)
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

        let (native_hit, native_error) = self.try_native_search(query, policy).await;
        match native_hit {
            Some(hit) => return Ok(hit),
            None => {}
        }

        match policy.allows_native() {
            true => {
                let rescue_results = native_rescue_results(query);
                match rescue_results.is_empty() {
                    false => return Ok(Value::Array(rescue_results)),
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
                    true => match crate::tools::searchxyz::web::build_searchxyz_doctor_report(false).await {
                        Ok(report) => {
                            message.push_str("\n\n---\n");
                            message.push_str(&report);
                        }
                        Err(err) => {
                            message.push_str(&format!(
                                "\n\nSearchXyz doctor failed while building diagnostics: {err}"
                            ));
                        }
                    },
                    false => {}
                }
                return Err(anyhow!(message));
            }
        }

        // 1. Try Websurfx Local/Private Search Engine API (if WEBSURFX_URL is set)
        match std::env::var("WEBSURFX_URL")
            .ok()
            .filter(|url| !url.trim().is_empty())
        {
            Some(websurfx_url) => {
                let base = websurfx_url.trim().trim_end_matches('/');
                let encoded_query = percent_encoding::utf8_percent_encode(
                    query,
                    percent_encoding::NON_ALPHANUMERIC,
                )
                .to_string();
                let url = format!("{}/?q={}&json=true", base, encoded_query);

                let res = self.client.get(&url).send().await?;
                match res.status().is_success() {
                    true => {
                        let resp_json: Value = res.json().await?;
                        match extract_external_json_results(&resp_json, "content")
                            .filter(|r| !r.is_empty())
                        {
                            Some(search_results) => return Ok(Value::Array(search_results)),
                            None => {}
                        }
                    }
                    false => {}
                }
            }
            None => {}
        }

        // 2. Try Tavily Search API
        match std::env::var("TAVILY_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
        {
            Some(tavily_key) => {
                let body = json!({
                    "api_key": tavily_key,
                    "query": query,
                    "search_depth": "basic",
                    "max_results": 5
                });
                let res = self
                    .client
                    .post("https://api.tavily.com/search")
                    .json(&body)
                    .send()
                    .await?;

                match res.status().is_success() {
                    true => {
                        let resp_json: Value = res.json().await?;
                        match extract_external_json_results(&resp_json, "content") {
                            Some(search_results) => return Ok(Value::Array(search_results)),
                            None => {}
                        }
                    }
                    false => {}
                }
            }
            None => {}
        }

        // 3. Try Exa Search API
        match std::env::var("EXA_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
        {
            Some(exa_key) => {
                let body = json!({
                    "query": query,
                    "numResults": 5,
                    "useAutoprompt": true
                });
                let res = self
                    .client
                    .post("https://api.exa.ai/search")
                    .header("x-api-key", exa_key)
                    .json(&body)
                    .send()
                    .await?;

                match res.status().is_success() {
                    true => {
                        let resp_json: Value = res.json().await?;
                        match extract_external_json_results(&resp_json, "text") {
                            Some(search_results) => return Ok(Value::Array(search_results)),
                            None => {}
                        }
                    }
                    false => {}
                }
            }
            None => {}
        }

        // 4. Fallback to DuckDuckGo scraping
        let mut search_results = Vec::new();
        let res = self.client.get("https://html.duckduckgo.com/html/")
            .query(&[("q", query)])
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send()
            .await;

        match res {
            Ok(response) => match response.status().is_success() {
                true => match response.text().await {
                    Ok(html_content) => {
                        let document = Html::parse_document(&html_content);
                        match (
                            Selector::parse(".result"),
                            Selector::parse(".result__title .result__a"),
                            Selector::parse(".result__snippet"),
                        ) {
                            (Ok(result_selector), Ok(title_selector), Ok(snippet_selector)) => {
                                for element in document.select(&result_selector) {
                                    let title = element
                                        .select(&title_selector)
                                        .next()
                                        .map(|e| e.text().collect::<String>().trim().to_string())
                                        .unwrap_or_default();

                                    let href = element
                                        .select(&title_selector)
                                        .next()
                                        .and_then(|e| e.value().attr("href"))
                                        .map(|s| s.to_string())
                                        .unwrap_or_default();

                                    let snippet = element
                                        .select(&snippet_selector)
                                        .next()
                                        .map(|e| e.text().collect::<String>().trim().to_string())
                                        .unwrap_or_default();

                                    let _ = (!title.is_empty() && !href.is_empty()).then(|| {
                                        let clean_url = normalize_ddg_href(href);
                                        search_results.push(json!({
                                            "title": title,
                                            "url": clean_url,
                                            "snippet": snippet
                                        }));
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(_) => {}
                },
                false => {}
            },
            Err(_) => {}
        }

        // 5. Try Mojeek scraping if DuckDuckGo fails or returns no results
        match search_results.is_empty() {
            true => {
                tracing::warn!("DuckDuckGo search returned no results, falling back to Mojeek");
                let res = self.client.get("https://www.mojeek.com/search")
                    .query(&[("q", query)])
                    .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                    .send()
                    .await;

                match res {
                    Ok(response) => match response.status().is_success() {
                        true => match response.text().await {
                            Ok(html_content) => {
                                let document = Html::parse_document(&html_content);
                                match (
                                    Selector::parse("li"),
                                    Selector::parse("a.title"),
                                    Selector::parse("p.s"),
                                ) {
                                    (Ok(li_selector), Ok(title_selector), Ok(snippet_selector)) => {
                                        for element in document.select(&li_selector) {
                                            let title_node = element.select(&title_selector).next();
                                            let snippet_node =
                                                element.select(&snippet_selector).next();

                                            let _ = title_node.map(|tn| {
                                                let title = tn
                                                    .text()
                                                    .collect::<String>()
                                                    .trim()
                                                    .to_string();
                                                let href = tn
                                                    .value()
                                                    .attr("href")
                                                    .map(|s| s.to_string())
                                                    .unwrap_or_default();
                                                let snippet = snippet_node
                                                    .map(|e| {
                                                        e.text()
                                                            .collect::<String>()
                                                            .trim()
                                                            .to_string()
                                                    })
                                                    .unwrap_or_default();

                                                let _ = (!title.is_empty() && !href.is_empty())
                                                    .then(|| {
                                                        search_results.push(json!({
                                                            "title": title,
                                                            "url": href,
                                                            "snippet": snippet
                                                        }));
                                                    });
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Err(_) => {}
                        },
                        false => {}
                    },
                    Err(_) => {}
                }
            }
            false => {}
        }

        (!search_results.is_empty())
            .then_some(Value::Array(search_results))
            .ok_or_else(|| {
                anyhow!(
                    "All enabled external web search backends (Websurfx, Tavily, Exa, DuckDuckGo, Mojeek) failed or returned no results. Current policy: {}.",
                    policy.as_str()
                )
            })
    }
}

#[cfg(test)]
#[path = "web_search_tests.rs"]
mod tests;

