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

        // Query DuckDuckGo's HTML (no-JS) search
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
