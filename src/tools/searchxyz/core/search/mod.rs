pub mod bing;
pub mod brave;
pub mod cooldown;
pub mod duckduckgo;
pub mod exa;
pub mod google;
pub mod mojeek;
pub mod searxng;
pub mod tavily;
pub mod websurfx;

use crate::tools::searchxyz::core::error::SearchXyzError;
use async_trait::async_trait;

// ─────────────────────────────────────────────────────────────
// Core data types
// ─────────────────────────────────────────────────────────────

/// Incoming search request.
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
    pub source: String,
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
    fn is_available(&self) -> bool;

    /// Execute a search query and return results.
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError>;
}

// ─────────────────────────────────────────────────────────────
// Dispatcher — tries backends in order or merges concurrently.
// ─────────────────────────────────────────────────────────────

pub struct SearchDispatcher {
    backends: Vec<Box<dyn SearchBackend>>,
}

impl SearchDispatcher {
    pub fn new(backends: Vec<Box<dyn SearchBackend>>) -> Self {
        Self { backends }
    }

    /// Run the query against backends in configured order (or concurrently when `merge_backends` is set).
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchXyzError> {
        Ok(self.search_with_diagnostics(query).await?.results)
    }

    pub async fn search_with_diagnostics(
        &self,
        query: &SearchQuery,
    ) -> Result<SearchReport, SearchXyzError> {
        match query.merge_backends {
            true => self.search_merged(query).await,
            false => self.search_first_success(query).await,
        }
    }

    fn probe_backend_when_all_cooling(&self) -> Option<String> {
        let available_names: Vec<&str> = self
            .backends
            .iter()
            .filter(|b| b.is_available())
            .map(|b| b.name())
            .collect();
        let all_cooling = !available_names.is_empty()
            && available_names
                .iter()
                .all(|name| cooldown::is_backend_cooling_down(name).is_some());
        match all_cooling {
            true => cooldown::shortest_cooldown_backend(&available_names),
            false => None,
        }
    }

