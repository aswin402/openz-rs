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
                    let results = rank_results(query, filter_results(query, results));
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

        let merged = rank_results(query, merged);
        Ok(SearchReport {
            results: merged.into_iter().take(query.max_results).collect(),
            attempts,
            mode: "merge".to_string(),
        })
    }
}

fn rank_results(query: &SearchQuery, mut results: Vec<SearchResult>) -> Vec<SearchResult> {
    let model = RankingModel::from_results(query, &results);
    results.sort_by(|a, b| {
        let score_b = score_result(&model, b);
        let score_a = score_result(&model, a);
        score_b
            .cmp(&score_a)
            .then_with(|| {
                a.title
                    .to_ascii_lowercase()
                    .cmp(&b.title.to_ascii_lowercase())
            })
            .then_with(|| a.url.cmp(&b.url))
    });
    results
}

#[derive(Debug, Clone)]
struct RankingTerm {
    text: String,
    weight: i32,
}

#[derive(Debug, Clone)]
struct RankingModel {
    terms: Vec<RankingTerm>,
    phrase: String,
    distinctive_term: Option<String>,
}

impl RankingModel {
    fn from_results(query: &SearchQuery, results: &[SearchResult]) -> Self {
        let raw_terms = query_terms(&query.query);
        let total = results.len().max(1) as i32;
        let terms = raw_terms
            .iter()
            .map(|term| {
                let df = results
                    .iter()
                    .filter(|result| result_contains_term(result, term))
                    .count() as i32;
                RankingTerm {
                    text: term.clone(),
                    weight: dynamic_term_weight(df, total),
                }
            })
            .collect::<Vec<_>>();
        let distinctive_term = terms
            .iter()
            .max_by(|a, b| {
                a.weight
                    .cmp(&b.weight)
                    .then_with(|| a.text.len().cmp(&b.text.len()))
            })
            .map(|term| term.text.clone());
        Self {
            phrase: raw_terms.join(" "),
            terms,
            distinctive_term,
        }
    }
}

fn score_result(model: &RankingModel, result: &SearchResult) -> i32 {
    if model.terms.is_empty() {
        return 0;
    }

    let title = result.title.to_ascii_lowercase();
    let snippet = result.snippet.to_ascii_lowercase();
    let url = result.url.to_ascii_lowercase();
    let mut score = 0;
    let mut matched_terms = 0;
    let mut matched_weight = 0;

    for term in &model.terms {
        let mut matched = false;
        if title.contains(&term.text) {
            score += 10 * term.weight;
            matched = true;
        }
        if snippet.contains(&term.text) {
            score += 4 * term.weight;
            matched = true;
        }
        if url.contains(&term.text) {
            score += 3 * term.weight;
            matched = true;
        }
        if matched {
            matched_terms += 1;
            matched_weight += term.weight;
        }
    }

    if phrase_contains(&title, &model.phrase) {
        score += 40;
    }
    if phrase_contains(&url, &model.phrase.replace(' ', "-")) {
        score += 28;
    }
    if phrase_contains(&snippet, &model.phrase) {
        score += 18;
    }

    score += coverage_bonus(matched_terms, model.terms.len(), matched_weight);
    score += distinctive_target_bonus(model, &title, &url);
    score += trusted_domain_bonus(&url);
    score -= generic_low_coverage_penalty(model, result, matched_terms);
    score -= low_quality_url_penalty(&url);
    score
}

fn result_contains_term(result: &SearchResult, term: &str) -> bool {
    result.title.to_ascii_lowercase().contains(term)
        || result.snippet.to_ascii_lowercase().contains(term)
        || result.url.to_ascii_lowercase().contains(term)
}

fn dynamic_term_weight(document_frequency: i32, total_results: i32) -> i32 {
    if document_frequency <= 1 {
        8
    } else if document_frequency * 3 <= total_results {
        6
    } else if document_frequency * 3 <= total_results * 2 {
        4
    } else {
        2
    }
}

