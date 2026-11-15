use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use super::{SearchBackend, SearchQuery, SearchResult};
use crate::tools::searchxyz::core::error::SearchXyzError;

/// Websurfx metasearch JSON API backend.
pub struct WebsurfxBackend {
    client: Client,
    base_url: Option<String>,
    timeout_secs: u64,
}

impl WebsurfxBackend {
    pub fn new(client: Client, base_url: Option<String>, timeout_secs: u64) -> Self {
        Self {
            client,
            base_url: base_url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty()),
            timeout_secs: timeout_secs.max(1),
        }
    }
}

#[derive(Deserialize)]
struct WebsurfxResponse {
    #[serde(default)]
    results: Vec<WebsurfxResult>,
}

#[derive(Deserialize)]
struct WebsurfxResult {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
    content: Option<String>,
    snippet: Option<String>,
}

#[async_trait]
impl SearchBackend for WebsurfxBackend {
    fn name(&self) -> &str {
        "websurfx"
    }

    fn is_available(&self) -> bool {
        self.base_url.is_some()
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
        let base_url = self
            .base_url
            .as_deref()
            .ok_or_else(|| SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: "Websurfx URL not configured.".into(),
            })?
            .trim_end_matches('/');

        let resp = self
            .client
            .get(format!("{base_url}/search"))
            .timeout(Duration::from_secs(self.timeout_secs))
            .header("Accept", "application/json")
            .query(&[("q", query.query.as_str()), ("format", "json")])
            .send()
            .await
            .map_err(|e| SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: format!("Failed to reach Websurfx instance: {e}"),
            })?;

        match resp.status().is_success() {
            true => {}
            false => {
                return Err(SearchXyzError::SearchFailed {
                    query: query.query.clone(),
                    reason: format!("Websurfx returned HTTP {}", resp.status()),
                });
            }
        }

        let body: WebsurfxResponse =
            resp.json()
                .await
                .map_err(|e| SearchXyzError::SearchFailed {
                    query: query.query.clone(),
                    reason: format!("Failed to parse Websurfx JSON response: {e}"),
                })?;

        let results = body
            .results
            .into_iter()
            .filter_map(|item| {
                let url = item.url?.trim().to_string();
                let title = item.title.unwrap_or_default().trim().to_string();
                match url.is_empty() || title.is_empty() {
                    true => None,
                    false => {
                        let snippet = item
                            .description
                            .or(item.content)
                            .or(item.snippet)
                            .unwrap_or_default()
                            .trim()
                            .to_string();
                        Some(SearchResult {
                            title,
                            url,
                            snippet,
                            source: "websurfx".into(),
                        })
                    }
                }
            })
            .take(query.max_results)
            .collect();

        Ok(results)
    }
}