    async fn search_first_success(
        &self,
        query: &SearchQuery,
    ) -> Result<SearchReport, SearchXyzError> {
        let mut tried: Vec<String> = Vec::new();
        let mut attempts = Vec::new();
        let probe_backend = self.probe_backend_when_all_cooling();

        for backend in &self.backends {
            match backend.is_available() {
                false => {
                    attempts.push(skipped_backend_attempt(backend.name()));
                    continue;
                }
                true => {}
            }

            let cooldown_skip = cooldown::is_backend_cooling_down(backend.name())
                .filter(|_| probe_backend.as_deref() != Some(backend.name()));
            match cooldown_skip {
                Some((kind, remaining_secs)) => {
                    attempts.push(cooldown_backend_attempt(
                        backend.name(),
                        kind,
                        remaining_secs,
                    ));
                    tried.push(format!("{} (cooldown: {})", backend.name(), kind.as_str()));
                    continue;
                }
                None => {}
            }

            tracing::info!(backend = backend.name(), query = %query.query, "Trying search backend");

            match backend.search(query).await {
                Ok(results) => {
                    let results = rank_results(query, filter_results(query, results));
                    match results.is_empty() {
                        false => {
                            cooldown::clear_backend_cooldown(backend.name());
                            tracing::info!(
                                backend = backend.name(),
                                count = results.len(),
                                "Search succeeded"
                            );
                            let results: Vec<_> =
                                results.into_iter().take(query.max_results).collect();
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
                        true => {}
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
                    let err_msg = err.to_string();
                    cooldown::record_transient_backend_error(backend.name(), &err_msg);
                    tracing::error!(backend = backend.name(), error = %err_msg, "Backend failed");
                    attempts.push(BackendAttempt {
                        backend: backend.name().to_string(),
                        status: "error".to_string(),
                        usable_results: 0,
                        detail: Some(err_msg.clone()),
                    });
                    tried.push(format!("{} ({})", backend.name(), err_msg));
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
        let mut pre_attempts: Vec<(usize, BackendAttempt)> = Vec::new();
        let mut runnable: Vec<(usize, &dyn SearchBackend)> = Vec::new();
        let probe_backend = self.probe_backend_when_all_cooling();

        for (slot, backend) in self.backends.iter().enumerate() {
            match backend.is_available() {
                false => {
                    pre_attempts.push((slot, skipped_backend_attempt(backend.name())));
                    continue;
                }
                true => {}
            }

            let cooldown_skip = cooldown::is_backend_cooling_down(backend.name())
                .filter(|_| probe_backend.as_deref() != Some(backend.name()));
            match cooldown_skip {
                Some((kind, remaining_secs)) => {
                    pre_attempts.push((
                        slot,
                        cooldown_backend_attempt(backend.name(), kind, remaining_secs),
                    ));
                    tried.push(format!("{} (cooldown: {})", backend.name(), kind.as_str()));
                }
                None => runnable.push((slot, backend.as_ref())),
            }
        }

        let futures = runnable.iter().map(|(_, backend)| {
            tracing::info!(backend = backend.name(), query = %query.query, "Querying search backend concurrently for merge");
            backend.search(query)
        });
        let outcomes = futures_util::future::join_all(futures).await;

        let mut merged: Vec<SearchResult> = Vec::new();
        let mut key_to_index: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut rrf_scores: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        let mut post_attempts: Vec<(usize, BackendAttempt)> = Vec::new();

        for ((slot, backend), outcome) in runnable.into_iter().zip(outcomes) {
            match outcome {
                Ok(raw_results) => {
                    let filtered = rank_results(query, filter_results(query, raw_results));
                    match filtered.is_empty() {
                        false => cooldown::clear_backend_cooldown(backend.name()),
                        true => {}
                    }
                    let usable_count = filtered.len();
                    for (rank, result) in filtered.into_iter().enumerate() {
                        let key = normalize_url_key(&result.url);
                        let rrf_delta = 60 / (rank as i32 + 10);
                        *rrf_scores.entry(key.clone()).or_insert(0) += rrf_delta;
                        match key_to_index.get(&key).copied() {
                            Some(existing_idx) => {
                                let existing = &mut merged[existing_idx];
                                match result.snippet.len() > existing.snippet.len() {
                                    true => existing.snippet = result.snippet,
                                    false => {}
                                }
                            }
                            None => {
                                key_to_index.insert(key, merged.len());
                                merged.push(result);
                            }
                        }
                    }
                    let status = match usable_count == 0 {
                        true => "empty",
                        false => "success",
                    };
                    post_attempts.push((
                        slot,
                        BackendAttempt {
                            backend: backend.name().to_string(),
                            status: status.to_string(),
                            usable_results: usable_count,
                            detail: None,
                        },
                    ));
                    tried.push(format!("{} ({} usable results)", backend.name(), usable_count));
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    cooldown::record_transient_backend_error(backend.name(), &err_msg);
                    tracing::error!(backend = backend.name(), error = %err_msg, "Backend failed during merge");
                    post_attempts.push((
                        slot,
                        BackendAttempt {
                            backend: backend.name().to_string(),
                            status: "error".to_string(),
                            usable_results: 0,
                            detail: Some(err_msg.clone()),
                        },
                    ));
                    tried.push(format!("{} ({})", backend.name(), err_msg));
                }
            }
        }

        pre_attempts.extend(post_attempts);
        pre_attempts.sort_by_key(|(slot, _)| *slot);
        let attempts: Vec<BackendAttempt> =
            pre_attempts.into_iter().map(|(_, attempt)| attempt).collect();

        match merged.is_empty() {
            true => Err(SearchXyzError::AllBackendsExhausted {
                query: query.query.clone(),
                backends_tried: tried.join(", "),
            }),
            false => {
                let merged = rank_merged_results(query, merged, &rrf_scores);
                Ok(SearchReport {
                    results: merged.into_iter().take(query.max_results).collect(),
                    attempts,
                    mode: "merge".to_string(),
                })
            }
        }
    }
}

fn skipped_backend_attempt(name: &str) -> BackendAttempt {
    tracing::debug!(backend = name, "Skipping unavailable backend");
    BackendAttempt {
        backend: name.to_string(),
        status: "skipped".to_string(),
        usable_results: 0,
        detail: Some("backend unavailable or missing configuration".to_string()),
    }
}

fn cooldown_backend_attempt(
    name: &str,
    kind: cooldown::SearchFailureKind,
    remaining_secs: u64,
) -> BackendAttempt {
    tracing::debug!(
        backend = name,
        kind = kind.as_str(),
        remaining_secs,
        "Skipping cooling-down search backend"
    );
    BackendAttempt {
        backend: name.to_string(),
        status: "cooldown".to_string(),
        usable_results: 0,
        detail: Some(format!(
            "backend cooling down ({}, {}s remaining)",
            kind.as_str(),
            remaining_secs
        )),
    }
}

pub(crate) fn choose_proxy_client<'a>(
    default_client: &'a reqwest::Client,
    clients: &'a Option<Vec<reqwest::Client>>,
) -> &'a reqwest::Client {
    use rand::seq::SliceRandom;
    clients
        .as_deref()
        .filter(|list| !list.is_empty())
        .and_then(|list| list.choose(&mut rand::thread_rng()))
        .unwrap_or(default_client)
}

pub(crate) fn extract_container_snippet(
    container: &scraper::ElementRef<'_>,
    snippet_selectors: &[scraper::Selector],
    title: &str,
) -> String {
    let selected = snippet_selectors.iter().find_map(|sel| {
        let el = container.select(sel).next()?;
        let text = el.text().collect::<Vec<_>>().join("").trim().to_string();
        (!text.is_empty()).then_some(text)
    });

    match selected {
        Some(text) => text,
        None => {
            let mut snippet = container
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .replace(title, "")
                .trim()
                .to_string();
            let _ = snippet.char_indices().nth(200).map(|(idx, _)| {
                snippet.truncate(idx);
                snippet.push_str("...");
            });
            snippet
        }
    }
}

pub(crate) fn find_ancestor_http_href(element: &scraper::ElementRef<'_>) -> Option<String> {
    let mut current = element.parent();
    for _ in 0..5 {
        let node = current?;
        let found = scraper::ElementRef::wrap(node)
            .filter(|el| el.value().name() == "a")
            .and_then(|el| el.value().attr("href"))
            .filter(|href| href.starts_with("http"))
            .map(str::to_string);
        match found {
            Some(url) => return Some(url),
            None => current = node.parent(),
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn parse_container_or_heading_results<F, G>(
    html_body: &str,
    max_results: usize,
    source: &str,
    container_selector: &str,
    title_selector: &str,
    snippet_selector_strs: &[&str],
    extract_container_url: F,
    normalize_fallback_url: G,
) -> Vec<SearchResult>
where
    F: Fn(&scraper::ElementRef<'_>, &scraper::ElementRef<'_>, &scraper::Selector) -> Option<String>,
    G: Fn(String) -> String,
{
    let document = scraper::Html::parse_document(html_body);
    let container_sel = scraper::Selector::parse(container_selector).unwrap();
    let containers: Vec<_> = document.select(&container_sel).collect();
    let title_sel = scraper::Selector::parse(title_selector).unwrap();

    match containers.is_empty() {
        false => {
            let link_sel = scraper::Selector::parse("a[href]").unwrap();
            let snippet_selectors: Vec<_> = snippet_selector_strs
                .iter()
                .map(|s| scraper::Selector::parse(s).unwrap())
                .collect();

            containers
                .into_iter()
                .filter_map(|container| {
                    let title_el = container.select(&title_sel).next()?;
                    let title = title_el
                        .text()
                        .collect::<Vec<_>>()
                        .join("")
                        .trim()
                        .to_string();
                    match title.is_empty() {
                        true => return None,
                        false => {}
                    }
                    let url = extract_container_url(&container, &title_el, &link_sel)?;
                    let snippet =
                        extract_container_snippet(&container, &snippet_selectors, &title);
                    Some(SearchResult {
                        title,
                        url,
                        snippet,
                        source: source.to_string(),
                    })
                })
                .take(max_results)
                .collect()
        }
        true => document
            .select(&title_sel)
            .filter_map(|heading_el| {
                let title = heading_el
                    .text()
                    .collect::<Vec<_>>()
                    .join("")
                    .trim()
                    .to_string();
                match title.is_empty() {
                    true => None,
                    false => {
                        let url = find_ancestor_http_href(&heading_el)?;
                        Some(SearchResult {
                            title,
                            url: normalize_fallback_url(url),
                            snippet: String::new(),
                            source: source.to_string(),
                        })
                    }
                }
            })
            .take(max_results)
            .collect(),
    }
}

pub(crate) async fn execute_html_search_with_headless_fallback(
    backend_label: &str,
    request: reqwest::RequestBuilder,
    fallback_base_url: &str,
    query: &SearchQuery,
    headless_enabled: bool,
    parse_fn: fn(&str, usize) -> Vec<SearchResult>,
) -> Result<Vec<SearchResult>, SearchXyzError> {
    let mut results = Vec::new();
    let mut http_failed = false;

    match request.send().await {
        Ok(resp) => match resp.status().is_success() {
            true => match resp.text().await {
                Ok(html_body) => {
                    results = parse_fn(&html_body, query.max_results);
                    match is_poisoned_scraper_serp(query, &results) {
                        true => {
                            tracing::warn!(
                                query = %query.query,
                                "{} returned bot-decoy irrelevant SERP over HTTP",
                                backend_label
                            );
                            results.clear();
                            http_failed = true;
                        }
                        false => {}
                    }
                }
                Err(_) => {
                    http_failed = true;
                }
            },
            false => {
                http_failed = true;
                tracing::warn!(
                    "{} HTTP request returned status {}",
                    backend_label,
                    resp.status()
                );
            }
        },
        Err(e) => {
            http_failed = true;
            tracing::warn!("{} HTTP request failed: {:?}", backend_label, e);
        }
    }

    match (http_failed || results.is_empty()) && headless_enabled {
        true => {
            let parsed_base = url::Url::parse(fallback_base_url);
            match parsed_base {
                Ok(mut url_parsed) => {
                    url_parsed.query_pairs_mut().append_pair("q", &query.query);
                    let search_url = url_parsed.to_string();
                    tracing::info!(
                        url = %search_url,
                        "{} raw HTTP blocked or empty, falling back to OpenZ BrowserBroker",
                        backend_label
                    );
                    match crate::tools::browser_broker::render_page_with_browser_broker(
                        &search_url,
                        15,
                    )
                    .await
                    {
                        Ok(rendered) => {
                            results = parse_fn(&rendered.html, query.max_results);
                            match is_poisoned_scraper_serp(query, &results) {
                                true => {
                                    results.clear();
                                    return Err(SearchXyzError::SearchFailed {
                                        query: query.query.clone(),
                                        reason: format!(
                                            "{} returned bot-decoy irrelevant SERP (blocked)",
                                            backend_label
                                        ),
                                    });
                                }
                                false => {}
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "{} BrowserBroker fallback failed: {:?}",
                                backend_label,
                                e
                            );
                        }
                    }
                }
                Err(_) => {}
            }
        }
        false => {}
    }

    match results.is_empty() {
        true => tracing::warn!(
            query = %query.query,
            "{} returned no parsable results",
            backend_label
        ),
        false => {}
    }

    Ok(results)
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

fn rank_merged_results(
    query: &SearchQuery,
    mut results: Vec<SearchResult>,
    rrf_scores: &std::collections::HashMap<String, i32>,
) -> Vec<SearchResult> {
    let model = RankingModel::from_results(query, &results);
    results.sort_by(|a, b| {
        let key_a = normalize_url_key(&a.url);
        let key_b = normalize_url_key(&b.url);
        let score_a = score_result(&model, a) + rrf_scores.get(&key_a).copied().unwrap_or(0);
        let score_b = score_result(&model, b) + rrf_scores.get(&key_b).copied().unwrap_or(0);
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
    match model.terms.is_empty() {
        true => return 0,
        false => {}
    }

    let title = result.title.to_ascii_lowercase();
    let snippet = result.snippet.to_ascii_lowercase();
    let url = result.url.to_ascii_lowercase();
    let mut score = 0;
    let mut matched_terms = 0;
    let mut matched_weight = 0;

    for term in &model.terms {
        let in_title = title.contains(&term.text);
        let in_snippet = snippet.contains(&term.text);
        let in_url = url.contains(&term.text);

        score += (in_title as i32) * 10 * term.weight
            + (in_snippet as i32) * 4 * term.weight
            + (in_url as i32) * 3 * term.weight;

        match in_title || in_snippet || in_url {
            true => {
                matched_terms += 1;
                matched_weight += term.weight;
            }
            false => {}
        }
    }

    score += (phrase_contains(&title, &model.phrase) as i32) * 40
        + (phrase_contains(&url, &model.phrase.replace(' ', "-")) as i32) * 28
        + (phrase_contains(&snippet, &model.phrase) as i32) * 18;

    score += coverage_bonus(matched_terms, model.terms.len(), matched_weight);
    score += distinctive_target_bonus(model, &title, &url);
    score += trusted_domain_bonus(&url);
    score -= generic_low_coverage_penalty(model, result, matched_terms);
    score -= low_quality_url_penalty(&url);
    score
}

pub(crate) fn is_poisoned_scraper_serp(query: &SearchQuery, results: &[SearchResult]) -> bool {
    let terms = query_terms(&query.query);
    !results.is_empty()
        && !terms.is_empty()
        && query.include_domains.is_empty()
        && results
            .iter()
            .all(|res| terms.iter().all(|term| !result_contains_term(res, term)))
}

fn result_contains_term(result: &SearchResult, term: &str) -> bool {
    result.title.to_ascii_lowercase().contains(term)
        || result.snippet.to_ascii_lowercase().contains(term)
        || result.url.to_ascii_lowercase().contains(term)
}

fn dynamic_term_weight(document_frequency: i32, total_results: i32) -> i32 {
    match (
        document_frequency <= 1,
        document_frequency * 3 <= total_results,
        document_frequency * 3 <= total_results * 2,
    ) {
        (true, _, _) => 8,
        (false, true, _) => 6,
        (false, false, true) => 4,
        (false, false, false) => 2,
    }
}

fn coverage_bonus(matched_terms: usize, total_terms: usize, matched_weight: i32) -> i32 {
    match total_terms == 0 {
        true => 0,
        false => {
            let coverage = matched_terms as i32 * 100 / total_terms as i32;
            match coverage {
                100 => 36 + matched_weight,
                67..=99 => 18 + matched_weight / 2,
                34..=66 => 6,
                _ => 0,
            }
        }
    }
}

fn distinctive_target_bonus(model: &RankingModel, title: &str, url: &str) -> i32 {
    match model.distinctive_term.as_deref() {
        None => 0,
        Some(term) => {
            (title.starts_with(term) as i32) * 24
                + (url_domain_or_path_contains(url, term) as i32) * 22
        }
    }
}

fn url_domain_or_path_contains(url: &str, term: &str) -> bool {
    match url::Url::parse(url) {
        Err(_) => url.contains(term),
        Ok(parsed) => {
            parsed
                .host_str()
                .is_some_and(|host| host.to_ascii_lowercase().contains(term))
                || parsed.path().to_ascii_lowercase().contains(term)
        }
    }
}

fn generic_low_coverage_penalty(
    model: &RankingModel,
    result: &SearchResult,
    matched_terms: usize,
) -> i32 {
    match model.terms.len() < 3 || matched_terms > 1 {
        true => 0,
        false => match url::Url::parse(&result.url) {
            Err(_) => 0,
            Ok(parsed) => {
                let path = parsed.path().trim_matches('/');
                match path.is_empty() || matches!(path, "learn" | "docs" | "blog" | "articles") {
                    true => 28,
                    false => 0,
                }
            }
        },
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
    let parsed = match url::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return 0,
    };
    let host = parsed.host_str().unwrap_or_default();
    match (
        host.ends_with(".edu") || host.ends_with(".gov"),
        matches!(
            host,
            "docs.rs" | "crates.io" | "github.com" | "arxiv.org" | "developer.mozilla.org"
        ),
    ) {
        (true, _) => 8,
        (false, true) => 6,
        (false, false) => 0,
    }
}

fn low_quality_url_penalty(url: &str) -> i32 {
    let lower = url.to_ascii_lowercase();
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
    let bad_count = bad_path_markers
        .into_iter()
        .filter(|marker| lower.contains(marker))
        .count() as i32;

    let tracking_markers = ["utm_", "fbclid=", "gclid=", "mc_cid=", "ref="];
    let tracking_count = tracking_markers
        .into_iter()
        .filter(|marker| lower.contains(marker))
        .count() as i32;

    let tail_penalty = ((lower.ends_with("/search") || lower.ends_with("/login")) as i32) * 10;
    bad_count * 18 + tracking_count * 6 + tail_penalty
}

pub fn filter_results(query: &SearchQuery, results: Vec<SearchResult>) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|result| {
            domain_allowed(&result.url, &query.include_domains, &query.exclude_domains)
        })
        .collect()
}

pub fn domain_allowed(url: &str, include_domains: &[String], exclude_domains: &[String]) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let host = match parsed.host_str().map(|h| h.to_ascii_lowercase()) {
        Some(h) => h,
        None => return false,
    };

    let excluded = !exclude_domains.is_empty()
        && exclude_domains.iter().any(|d| domain_matches(&host, d));
    match excluded {
        true => false,
        false => {
            include_domains.is_empty() || include_domains.iter().any(|d| domain_matches(&host, d))
        }
    }
}

fn domain_matches(host: &str, domain: &str) -> bool {
    let domain = domain.trim().trim_start_matches("*.").to_ascii_lowercase();
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn is_tracking_query_param(key_lower: &str) -> bool {
    key_lower.starts_with("utm_")
        || matches!(
            key_lower,
            "utm"
                | "fbclid"
                | "gclid"
                | "dclid"
                | "mc_cid"
                | "mc_eid"
                | "msclkid"
                | "yclid"
                | "igshid"
                | "ref"
                | "ref_src"
                | "si"
                | "_hsenc"
                | "_hsmi"
        )
}

pub fn normalize_url_key(url: &str) -> String {
    match url::Url::parse(url) {
        Err(_) => url.trim().to_ascii_lowercase(),
        Ok(mut parsed) => {
            parsed.set_fragment(None);
            let mut semantic_pairs: Vec<(String, String)> = parsed
                .query_pairs()
                .filter_map(|(k, v)| {
                    let key_lower = k.to_ascii_lowercase();
                    (!is_tracking_query_param(&key_lower)).then(|| (key_lower, v.into_owned()))
                })
                .collect();
            semantic_pairs.sort();
            match semantic_pairs.is_empty() {
                true => parsed.set_query(None),
                false => {
                    let mut qp = url::form_urlencoded::Serializer::new(String::new());
                    for (k, v) in &semantic_pairs {
                        qp.append_pair(k, v);
                    }
                    let encoded = qp.finish();
                    parsed.set_query(Some(&encoded));
                }
            }
            let rendered = parsed.to_string();
            match semantic_pairs.is_empty() {
                true => rendered.trim_end_matches('/').to_ascii_lowercase(),
                false => {
                    let host_normalized = match parsed.query() {
                        Some(q) => {
                            let mut base = parsed.clone();
                            base.set_query(None);
                            format!("{}?{}", base.to_string().trim_end_matches('/').to_ascii_lowercase(), q)
                        }
                        None => rendered.to_ascii_lowercase(),
                    };
                    host_normalized
                }
            }
        }
    }
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

    #[test]
    fn normalize_url_key_preserves_semantic_query_params_and_strips_tracking() {
        assert_ne!(
            normalize_url_key("https://www.youtube.com/watch?v=AbCdEf12345"),
            normalize_url_key("https://www.youtube.com/watch?v=ZyXwVu98765")
        );
        assert_eq!(
            normalize_url_key("https://www.youtube.com/watch?v=AbCdEf12345&utm_source=share&si=xyz"),
            normalize_url_key("https://www.youtube.com/watch?v=AbCdEf12345")
        );
        assert_eq!(
            normalize_url_key("https://example.com/docs?page=2&lang=en&fbclid=123"),
            normalize_url_key("https://example.com/docs/?lang=en&page=2")
        );
    }

    #[tokio::test]
    async fn dispatcher_skips_cooling_down_backend_and_probes_when_all_cooling() {
        let _guard = cooldown::COOLDOWN_TEST_LOCK.lock().unwrap();
        cooldown::clear_search_backend_cooldowns_for_tests();
        cooldown::record_search_backend_failure(
            &["cooldown_primary".to_string()],
            cooldown::SearchFailureKind::Blocked,
        );

        let dispatcher = SearchDispatcher::new(vec![
            Box::new(StaticBackend {
                name: "cooldown_primary",
                results: vec![result("https://primary.example/page", "cooldown_primary")],
            }),
            Box::new(StaticBackend {
                name: "healthy_secondary",
                results: vec![result("https://secondary.example/page", "healthy_secondary")],
            }),
        ]);
        let report = dispatcher
            .search_with_diagnostics(&SearchQuery::new("rust", 5))
            .await
            .unwrap();
        assert_eq!(report.attempts[0].backend, "cooldown_primary");
        assert_eq!(report.attempts[0].status, "cooldown");
        assert_eq!(report.attempts[1].backend, "healthy_secondary");
        assert_eq!(report.attempts[1].status, "success");

        // When every available backend is in cooldown, probe the shortest remaining cooldown instead of deadlocking.
        let single_dispatcher = SearchDispatcher::new(vec![Box::new(StaticBackend {
            name: "cooldown_primary",
            results: vec![result("https://primary.example/page", "cooldown_primary")],
        })]);
        let recovered = single_dispatcher
            .search_with_diagnostics(&SearchQuery::new("rust", 5))
            .await
            .unwrap();
        assert_eq!(recovered.results.len(), 1);
        assert!(cooldown::is_backend_cooling_down("cooldown_primary").is_none());
        cooldown::clear_search_backend_cooldowns_for_tests();
    }

    #[tokio::test]
    async fn merge_backends_records_error_attempts_and_boosts_consensus() {
        let dispatcher = SearchDispatcher::new(vec![
            Box::new(FailingBackend),
            Box::new(StaticBackend {
                name: "b1",
                results: vec![
                    result("https://example.com/solo", "b1"),
                    result("https://example.com/consensus", "b1"),
                ],
            }),
            Box::new(StaticBackend {
                name: "b2",
                results: vec![result("https://example.com/consensus?utm_source=b2", "b2")],
            }),
        ]);
        let mut query = SearchQuery::new("rust", 10);
        query.merge_backends = true;

        let report = dispatcher.search_with_diagnostics(&query).await.unwrap();
        assert_eq!(report.attempts.len(), 3);
        assert_eq!(report.attempts[0].status, "error");
        assert_eq!(report.results[0].url, "https://example.com/consensus");
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
