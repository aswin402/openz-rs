use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use super::{SearchBackend, SearchQuery, SearchResult};
use crate::tools::searchxyz::core::error::SearchXyzError;

/// Exa Neural/Auto Search API backend.
pub struct ExaBackend {
    client: Client,
    api_key: Option<String>,
    timeout_secs: u64,
}

impl ExaBackend {
    pub fn new(client: Client, api_key: Option<String>, timeout_secs: u64) -> Self {
        Self {
            client,
            api_key: api_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()),
            timeout_secs: timeout_secs.max(1),
        }
    }
}

#[derive(Deserialize)]
struct ExaResponse {
    #[serde(default)]
    results: Vec<ExaResultItem>,
}

#[derive(Deserialize)]
struct ExaResultItem {
    title: Option<String>,
    url: Option<String>,
    text: Option<String>,
}

#[async_trait]
impl SearchBackend for ExaBackend {
    fn name(&self) -> &str {
        "exa"
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
                reason: "Exa API key not configured.".into(),
            })?;

        let mut payload = json!({
            "query": query.query,
            "numResults": query.max_results.clamp(1, 20),
            "type": "auto",
            "contents": {
                "text": {
                    "maxCharacters": 500
                }
            }
        });
        match query.include_domains.is_empty() {
            true => {}
            false => payload["includeDomains"] = json!(query.include_domains),
        }
        match query.exclude_domains.is_empty() {
            true => {}
            false => payload["excludeDomains"] = json!(query.exclude_domains),
        }

        let resp = self
            .client
            .post("https://api.exa.ai/search")
            .timeout(Duration::from_secs(self.timeout_secs))
            .header("x-api-key", api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: format!("Exa API request failed: {e}"),
            })?;

        match resp.status().as_u16() {
            200 => {
                let body: ExaResponse =
                    resp.json()
                        .await
                        .map_err(|e| SearchXyzError::SearchFailed {
                            query: query.query.clone(),
                            reason: format!("Failed to parse Exa JSON: {e}"),
                        })?;
                let results = body
                    .results
                    .into_iter()
                    .filter_map(|item| {
                        let url = item.url?.trim().to_string();
                        let title = item
                            .title
                            .filter(|t| !t.trim().is_empty())
                            .unwrap_or_else(|| url.clone());
                        match url.is_empty() {
                            true => None,
                            false => Some(SearchResult {
                                title: title.trim().to_string(),
                                url,
                                snippet: item.text.unwrap_or_default().trim().to_string(),
                                source: "exa".into(),
                            }),
                        }
                    })
                    .take(query.max_results)
                    .collect();
                Ok(results)
            }
            429 => Err(SearchXyzError::RateLimited {
                provider: "exa".into(),
                retry_after_secs: 60,
            }),
            status => Err(SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: format!("Exa API returned HTTP {status}"),
            }),
        }
    }
}
