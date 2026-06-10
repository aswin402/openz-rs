use crate::tools::Tool;
use anyhow::{anyhow, Result};
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::{json, Value};

pub struct WebSearchTool {
    client: Client,
}

impl WebSearchTool {
    pub fn new() -> Self {
        WebSearchTool {
            client: Client::builder().use_rustls_tls().build().unwrap_or_default(),
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
        let query = arguments.get("query").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;

        // 1. Try Tavily Search API
        if let Ok(tavily_key) = std::env::var("TAVILY_API_KEY") {
            if !tavily_key.trim().is_empty() {
                let body = json!({
                    "api_key": tavily_key,
                    "query": query,
                    "search_depth": "basic",
                    "max_results": 5
                });
                let res = self.client.post("https://api.tavily.com/search")
                    .json(&body)
                    .send()
                    .await?;

                if res.status().is_success() {
                    let resp_json: Value = res.json().await?;
                    if let Some(results) = resp_json.get("results").and_then(|r| r.as_array()) {
                        let mut search_results = Vec::new();
                        for r in results {
                            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let url = r.get("url").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let snippet = r.get("content").and_then(|v| v.as_str()).unwrap_or_default().to_string();
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
                let res = self.client.post("https://api.exa.ai/search")
                    .header("x-api-key", exa_key)
                    .json(&body)
                    .send()
                    .await?;

                if res.status().is_success() {
                    let resp_json: Value = res.json().await?;
                    if let Some(results) = resp_json.get("results").and_then(|r| r.as_array()) {
                        let mut search_results = Vec::new();
                        for r in results {
                            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let url = r.get("url").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let snippet = r.get("text").and_then(|v| v.as_str()).unwrap_or_default().to_string();
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
        let res = self.client.get("https://html.duckduckgo.com/html/")
            .query(&[("q", query)])
            .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(anyhow!("Search request failed with HTTP {}", res.status()));
        }

        let html_content = res.text().await?;
        let document = Html::parse_document(&html_content);

        // Select search results
        let result_selector = Selector::parse(".result").map_err(|e| anyhow!("Invalid selector: {:?}", e))?;
        let title_selector = Selector::parse(".result__title .result__a").map_err(|e| anyhow!("Invalid selector: {:?}", e))?;
        let snippet_selector = Selector::parse(".result__snippet").map_err(|e| anyhow!("Invalid selector: {:?}", e))?;

        let mut search_results = Vec::new();

        for element in document.select(&result_selector) {
            let title = element.select(&title_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();

            let href = element.select(&title_selector)
                .next()
                .and_then(|e| e.value().attr("href"))
                .map(|s| s.to_string())
                .unwrap_or_default();

            let snippet = element.select(&snippet_selector)
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
}
