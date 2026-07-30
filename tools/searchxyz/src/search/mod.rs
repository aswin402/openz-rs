pub mod bing;
pub mod brave;
pub mod duckduckgo;
pub mod google;
pub mod searxng;

use crate::error::SearchXyzError;
use async_trait::async_trait;

// ─────────────────────────────────────────────────────────────
// Core data types
// ─────────────────────────────────────────────────────────────

/// Incoming search request from an MCP tool call.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub query: String,
    pub max_results: usize,
    pub include_domains: Vec<String>,
    pub exclude_domains: Vec<String>,
    pub merge_backends: bool,
}

impl SearchQuery {
    pub fn new(query: impl Into<String>, max_results: usize) -> Self {
        Self {
            query: query.into(),
            max_results,
            include_domains: Vec::new(),
            exclude_domains: Vec::new(),
            merge_backends: false,
        }
    }
}

/// A single search result returned by any backend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String, // "duckduckgo", "brave", etc.
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackendAttempt {
    pub backend: String,
    pub status: String,
    pub usable_results: usize,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchReport {
    pub results: Vec<SearchResult>,
    pub attempts: Vec<BackendAttempt>,
    pub mode: String,
}

// ─────────────────────────────────────────────────────────────
// Backend trait — each search provider implements this.
// ─────────────────────────────────────────────────────────────

#[async_trait]
pub trait SearchBackend: Send + Sync {
    /// Human-readable backend name (for logs and error messages).
    fn name(&self) -> &str;

    /// Pre-flight check: is this backend configured and reachable?
    /// E.g. Brave returns false when no API key is set.
    fn is_available(&self) -> bool;

    /// Execute a search query and return results.
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError>;
}

// ─────────────────────────────────────────────────────────────
// Dispatcher — tries backends in order until one succeeds.
// ─────────────────────────────────────────────────────────────

pub struct SearchDispatcher {
    backends: Vec<Box<dyn SearchBackend>>,
}

impl SearchDispatcher {
    pub fn new(backends: Vec<Box<dyn SearchBackend>>) -> Self {
        Self { backends }
    }

    /// Run the query against backends in configured order.
    /// Defaults to first successful backend; `merge_backends` deduplicates and reranks all available results.
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
        Ok(self.search_with_diagnostics(query).await?.results)
    }

    pub async fn search_with_diagnostics(
        &self,
        query: &SearchQuery,
    ) -> Result<SearchReport, SearchXyzError> {
        if query.merge_backends {
            self.search_merged(query).await
        } else {
            self.search_first_success(query).await
        }
    }

    async fn search_first_success(
        &self,
        query: &SearchQuery,
    ) -> Result<SearchReport, SearchXyzError> {
        let mut tried: Vec<String> = Vec::new();
        let mut attempts = Vec::new();

        for backend in &self.backends {
            if !backend.is_available() {
                tracing::debug!(backend = backend.name(), "Skipping unavailable backend");
                attempts.push(BackendAttempt {
                    backend: backend.name().to_string(),
                    status: "skipped".to_string(),
                    usable_results: 0,
                    detail: Some("backend unavailable or missing configuration".to_string()),
                });
                continue;
            }

            tracing::info!(backend = backend.name(), query = %query.query, "Trying search backend");

            match backend.search(query).await {
                Ok(results) => {
                    let results = filter_results(query, results);
                    if !results.is_empty() {
                        tracing::info!(
                            backend = backend.name(),
                            count = results.len(),
                            "Search succeeded"
                        );
                        let results: Vec<_> = results.into_iter().take(query.max_results).collect();
                        attempts.push(BackendAttempt {
                            backend: backend.name().to_string(),
                            status: "success".to_string(),
                            usable_results: results.len(),
                            detail: None,
                        });
                        return Ok(SearchReport {
                            results,
                            attempts,
                            mode: "first_success".to_string(),
                        });
                    }
                    tracing::warn!(
                        backend = backend.name(),
                        "Backend returned zero usable results"
                    );
                    attempts.push(BackendAttempt {
                        backend: backend.name().to_string(),
                        status: "empty".to_string(),
                        usable_results: 0,
                        detail: Some(
                            "backend returned no usable results after filters".to_string(),
                        ),
                    });
                    tried.push(format!("{} (0 usable results)", backend.name()));
                }
                Err(err) => {
                    tracing::error!(backend = backend.name(), error = %err, "Backend failed");
                    attempts.push(BackendAttempt {
                        backend: backend.name().to_string(),
                        status: "error".to_string(),
                        usable_results: 0,
                        detail: Some(err.to_string()),
                    });
                    tried.push(format!("{} ({})", backend.name(), err));
                }
            }
        }

        Err(SearchXyzError::AllBackendsExhausted {
            query: query.query.clone(),
            backends_tried: tried.join(", "),
        })
    }

    async fn search_merged(&self, query: &SearchQuery) -> Result<SearchReport, SearchXyzError> {
        let mut tried: Vec<String> = Vec::new();
        let mut attempts = Vec::new();
        let mut merged = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for backend in &self.backends {
            if !backend.is_available() {
                tracing::debug!(backend = backend.name(), "Skipping unavailable backend");
                attempts.push(BackendAttempt {
                    backend: backend.name().to_string(),
                    status: "skipped".to_string(),
                    usable_results: 0,
                    detail: Some("backend unavailable or missing configuration".to_string()),
                });
                continue;
            }

            tracing::info!(backend = backend.name(), query = %query.query, "Trying search backend for merge");
            match backend.search(query).await {
                Ok(results) => {
                    let before = merged.len();
                    for result in filter_results(query, results) {
                        let key = normalize_url_key(&result.url);
                        if seen.insert(key) {
                            merged.push(result);
                        }
                    }
                    let usable_results = merged.len() - before;
                    attempts.push(BackendAttempt {
                        backend: backend.name().to_string(),
                        status: if usable_results == 0 {
                            "empty"
                        } else {
                            "success"
                        }
                        .to_string(),
                        usable_results,
                        detail: None,
                    });
                    tried.push(format!(
                        "{} ({} usable results)",
                        backend.name(),
                        usable_results
                    ));
                }
                Err(err) => {
                    tracing::error!(backend = backend.name(), error = %err, "Backend failed during merge");
                    tried.push(format!("{} ({})", backend.name(), err));
                }
            }
        }

        if merged.is_empty() {
            return Err(SearchXyzError::AllBackendsExhausted {
                query: query.query.clone(),
                backends_tried: tried.join(", "),
            });
        }

        Ok(SearchReport {
            results: merged.into_iter().take(query.max_results).collect(),
            attempts,
            mode: "merge".to_string(),
        })
    }
}

