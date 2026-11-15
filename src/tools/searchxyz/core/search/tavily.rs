use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use super::{SearchBackend, SearchQuery, SearchResult};
use crate::tools::searchxyz::core::error::SearchXyzError;

/// Tavily AI Search API backend.
pub struct TavilyBackend {
    client: Client,
    api_key: Option<String>,
    timeout_secs: u64,
}

impl TavilyBackend {
    pub fn new(client: Client, api_key: Option<String>, timeout_secs: u64) -> Self {
        Self {
            client,
            api_key: api_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()),
            timeout_secs: timeout_secs.max(1),
        }
    }
}

#[derive(Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    results: Vec<TavilyResultItem>,
}

#[derive(Deserialize)]
struct TavilyResultItem {
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
}

#[async_trait]
impl SearchBackend for TavilyBackend {
    fn name(&self) -> &str {
        "tavily"
    }

    fn is_available(&self) -> bool {
        self.api_key.is_some()
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
        let api_key = self
            .api_key
            .as_deref()
            .ok_or_else(|| SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: "Tavily API key not configured.".into(),
            })?;

        let mut payload = json!({
            "api_key": api_key,
            "query": query.query,
            "search_depth": "basic",
            "max_results": query.max_results.clamp(1, 20),
        });
        match query.include_domains.is_empty() {
            true => {}
            false => payload["include_domains"] = json!(query.include_domains),
        }
        match query.exclude_domains.is_empty() {
            true => {}
            false => payload["exclude_domains"] = json!(query.exclude_domains),
        }

        let resp = self
            .client
            .post("https://api.tavily.com/search")
            .timeout(Duration::from_secs(self.timeout_secs))
            .json(&payload)
            .send()
            .await
            .map_err(|e| SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: format!("Tavily API request failed: {e}"),
            })?;

        match resp.status().as_u16() {
            200 => {
                let body: TavilyResponse =
                    resp.json()
                        .await
                        .map_err(|e| SearchXyzError::SearchFailed {
                            query: query.query.clone(),
                            reason: format!("Failed to parse Tavily JSON: {e}"),
                        })?;
                let results = body
                    .results
                    .into_iter()
                    .filter_map(|item| {
                        let url = item.url?.trim().to_string();
                        let title = item.title.unwrap_or_default().trim().to_string();
                        match url.is_empty() || title.is_empty() {
                            true => None,
                            false => Some(SearchResult {
                                title,
                                url,
                                snippet: item.content.unwrap_or_default().trim().to_string(),
                                source: "tavily".into(),
                            }),
                        }
                    })
                    .take(query.max_results)
                    .collect();
                Ok(results)
            }
            429 => Err(SearchXyzError::RateLimited {
                provider: "tavily".into(),
                retry_after_secs: 60,
            }),
            status => Err(SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: format!("Tavily API returned HTTP {status}"),
            }),
        }
    }
}
