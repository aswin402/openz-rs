use std::sync::Arc;

use tokio::task::JoinSet;

use crate::crawler::{Crawler, FetchCacheMode};
use crate::error::SearchXyzError;
use crate::extractor::{ExtractedContent, ExtractionPipeline};
use crate::index::SearchIndex;
use crate::search::{SearchDispatcher, SearchQuery};

/// Combined search → crawl → extract → index pipeline.
///
/// Used by the `search_and_read` MCP tool to execute the full
/// research workflow in one call.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PageAttempt {
    pub url: String,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchAndReadReport {
    pub pages: Vec<ExtractedContent>,
    pub search: crate::search::SearchReport,
    pub page_attempts: Vec<PageAttempt>,
}

pub struct SearchAndReadPipeline {
    dispatcher: Arc<SearchDispatcher>,
    crawler: Arc<Crawler>,
    extractor: Arc<ExtractionPipeline>,
    index: Arc<SearchIndex>,
}

impl SearchAndReadPipeline {
    pub fn new(
        dispatcher: Arc<SearchDispatcher>,
        crawler: Arc<Crawler>,
        extractor: Arc<ExtractionPipeline>,
        index: Arc<SearchIndex>,
    ) -> Self {
        Self {
            dispatcher,
            crawler,
            extractor,
            index,
        }
    }

    /// Run the full pipeline:
    /// 1. Search for `query`
    /// 2. Take the top `max_pages` result URLs
    /// 3. Crawl them in parallel
    /// 4. Extract content from each
    /// 5. Index all successful extractions
    /// 6. Return results (partial failures are tolerated)
    pub async fn run(
        &self,
        query: &str,
        max_pages: usize,
        render_js: bool,
    ) -> Result<Vec<ExtractedContent>, SearchXyzError> {
        Ok(self
            .run_with_options(
                SearchQuery::new(query, max_pages * 2),
                max_pages,
                render_js,
                FetchCacheMode::Auto,
                true,
            )
            .await?
            .pages)
    }

    pub async fn run_with_options(
        &self,
        search_query: SearchQuery,
        max_pages: usize,
        render_js: bool,
        cache_mode: FetchCacheMode,
        save: bool,
    ) -> Result<SearchAndReadReport, SearchXyzError> {
        // ── Step 1: Search ──

        let search_report = self
            .dispatcher
            .search_with_diagnostics(&search_query)
            .await?;
        let search_results = &search_report.results;

        if search_results.is_empty() {
            return Err(SearchXyzError::SearchFailed {
                query: search_query.query.clone(),
                reason: "No search results found".into(),
            });
        }

        // ── Step 2: Take top N URLs ──
        let urls: Vec<String> = search_results
            .iter()
            .take(max_pages)
            .map(|r| r.url.clone())
            .collect();

        tracing::info!(
            count = urls.len(),
            query = %search_query.query,
            "Crawling top search results in parallel"
        );

        // ── Step 3: Parallel crawl with JoinSet ──
        let mut join_set = JoinSet::new();

        for url in urls {
            let crawler = self.crawler.clone();
            let extractor = self.extractor.clone();

            join_set.spawn(async move {
                let attempted_url = url.clone();
                // Crawl the page.
                let fetch_result = crawler
                    .fetch_url_with_cache_mode(&url, render_js, cache_mode)
                    .await
                    .map_err(|e| (attempted_url.clone(), e))?;

                // Extract content.
                let content = extractor
                    .extract(&url, &fetch_result.body, Some(&fetch_result.content_type))
                    .map_err(|e| (attempted_url.clone(), e))?;

                Ok::<(String, ExtractedContent), (String, SearchXyzError)>((attempted_url, content))
            });
        }

        // ── Step 4: Collect results, tolerating partial failures ──
        let mut extracted: Vec<ExtractedContent> = Vec::new();
        let mut page_attempts: Vec<PageAttempt> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok((url, content))) => {
                    page_attempts.push(PageAttempt {
                        url,
                        status: "success".to_string(),
                        detail: None,
                    });
                    extracted.push(content);
                }
                Ok(Err((url, e))) => {
                    tracing::warn!(url = %url, error = %e, "Pipeline: one URL failed");
                    page_attempts.push(PageAttempt {
                        url,
                        status: "error".to_string(),
                        detail: Some(e.to_string()),
                    });
                    errors.push(e.to_string());
                }
                Err(join_err) => {
                    tracing::error!(error = %join_err, "Pipeline: task panicked");
                    page_attempts.push(PageAttempt {
                        url: "<task>".to_string(),
                        status: "panic".to_string(),
                        detail: Some(join_err.to_string()),
                    });
                    errors.push(format!("Task panicked: {join_err}"));
                }
            }
        }

        // ── Step 5: Index all successful extractions ──
        if save {
            for content in &extracted {
                if let Err(e) = self.index.add_document(content, "search_and_read").await {
                    // Indexing failure is non-fatal — log and continue.
                    tracing::warn!(
                        url = %content.url,
                        error = %e,
                        "Failed to index document (non-fatal)"
                    );
                }
            }
        }

        // ── Step 6: Return results ──
        if extracted.is_empty() {
            // All URLs failed — report all errors.
            return Err(SearchXyzError::SearchFailed {
                query: search_query.query.clone(),
                reason: format!(
                    "All pages failed to load or extract. Errors:\n{}",
                    errors.join("\n")
                ),
            });
        }

        if !errors.is_empty() {
            tracing::info!(
                succeeded = extracted.len(),
                failed = errors.len(),
                "Pipeline completed with partial failures"
            );
        }

        Ok(SearchAndReadReport {
            pages: extracted,
            search: search_report,
            page_attempts,
        })
    }
}
