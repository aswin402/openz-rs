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
    let mut results = Vec::new();

    if terms.iter().any(|term| term == "rust") {
        if let Some(crate_name) = detect_rust_crate(&terms) {
            results.push(json!({
                "title": format!("{} - Rust crate documentation", crate_name),
                "url": format!("https://docs.rs/{crate_name}"),
                "snippet": format!("Native rescue result: docs.rs documentation for the Rust crate `{crate_name}`."),
                "source": "native_rescue"
            }));
            results.push(json!({
                "title": format!("{} - crates.io", crate_name),
                "url": format!("https://crates.io/crates/{crate_name}"),
                "snippet": format!("Native rescue result: crates.io package page for `{crate_name}`."),
                "source": "native_rescue"
            }));
        }
    }

    results
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
    if terms.len() < 3 || results.len() < 3 {
        return false;
    }

    let best_coverage = results
        .iter()
        .take(3)
        .map(|result| result_term_coverage(result, &terms))
        .max()
        .unwrap_or(0);

    best_coverage < 2
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

fn searchxyz_results_to_json(results: Vec<searchxyz::search::SearchResult>) -> Vec<Value> {
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
    if let Some(explicit) = arguments
        .get("read_top_results")
        .and_then(|value| value.as_bool())
    {
        return explicit;
    }

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
}

fn web_search_auto_read_max_pages(arguments: &Value, should_read: bool) -> usize {
    if !should_read {
        return 0;
    }
    arguments
        .get("max_pages")
        .and_then(|value| value.as_u64())
        .map(|value| value.clamp(1, 5) as usize)
        .unwrap_or(3)
}