fn coverage_bonus(matched_terms: usize, total_terms: usize, matched_weight: i32) -> i32 {
    if total_terms == 0 {
        return 0;
    }
    let coverage = matched_terms as i32 * 100 / total_terms as i32;
    match coverage {
        100 => 36 + matched_weight,
        67..=99 => 18 + matched_weight / 2,
        34..=66 => 6,
        _ => 0,
    }
}

fn distinctive_target_bonus(model: &RankingModel, title: &str, url: &str) -> i32 {
    let Some(term) = model.distinctive_term.as_deref() else {
        return 0;
    };
    let mut score = 0;
    if title.starts_with(term) {
        score += 24;
    }
    if url_domain_or_path_contains(url, term) {
        score += 22;
    }
    score
}

fn url_domain_or_path_contains(url: &str, term: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return url.contains(term);
    };
    parsed
        .host_str()
        .is_some_and(|host| host.to_ascii_lowercase().contains(term))
        || parsed.path().to_ascii_lowercase().contains(term)
}

fn generic_low_coverage_penalty(
    model: &RankingModel,
    result: &SearchResult,
    matched_terms: usize,
) -> i32 {
    if model.terms.len() < 3 || matched_terms > 1 {
        return 0;
    }
    let Ok(parsed) = url::Url::parse(&result.url) else {
        return 0;
    };
    let path = parsed.path().trim_matches('/');
    if path.is_empty() || matches!(path, "learn" | "docs" | "blog" | "articles") {
        28
    } else {
        0
    }
}

