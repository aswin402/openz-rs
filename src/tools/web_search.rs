use crate::tools::Tool;
use anyhow::{anyhow, Result};
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::{json, Value};

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

        if let Some(arr) = search_res.as_array() {
            if !arr.is_empty() {
                let results_str = arr
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
                let _ = crate::tools::shared_memory::archive_research_entry(
                    query,
                    &results_str,
                    "web_search",
                )
                .await;
            }
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

        // 0. Try SearchXyz Dispatcher (Local/stealth federated searches)
        let search_query = searchxyz::search::SearchQuery {
            query: query.to_string(),
            max_results: 10,
        };
        match crate::tools::searchxyz::get_server()
            .dispatcher
            .search(&search_query)
            .await
        {
            Ok(results) => {
                let mut search_results = Vec::new();
                for r in results {
                    search_results.push(json!({
                        "title": r.title,
                        "url": r.url,
                        "snippet": r.snippet
                    }));
                }
                if !search_results.is_empty() {
                    return Ok(Value::Array(search_results));
                }
            }
            Err(e) => {
                tracing::warn!(
                    "SearchXyz dispatcher query failed, falling back to other engines: {:?}",
                    e
                );
            }
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
            return Err(anyhow!("All web search backends (Tavily, Exa, DuckDuckGo, Mojeek) failed or returned no results."));
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

    #[tokio::test]
    async fn test_web_search_query() -> Result<()> {
        let tool = WebSearchTool::new();
        let args = json!({ "query": "Rust programming language" });
        let res = tool.call(&args).await;
        assert!(res.is_ok());
        let val = res.unwrap();
        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert!(!arr.is_empty(), "Search results should not be empty");
        Ok(())
    }
}