fn filter_results(query: &SearchQuery, results: Vec<SearchResult>) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|result| {
            domain_allowed(&result.url, &query.include_domains, &query.exclude_domains)
        })
        .collect()
}

fn domain_allowed(url: &str, include_domains: &[String], exclude_domains: &[String]) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str().map(|h| h.to_ascii_lowercase()) else {
        return false;
    };

    if !exclude_domains.is_empty() && exclude_domains.iter().any(|d| domain_matches(&host, d)) {
        return false;
    }
    include_domains.is_empty() || include_domains.iter().any(|d| domain_matches(&host, d))
}

fn domain_matches(host: &str, domain: &str) -> bool {
    let domain = domain.trim().trim_start_matches("*.").to_ascii_lowercase();
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn normalize_url_key(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.trim().to_ascii_lowercase();
    };
    parsed.set_fragment(None);
    parsed.set_query(None);
    parsed
        .to_string()
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct StaticBackend {
        name: &'static str,
        results: Vec<SearchResult>,
    }

    struct FailingBackend;

    #[async_trait]
    impl SearchBackend for StaticBackend {
        fn name(&self) -> &str {
            self.name
        }

        fn is_available(&self) -> bool {
            true
        }

        async fn search(&self, _query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
            Ok(self.results.clone())
        }
    }

    #[async_trait]
    impl SearchBackend for FailingBackend {
        fn name(&self) -> &str {
            "failing"
        }

        fn is_available(&self) -> bool {
            true
        }

        async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
            Err(SearchXyzError::SearchFailed {
                query: query.query.clone(),
                reason: "backend down".to_string(),
            })
        }
    }

    fn result(url: &str, source: &str) -> SearchResult {
        SearchResult {
            title: format!("Title {source}"),
            url: url.to_string(),
            snippet: "snippet".to_string(),
            source: source.to_string(),
        }
    }

    #[tokio::test]
    async fn first_success_report_records_failed_backend_attempts() {
        let dispatcher = SearchDispatcher::new(vec![
            Box::new(FailingBackend),
            Box::new(StaticBackend {
                name: "static",
                results: vec![result("https://example.com/page", "static")],
            }),
        ]);
        let query = SearchQuery::new("rust", 10);

        let report = dispatcher.search_with_diagnostics(&query).await.unwrap();
        assert_eq!(report.mode, "first_success");
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.attempts.len(), 2);
        assert_eq!(report.attempts[0].backend, "failing");
        assert_eq!(report.attempts[0].status, "error");
        assert_eq!(report.attempts[1].backend, "static");
        assert_eq!(report.attempts[1].status, "success");
    }

    #[tokio::test]
    async fn merge_backends_deduplicates_normalized_urls() {
        let dispatcher = SearchDispatcher::new(vec![
            Box::new(StaticBackend {
                name: "a",
                results: vec![result("https://example.com/page?utm=1", "a")],
            }),
            Box::new(StaticBackend {
                name: "b",
                results: vec![
                    result("https://example.com/page", "b"),
                    result("https://rust-lang.org/learn", "b"),
                ],
            }),
        ]);
        let mut query = SearchQuery::new("rust", 10);
        query.merge_backends = true;

        let results = dispatcher.search(&query).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "a");
        assert_eq!(results[1].url, "https://rust-lang.org/learn");
    }

    #[tokio::test]
    async fn domain_filters_apply_to_search_results() {
        let dispatcher = SearchDispatcher::new(vec![Box::new(StaticBackend {
            name: "a",
            results: vec![
                result("https://docs.rs/tokio", "a"),
                result("https://example.com/tokio", "a"),
            ],
        })]);
        let mut query = SearchQuery::new("tokio", 10);
        query.include_domains = vec!["docs.rs".to_string()];

        let results = dispatcher.search(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://docs.rs/tokio");
    }
}