fn query_terms(query: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    query
        .split(|ch: char| !ch.is_alphanumeric())
        .map(str::trim)
        .filter(|term| term.len() >= 2)
        .map(str::to_ascii_lowercase)
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

fn phrase_contains(text: &str, phrase: &str) -> bool {
    !phrase.trim().is_empty() && text.contains(phrase)
}

fn trusted_domain_bonus(url: &str) -> i32 {
    let Ok(parsed) = url::Url::parse(url) else {
        return 0;
    };
    let host = parsed.host_str().unwrap_or_default();
    match host {
        host if host.ends_with(".edu") => 8,
        host if host.ends_with(".gov") => 8,
        "docs.rs" | "crates.io" | "github.com" | "arxiv.org" | "developer.mozilla.org" => 6,
        _ => 0,
    }
}

fn low_quality_url_penalty(url: &str) -> i32 {
    let lower = url.to_ascii_lowercase();
    let mut penalty = 0;
    let bad_path_markers = [
        "/search",
        "/tag/",
        "/tags/",
        "/login",
        "/signin",
        "/signup",
        "/account",
        "/category/",
        "/author/",
    ];
    for marker in bad_path_markers {
        if lower.contains(marker) {
            penalty += 18;
        }
    }

    let tracking_markers = ["utm_", "fbclid=", "gclid=", "mc_cid=", "ref="];
    for marker in tracking_markers {
        if lower.contains(marker) {
            penalty += 6;
        }
    }

    if lower.ends_with("/search") || lower.ends_with("/login") {
        penalty += 10;
    }
    penalty
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
        assert!(results
            .iter()
            .any(|r| r.url.starts_with("https://example.com/page")));
        assert!(results
            .iter()
            .any(|r| r.url == "https://rust-lang.org/learn"));
    }

    #[tokio::test]
    async fn ranking_prefers_title_phrase_match_over_weak_snippet_match() {
        let dispatcher = SearchDispatcher::new(vec![Box::new(StaticBackend {
            name: "a",
            results: vec![
                SearchResult {
                    title: "Generic article".to_string(),
                    url: "https://example.com/article".to_string(),
                    snippet: "tokio async runtime appears once".to_string(),
                    source: "a".to_string(),
                },
                SearchResult {
                    title: "Tokio Async Runtime Guide".to_string(),
                    url: "https://docs.rs/tokio".to_string(),
                    snippet: "reference docs".to_string(),
                    source: "a".to_string(),
                },
            ],
        })]);
        let query = SearchQuery::new("tokio async runtime", 10);

        let results = dispatcher.search(&query).await.unwrap();
        assert_eq!(results[0].title, "Tokio Async Runtime Guide");
    }

    #[tokio::test]
    async fn ranking_prefers_specific_topic_page_over_broad_language_pages() {
        let dispatcher = SearchDispatcher::new(vec![Box::new(StaticBackend {
            name: "a",
            results: vec![
                SearchResult {
                    title: "Rust Programming Language".to_string(),
                    url: "https://www.rust-lang.org/".to_string(),
                    snippet: "Rust is a programming language empowering everyone to build reliable software.".to_string(),
                    source: "a".to_string(),
                },
                SearchResult {
                    title: "Tokio - Asynchronous runtime for Rust".to_string(),
                    url: "https://tokio.rs/".to_string(),
                    snippet: "Tokio is an event-driven async runtime for building reliable network applications with Rust.".to_string(),
                    source: "a".to_string(),
                },
                SearchResult {
                    title: "tokio - docs.rs".to_string(),
                    url: "https://docs.rs/tokio".to_string(),
                    snippet: "API documentation for the tokio crate.".to_string(),
                    source: "a".to_string(),
                },
            ],
        })]);
        let query = SearchQuery::new("rust tokio async runtime", 10);

        let results = dispatcher.search(&query).await.unwrap();
        assert_ne!(results[0].url, "https://www.rust-lang.org/");
        assert!(results[0].url.contains("tokio"));
    }

    #[tokio::test]
    async fn ranking_prefers_specific_requested_page_for_non_rust_queries() {
        let dispatcher = SearchDispatcher::new(vec![Box::new(StaticBackend {
            name: "a",
            results: vec![
                SearchResult {
                    title: "Sakana AI".to_string(),
                    url: "https://sakana.ai/".to_string(),
                    snippet: "Sakana AI builds nature-inspired artificial intelligence systems."
                        .to_string(),
                    source: "a".to_string(),
                },
                SearchResult {
                    title: "Fugu Pricing - Sakana AI".to_string(),
                    url: "https://sakana.ai/fugu/pricing".to_string(),
                    snippet: "Pricing for Fugu subscriptions, plans, and usage tiers.".to_string(),
                    source: "a".to_string(),
                },
                SearchResult {
                    title: "Sakana AI Blog".to_string(),
                    url: "https://sakana.ai/blog".to_string(),
                    snippet: "Company updates and research articles.".to_string(),
                    source: "a".to_string(),
                },
            ],
        })]);
        let query = SearchQuery::new("sakana fugu pricing", 10);

        let results = dispatcher.search(&query).await.unwrap();
        assert_eq!(results[0].url, "https://sakana.ai/fugu/pricing");
    }

    #[tokio::test]
    async fn ranking_penalizes_search_login_and_tracking_urls() {
        let dispatcher = SearchDispatcher::new(vec![Box::new(StaticBackend {
            name: "a",
            results: vec![
                SearchResult {
                    title: "Tokio Async Runtime".to_string(),
                    url: "https://example.com/search?q=tokio&utm_source=x".to_string(),
                    snippet: "tokio async runtime".to_string(),
                    source: "a".to_string(),
                },
                SearchResult {
                    title: "Tokio Async Runtime".to_string(),
                    url: "https://example.com/guides/tokio-async-runtime".to_string(),
                    snippet: "tokio async runtime".to_string(),
                    source: "a".to_string(),
                },
            ],
        })]);
        let query = SearchQuery::new("tokio async runtime", 10);

        let results = dispatcher.search(&query).await.unwrap();
        assert_eq!(
            results[0].url,
            "https://example.com/guides/tokio-async-runtime"
        );
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