fn web_search_should_diagnose_on_failure(arguments: &Value) -> bool {
    arguments
        .get("diagnose_on_failure")
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

fn browser_search_value_to_web_search_result(value: Value) -> Value {
    if value
        .get("read_results")
        .and_then(|read_results| read_results.as_array())
        .is_some_and(|read_results| !read_results.is_empty())
    {
        value
    } else {
        Value::Array(
            value
                .get("results")
                .and_then(|results| results.as_array())
                .cloned()
                .unwrap_or_default(),
        )
    }
}

fn web_search_archive_text(search_res: &Value) -> Option<String> {
    let results = if let Some(arr) = search_res.as_array() {
        arr
    } else {
        search_res.get("results")?.as_array()?
    };

    if results.is_empty() {
        return None;
    }

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

    if let Some(read_results) = search_res
        .get("read_results")
        .and_then(|value| value.as_array())
    {
        for read in read_results.iter().take(3) {
            let url = read["url"].as_str().unwrap_or_default();
            let content = read.get("content").unwrap_or(&Value::Null);
            let content_text = content
                .as_str()
                .or_else(|| content.get("content").and_then(|value| value.as_str()))
                .or_else(|| content.get("markdown").and_then(|value| value.as_str()))
                .unwrap_or_default();
            if !content_text.trim().is_empty() {
                text.push_str(&format!(
                    "\nRead URL: {}\nContent: {}\n---",
                    url,
                    content_text.chars().take(2000).collect::<String>()
                ));
            }
        }
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
                    "description": "The search query term."
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
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;

        let search_res = self.perform_search(arguments).await?;

        if let Some(results_str) = web_search_archive_text(&search_res) {
            let _ = crate::tools::shared_memory::archive_research_entry(
                query,
                &results_str,
                "web_search",
            )
            .await;
        }

        Ok(search_res)
    }
}

impl WebSearchTool {
    async fn perform_search(&self, arguments: &Value) -> Result<Value> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;

        let policy =
            WebSearchPolicy::parse(arguments.get("search_policy").and_then(|v| v.as_str()));
        let diagnose_on_failure = web_search_should_diagnose_on_failure(arguments);
        let mut native_error = None;

        // 0. Try SearchXyz Dispatcher (OpenZ-native search path).
        if policy.allows_native() {
            let search_query = searchxyz::search::SearchQuery::new(query, 10);
            match crate::tools::searchxyz::get_server()
                .dispatcher
                .search(&search_query)
                .await
            {
                Ok(results) => {
                    let search_results = searchxyz_results_to_json(results);
                    if !search_results.is_empty() {
                        if native_search_results_need_merge_retry(query, &search_results) {
                            let mut merged_query = searchxyz::search::SearchQuery::new(query, 10);
                            merged_query.merge_backends = true;
                            match crate::tools::searchxyz::get_server()
                                .dispatcher
                                .search(&merged_query)
                                .await
                            {
                                Ok(merged_results) => {
                                    let merged_results = searchxyz_results_to_json(merged_results);
                                    if !merged_results.is_empty() {
                                        tracing::info!(query = %query, "SearchXyz first native results looked weak; returned merged backend results");
                                        return Ok(Value::Array(merged_results));
                                    }
                                }
                                Err(err) => {
                                    tracing::warn!(query = %query, error = ?err, "SearchXyz merge retry failed after weak native results");
                                }
                            }
                        }
                        return Ok(Value::Array(search_results));
                    }
                }
                Err(e) => {
                    tracing::warn!(policy = policy.as_str(), error = ?e, "SearchXyz native search failed");
                    native_error = Some(e.to_string());
                }
            }
        }

        if policy.allows_native() {
            let rescue_results = native_rescue_results(query);
            if !rescue_results.is_empty() {
                return Ok(Value::Array(rescue_results));
            }
        }

        if policy.allows_browser() {
            let should_read_top_results = web_search_should_auto_read_top_results(query, arguments);
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
                        if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
                            if !results.is_empty() {
                                return Ok(browser_search_value_to_web_search_result(value));
                            }
                        }
                        tracing::warn!(query = %query, engine = %engine, result = %value, "browser search fallback returned no usable links");
                    }
                    Err(err) => {
                        tracing::warn!(query = %query, engine = %engine, error = ?err, "browser search fallback failed");
                    }
                }
            }
        }

        if !policy.allows_external() {
            let mut message = format_native_only_failure(native_error.as_deref());
            if diagnose_on_failure {
                match crate::tools::searchxyz::web::build_searchxyz_doctor_report(false).await {
                    Ok(report) => {
                        message.push_str(
                            "

---
",
                        );
                        message.push_str(&report);
                    }
                    Err(err) => {
                        message.push_str(&format!(
                            "

SearchXyz doctor failed while building diagnostics: {err}"
                        ));
                    }
                }
            }
            return Err(anyhow!(message));
        }

        // 1. Try Websurfx Local/Private Search Engine API (if WEBSURFX_URL is set)
        if let Ok(websurfx_url) = std::env::var("WEBSURFX_URL") {
            if !websurfx_url.trim().is_empty() {
                let base = websurfx_url.trim().trim_end_matches('/');
                let encoded_query = percent_encoding::utf8_percent_encode(
                    query,
                    percent_encoding::NON_ALPHANUMERIC,
                )
                .to_string();
                let url = format!("{}/?q={}&json=true", base, encoded_query);

                let res = self.client.get(&url).send().await?;

                if res.status().is_success() {
                    let resp_json: Value = res.json().await?;
                    if let Some(results) = resp_json.get("results").and_then(|r| r.as_array()) {
                        let mut search_results = Vec::new();
                        for r in results {
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
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            search_results.push(json!({
                                "title": title,
                                "url": url,
                                "snippet": snippet
                            }));
                        }
                        if !search_results.is_empty() {
                            return Ok(Value::Array(search_results));
                        }
                    }
                }
            }
        }

        // 1. Try Tavily Search API
        if let Ok(tavily_key) = std::env::var("TAVILY_API_KEY") {
            if !tavily_key.trim().is_empty() {
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

                if res.status().is_success() {
                    let resp_json: Value = res.json().await?;
                    if let Some(results) = resp_json.get("results").and_then(|r| r.as_array()) {
                        let mut search_results = Vec::new();
                        for r in results {
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
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            search_results.push(json!({
                                "title": title,
                                "url": url,
                                "snippet": snippet
                            }));
                        }
                        return Ok(Value::Array(search_results));
                    }
                }
            }
        }

        // 2. Try Exa Search API
        if let Ok(exa_key) = std::env::var("EXA_API_KEY") {
            if !exa_key.trim().is_empty() {
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

                if res.status().is_success() {
                    let resp_json: Value = res.json().await?;
                    if let Some(results) = resp_json.get("results").and_then(|r| r.as_array()) {
                        let mut search_results = Vec::new();
                        for r in results {
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
                                .get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            search_results.push(json!({
                                "title": title,
                                "url": url,
                                "snippet": snippet
                            }));
                        }
                        return Ok(Value::Array(search_results));
                    }
                }
            }
        }

        // 3. Fallback to DuckDuckGo scraping
        let mut search_results = Vec::new();
        let mut ddg_success = false;

        let res = self.client.get("https://html.duckduckgo.com/html/")
            .query(&[("q", query)])
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send()
            .await;

        if let Ok(response) = res {
            if response.status().is_success() {
                if let Ok(html_content) = response.text().await {
                    let document = Html::parse_document(&html_content);

                    // Select search results
                    if let (Ok(result_selector), Ok(title_selector), Ok(snippet_selector)) = (
                        Selector::parse(".result"),
                        Selector::parse(".result__title .result__a"),
                        Selector::parse(".result__snippet"),
                    ) {
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

                            if !title.is_empty() && !href.is_empty() {
                                // DuckDuckGo redirects URLs inside href (e.g. //duckduckgo.com/l/?uddg=URL)
                                // We clean it up by extracting uddg parameter if present
                                let clean_url = if href.contains("uddg=") {
                                    if let Some(pos) = href.find("uddg=") {
                                        let raw_url = &href[pos + 5..];
                                        percent_encoding::percent_decode_str(raw_url)
                                            .decode_utf8_lossy()
                                            .into_owned()
                                    } else {
                                        href
                                    }
                                } else if href.starts_with("//") {
                                    format!("https:{}", href)
                                } else {
                                    href
                                };

                                // Filter out external parameters after URL if there are any
                                let clean_url = if let Some(pos) = clean_url.find("&rut=") {
                                    clean_url[..pos].to_string()
                                } else {
                                    clean_url
                                };

                                search_results.push(json!({
                                    "title": title,
                                    "url": clean_url,
                                    "snippet": snippet
                                }));
                            }
                        }
                        if !search_results.is_empty() {
                            ddg_success = true;
                        }
                    }
                }
            }
        }

        // 4. Try Mojeek scraping if DuckDuckGo fails or returns no results
        if !ddg_success {
            tracing::warn!("DuckDuckGo search returned no results, falling back to Mojeek");
            let res = self.client.get("https://www.mojeek.com/search")
                .query(&[("q", query)])
                .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .send()
                .await;

            if let Ok(response) = res {
                if response.status().is_success() {
                    if let Ok(html_content) = response.text().await {
                        let document = Html::parse_document(&html_content);
                        if let (Ok(li_selector), Ok(title_selector), Ok(snippet_selector)) = (
                            Selector::parse("li"),
                            Selector::parse("a.title"),
                            Selector::parse("p.s"),
                        ) {
                            for element in document.select(&li_selector) {
                                let title_node = element.select(&title_selector).next();
                                let snippet_node = element.select(&snippet_selector).next();

                                if let Some(tn) = title_node {
                                    let title = tn.text().collect::<String>().trim().to_string();
                                    let href = tn
                                        .value()
                                        .attr("href")
                                        .map(|s| s.to_string())
                                        .unwrap_or_default();
                                    let snippet = snippet_node
                                        .map(|e| e.text().collect::<String>().trim().to_string())
                                        .unwrap_or_default();

                                    if !title.is_empty() && !href.is_empty() {
                                        search_results.push(json!({
                                            "title": title,
                                            "url": href,
                                            "snippet": snippet
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if search_results.is_empty() {
            return Err(anyhow!(
                "All enabled external web search backends (Websurfx, Tavily, Exa, DuckDuckGo, Mojeek) failed or returned no results. Current policy: {}.",
                policy.as_str()
            ));
        }

        Ok(Value::Array(search_results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_web_search() -> Result<()> {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        Ok(())
    }

    #[test]
    fn web_search_policy_defaults_to_native_only() {
        std::env::remove_var("OPENZ_WEB_SEARCH_POLICY");
        assert_eq!(WebSearchPolicy::parse(None), WebSearchPolicy::NativeOnly);
    }

    #[test]
    fn default_native_policy_uses_provider_free_browser_fallback() {
        std::env::remove_var("OPENZ_WEB_SEARCH_POLICY");
        let policy = WebSearchPolicy::parse(None);
        assert!(policy.allows_native());
        assert!(policy.allows_browser());
        assert!(!policy.allows_external());
    }

    #[test]
    fn web_search_policy_accepts_native_then_browser() {
        assert_eq!(
            WebSearchPolicy::parse(Some("native_then_browser")),
            WebSearchPolicy::NativeThenBrowser
        );
        assert!(WebSearchPolicy::NativeThenBrowser.allows_native());
        assert!(WebSearchPolicy::NativeThenBrowser.allows_browser());
        assert!(!WebSearchPolicy::NativeThenBrowser.allows_external());
    }

    #[test]
    fn web_search_schema_exposes_browser_fallback_policy() {
        let tool = WebSearchTool::new();
        let params = tool.parameters();
        let policy_enum = params["properties"]["search_policy"]["enum"]
            .as_array()
            .expect("policy enum");
        assert!(policy_enum.iter().any(|v| v == "native_then_browser"));
    }

    #[test]
    fn web_search_auto_reads_research_style_queries() {
        assert!(web_search_should_auto_read_top_results(
            "research ai agent marketplaces",
            &json!({})
        ));
        assert!(web_search_should_auto_read_top_results(
            "latest rust async runtime docs",
            &json!({})
        ));
        assert!(web_search_should_auto_read_top_results(
            "compare crawlee crawl4ai scrapling",
            &json!({})
        ));
        assert!(!web_search_should_auto_read_top_results(
            "rust homepage",
            &json!({})
        ));
    }

    #[test]
    fn web_search_read_controls_allow_explicit_override() {
        assert!(!web_search_should_auto_read_top_results(
            "research ai agent marketplaces",
            &json!({ "read_top_results": false })
        ));
        assert!(web_search_should_auto_read_top_results(
            "rust homepage",
            &json!({ "read_top_results": true })
        ));
        assert_eq!(web_search_auto_read_max_pages(&json!({}), true), 3);
        assert_eq!(
            web_search_auto_read_max_pages(&json!({ "max_pages": 99 }), true),
            5
        );
        assert_eq!(web_search_auto_read_max_pages(&json!({}), false), 0);
    }

    #[test]
    fn web_search_failure_diagnostics_default_on_with_explicit_opt_out() {
        assert!(web_search_should_diagnose_on_failure(&json!({})));
        assert!(web_search_should_diagnose_on_failure(
            &json!({ "diagnose_on_failure": true })
        ));
        assert!(!web_search_should_diagnose_on_failure(
            &json!({ "diagnose_on_failure": false })
        ));
    }

    #[test]
    fn web_search_archive_text_includes_browser_read_results() {
        let archive = web_search_archive_text(&json!({
            "results": [{
                "title": "Example",
                "url": "https://example.com",
                "snippet": "result snippet"
            }],
            "read_results": [{
                "status": "success",
                "url": "https://example.com",
                "content": { "content": "full page text from browser-discovered result" }
            }]
        }))
        .expect("archive text");

        assert!(archive.contains("result snippet"));
        assert!(archive.contains("full page text from browser-discovered result"));
    }

    #[test]
    fn web_search_browser_fallback_engines_include_bing_retry() {
        assert_eq!(browser_fallback_engines(), ["duckduckgo", "bing"]);
    }

    #[test]
    fn web_search_policy_accepts_external_fallback_alias() {
        assert_eq!(
            WebSearchPolicy::parse(Some("native_then_external")),
            WebSearchPolicy::NativeThenExternal
        );
        assert_eq!(
            WebSearchPolicy::parse(Some("fallback")),
            WebSearchPolicy::NativeThenExternal
        );
        assert_eq!(
            WebSearchPolicy::parse(Some("external_only")),
            WebSearchPolicy::ExternalOnly
        );
    }

    #[test]
    fn web_search_schema_exposes_diagnose_on_failure() {
        let schema = WebSearchTool::new().parameters();
        assert!(schema["properties"].get("diagnose_on_failure").is_some());
    }

    #[test]
    fn native_only_failure_mentions_doctor_and_external_policy() {
        let message = format_native_only_failure(Some("all backends exhausted"));
        assert!(message.contains("searchxyz_doctor"));
        assert!(message.contains("SEARCHXYZ_SEARXNG_URL"));
        assert!(message.contains("diagnose_on_failure=false"));
        assert!(message.contains("search_policy=native_then_external"));
        assert!(message.contains("all backends exhausted"));
    }

    #[test]
    fn weak_native_results_trigger_merge_retry_for_multi_term_queries() {
        let results = vec![
            json!({"title":"Google Gemini","url":"https://gemini.google.com/app","snippet":"Google AI chat app"}),
            json!({"title":"Gemini Help","url":"https://support.google.com/gemini","snippet":"Help center"}),
            json!({"title":"Google AI Studio","url":"https://aistudio.google.com","snippet":"Build with Gemini"}),
        ];

        assert!(native_search_results_need_merge_retry(
            "sakana fugu pricing",
            &results
        ));
    }

    #[test]
    fn relevant_native_results_skip_merge_retry() {
        let results = vec![
            json!({"title":"Fugu Pricing - Sakana AI","url":"https://sakana.ai/fugu/pricing","snippet":"Pricing for Fugu subscriptions and usage tiers"}),
            json!({"title":"Fugu Docs","url":"https://sakana.ai/fugu/docs","snippet":"Sakana Fugu API docs"}),
            json!({"title":"Sakana AI","url":"https://sakana.ai","snippet":"Fugu is a multi-agent model"}),
        ];

        assert!(!native_search_results_need_merge_retry(
            "sakana fugu pricing",
            &results
        ));
    }

    #[test]
    fn native_rescue_returns_docs_for_tokio_rust_query() {
        let results = native_rescue_results("rust tokio async runtime");
        assert!(results.iter().any(|r| r["url"] == "https://docs.rs/tokio"));
        assert!(results
            .iter()
            .any(|r| r["url"] == "https://crates.io/crates/tokio"));
    }

    #[test]
    fn native_rescue_ignores_unknown_rust_query() {
        let results = native_rescue_results("rust totallyunknowncrate async runtime");
        assert!(results.is_empty());
    }
}
