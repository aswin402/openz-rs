use std::sync::Arc;

use schemars::JsonSchema;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::tools::searchxyz::core::cache::Cache;
use crate::tools::searchxyz::core::config::Config;
use crate::tools::searchxyz::core::crawler::{Crawler, FetchCacheMode as CrawlerCacheMode};
use crate::tools::searchxyz::core::diagnostics::{
    format_read_url_diagnostics, format_search_and_read_diagnostics, format_search_diagnostics,
};
use crate::tools::searchxyz::core::evidence::build_evidence_summary;
use crate::tools::searchxyz::core::extractor::{ExtractedContent, ExtractionPipeline};
use crate::tools::searchxyz::core::index::SearchIndex;
use crate::tools::searchxyz::core::pipeline::SearchAndReadPipeline;
use crate::tools::searchxyz::core::search::{SearchDispatcher, SearchQuery};

// ── MCP tool request parameter schemas ─────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SaveMode {
    Full,
    None,
}

impl SaveMode {
    fn should_save(self) -> bool {
        matches!(self, SaveMode::Full)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FetchCacheMode {
    Auto,
    PreferCache,
    Revalidate,
    Bypass,
}

impl From<FetchCacheMode> for crate::tools::searchxyz::core::crawler::FetchCacheMode {
    fn from(value: FetchCacheMode) -> Self {
        match value {
            FetchCacheMode::Auto => crate::tools::searchxyz::core::crawler::FetchCacheMode::Auto,
            FetchCacheMode::PreferCache => crate::tools::searchxyz::core::crawler::FetchCacheMode::PreferCache,
            FetchCacheMode::Revalidate => crate::tools::searchxyz::core::crawler::FetchCacheMode::Revalidate,
            FetchCacheMode::Bypass => crate::tools::searchxyz::core::crawler::FetchCacheMode::Bypass,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchWebRequest {
    #[schemars(description = "The search query string (e.g. 'rust async patterns')")]
    pub query: String,
    #[schemars(description = "Maximum number of results to return (default: 10, max: 20)")]
    pub max_results: Option<usize>,
    #[schemars(
        description = "Only keep results from these domains. Supports bare domains and subdomains."
    )]
    pub include_domains: Option<Vec<String>>,
    #[schemars(
        description = "Drop results from these domains. Supports bare domains and subdomains."
    )]
    pub exclude_domains: Option<Vec<String>>,
    #[schemars(
        description = "Query all available backends and deduplicate results instead of stopping at the first successful backend."
    )]
    pub merge_backends: Option<bool>,
    #[schemars(description = "Append compact backend diagnostics to the output.")]
    pub include_diagnostics: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadUrlRequest {
    #[schemars(description = "The full URL to fetch (must start with http:// or https://)")]
    pub url: String,
    #[schemars(
        description = "Crawl depth for recursive scoping. Defaults to 1 (only target URL). Max is 3."
    )]
    pub depth: Option<usize>,
    #[schemars(
        description = "Enable JavaScript rendering with a headless browser for dynamic or JS-heavy websites."
    )]
    pub render_js: Option<bool>,
    #[schemars(
        description = "Cache behavior: auto, prefer_cache, revalidate, or bypass. Revalidate currently bypasses the SearchXyz TTL cache."
    )]
    pub cache_mode: Option<FetchCacheMode>,
    #[schemars(
        description = "Persistence behavior: full indexes the page and graph; none returns content without saving."
    )]
    pub save_mode: Option<SaveMode>,
    #[schemars(description = "Append compact fetch/cache diagnostics to the output.")]
    pub include_diagnostics: Option<bool>,
    #[schemars(
        description = "Optional maximum characters to return; appends truncation metadata when exceeded."
    )]
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchAndReadRequest {
    #[schemars(description = "The search query string")]
    pub query: String,
    #[schemars(description = "How many top results to read (default: 3, max: 5)")]
    pub max_pages: Option<usize>,
    #[schemars(description = "Only keep search results from these domains before crawling.")]
    pub include_domains: Option<Vec<String>>,
    #[schemars(description = "Drop search results from these domains before crawling.")]
    pub exclude_domains: Option<Vec<String>>,
    #[schemars(
        description = "Query all available backends and deduplicate results before crawling."
    )]
    pub merge_backends: Option<bool>,
    #[schemars(
        description = "Enable JavaScript rendering with a headless browser for dynamic or JS-heavy websites."
    )]
    pub render_js: Option<bool>,
    #[schemars(
        description = "Cache behavior: auto, prefer_cache, revalidate, or bypass. Revalidate currently bypasses the SearchXyz TTL cache."
    )]
    pub cache_mode: Option<FetchCacheMode>,
    #[schemars(
        description = "Persistence behavior: full indexes pages and graph; none returns content without saving."
    )]
    pub save_mode: Option<SaveMode>,
    #[schemars(description = "Append compact search and page diagnostics to the output.")]
    pub include_diagnostics: Option<bool>,
    #[schemars(
        description = "Optional maximum characters to return; appends truncation metadata when exceeded."
    )]
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RecallRequest {
    #[schemars(description = "The search query for the local index")]
    pub query: String,
    #[schemars(description = "Max results (default: 5)")]
    pub max_results: Option<usize>,
    #[schemars(
        description = "Perform a semantic vector search instead of strict BM25 keyword matching (default: true)."
    )]
    pub semantic: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSourcesRequest {
    #[schemars(description = "Filter by indexing source (e.g. 'read_url', 'manual', 'spider')")]
    pub source: Option<String>,
    #[schemars(description = "Maximum number of results to return (default: 50, max: 100)")]
    pub limit: Option<usize>,
    #[schemars(description = "Offset for pagination (default: 0)")]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeepResearchRequest {
    #[schemars(description = "The research query or topic")]
    pub query: String,
    #[schemars(description = "Number of sub-queries to expand and execute (default: 3, max: 5)")]
    pub breadth: Option<usize>,
    #[schemars(description = "How many top pages to crawl per sub-query (default: 2, max: 4)")]
    pub max_pages_per_query: Option<usize>,
    #[schemars(description = "Only keep search results from these domains before crawling.")]
    pub include_domains: Option<Vec<String>>,
    #[schemars(description = "Drop search results from these domains before crawling.")]
    pub exclude_domains: Option<Vec<String>>,
    #[schemars(
        description = "Query all available backends and deduplicate results before crawling."
    )]
    pub merge_backends: Option<bool>,
    #[schemars(
        description = "Enable JavaScript rendering with a headless browser for dynamic or JS-heavy websites."
    )]
    pub render_js: Option<bool>,
    #[schemars(
        description = "Cache behavior: auto, prefer_cache, revalidate, or bypass. Revalidate currently bypasses the SearchXyz TTL cache."
    )]
    pub cache_mode: Option<FetchCacheMode>,
    #[schemars(
        description = "Persistence behavior: full indexes pages and graph; none returns content without saving."
    )]
    pub save_mode: Option<SaveMode>,
    #[schemars(description = "Append compact sub-query diagnostics to the output.")]
    pub include_diagnostics: Option<bool>,
    #[schemars(
        description = "Optional maximum characters to return; appends truncation metadata when exceeded."
    )]
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexContentRequest {
    #[schemars(description = "A URL or identifier for this content")]
    pub url: String,
    #[schemars(description = "Title for the content")]
    pub title: String,
    #[schemars(description = "The text content to index")]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SiteMapRequest {
    #[schemars(description = "The root URL or domain to map (e.g. 'https://example.com')")]
    pub url: String,
    #[schemars(description = "Try to locate and parse sitemap.xml (default: true)")]
    pub use_sitemap: Option<bool>,
    #[schemars(description = "Fallback to spider crawling of internal links (default: true)")]
    pub crawl_links: Option<bool>,
    #[schemars(
        description = "Maximum number of discovered links to return (default: 100, max: 500)"
    )]
    pub max_links: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexRelationshipRequest {
    #[schemars(description = "Source entity name (e.g. 'Tokio')")]
    pub source: String,
    #[schemars(description = "Source entity type/label (e.g. 'Library')")]
    pub source_type: String,
    #[schemars(description = "Target entity name (e.g. 'Rust')")]
    pub target: String,
    #[schemars(description = "Target entity type/label (e.g. 'Language')")]
    pub target_type: String,
    #[schemars(description = "Relationship type/verb (e.g. 'written_in', 'depends_on')")]
    pub relationship: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryGraphRequest {
    #[schemars(description = "The entity name to query (e.g. 'Rust')")]
    pub entity: String,
    #[schemars(description = "Max traversal depth (default: 2, max: 4)")]
    pub max_depth: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadGithubRepoRequest {
    #[schemars(
        description = "The GitHub repository URL (e.g. 'https://github.com/tokio-rs/tokio')"
    )]
    pub repo_url: String,
    #[schemars(
        description = "Optional branch name (e.g. 'master', 'main'). Defaults to the default branch."
    )]
    pub branch: Option<String>,
    #[schemars(
        description = "Optional list of file extensions to include (e.g. ['rs', 'md']). Defaults to standard code/text extensions."
    )]
    pub include_extensions: Option<Vec<String>>,
    #[schemars(
        description = "Optional list of folder/file paths to ignore. Defaults to standard ignore folders (target, node_modules, etc.)."
    )]
    pub exclude_paths: Option<Vec<String>>,
    #[schemars(
        description = "Maximum number of files to index from this repository (default 2000, max 10000)."
    )]
    pub max_files: Option<usize>,
    #[schemars(
        description = "Maximum total bytes to read from indexed repository files (default 20MB, max 200MB)."
    )]
    pub max_total_bytes: Option<u64>,
    #[schemars(description = "Git command timeout in seconds (default 60, max 600).")]
    pub git_timeout_secs: Option<u64>,
    #[schemars(
        description = "Optional maximum characters to return; appends truncation metadata when exceeded."
    )]
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportResearchRequest {
    #[schemars(
        description = "Optional query to filter exported documents. If omitted, all documents are exported."
    )]
    pub query: Option<String>,
    #[schemars(
        description = "Optional limit on how many documents to export (default 50, max 200)."
    )]
    pub limit: Option<usize>,
    #[schemars(
        description = "Optional maximum characters to return; appends truncation metadata when exceeded."
    )]
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImportResearchRequest {
    #[schemars(description = "The serialized JSON research bundle payload.")]
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteSourceRequest {
    #[schemars(
        description = "The source URL to delete from the search index and knowledge graph."
    )]
    pub url: String,
    #[schemars(description = "Must be true to confirm this destructive deletion.")]
    pub confirm: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClearIndexRequest {
    #[schemars(description = "Must be true to confirm wiping the full SearchXyz index and graph.")]
    pub confirm: Option<bool>,
}

#[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
pub struct ResearchBundle {
    pub version: String,
    pub exported_at: String,
    pub documents: Vec<ExtractedContent>,
    pub graph: crate::tools::searchxyz::core::graph::KnowledgeGraph,
}

fn limit_output(tool_name: &str, output: &str, max_chars: Option<usize>) -> String {
    let Some(max_chars) = max_chars else {
        return output.to_string();
    };

    let original_chars = output.chars().count();
    if original_chars <= max_chars {
        return output.to_string();
    }

    let mut end_byte = output.len();
    for (count, (idx, _)) in output.char_indices().enumerate() {
        if count == max_chars {
            end_byte = idx;
            break;
        }
    }

    let mut limited = output[..end_byte].to_string();
    limited.push_str(&format!(
        "

---
Output truncated by SearchXyz: tool={tool_name}, original_chars={original_chars}, returned_chars={max_chars}. Re-run with a higher max_chars value for more content."
    ));
    limited
}

// ─────────────────────────────────────────────────────────────
// MCP Search Server
// ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SearchXyzServer {
    pub dispatcher: Arc<SearchDispatcher>,
    pub crawler: Arc<Crawler>,
    pub extractor: Arc<ExtractionPipeline>,
    pub index: Arc<SearchIndex>,
    pub cache: Arc<Mutex<Cache>>,
    pub graph: Arc<Mutex<crate::tools::searchxyz::core::graph::KnowledgeGraph>>,
    pub config: Arc<Config>,
}

impl SearchXyzServer {
    pub fn new(
        dispatcher: SearchDispatcher,
        crawler: Crawler,
        extractor: ExtractionPipeline,
        index: SearchIndex,
        cache: Arc<Mutex<Cache>>,
        graph: Arc<Mutex<crate::tools::searchxyz::core::graph::KnowledgeGraph>>,
        config: Config,
    ) -> Self {
        Self {
            dispatcher: Arc::new(dispatcher),
            crawler: Arc::new(crawler),
            extractor: Arc::new(extractor),
            index: Arc::new(index),
            cache,
            graph,
            config: Arc::new(config),
        }
    }

    fn graph_path(&self) -> std::path::PathBuf {
        self.config.index.path.join("graph.json")
    }

    fn reload_index_nonfatal(&self, context: &str) {
        if let Err(e) = self.index.reload() {
            tracing::warn!(error = %e, context, "Failed to reload search index reader after mutation");
        }
    }

    async fn persist_graph_nonfatal(&self, context: &str) {
        let graph_path = self.graph_path();
        let graph = self.graph.lock().await;
        if let Err(e) = graph.save_to_file(&graph_path) {
            tracing::warn!(path = ?graph_path, error = %e, context, "Failed to persist SearchXyz graph");
        }
    }

    async fn persist_cache_nonfatal(&self, context: &str) {
        let cache = self.cache.lock().await;
        if let Err(e) = cache.save_to_file(&self.config.cache.path) {
            tracing::warn!(path = ?self.config.cache.path, error = %e, context, "Failed to persist SearchXyz cache");
        }
    }

    async fn persist_research_state_nonfatal(&self, context: &str) {
        self.reload_index_nonfatal(context);
        self.persist_graph_nonfatal(context).await;
        self.persist_cache_nonfatal(context).await;
    }

    pub async fn search_web(&self, req: SearchWebRequest) -> Result<String> {
        let max = req.max_results.unwrap_or(10).min(20);
        let mut search_query = SearchQuery::new(req.query.clone(), max);
        search_query.include_domains = req.include_domains.unwrap_or_default();
        search_query.exclude_domains = req.exclude_domains.unwrap_or_default();
        search_query.merge_backends = req.merge_backends.unwrap_or(false);

        let report = self
            .dispatcher
            .search_with_diagnostics(&search_query)
            .await?;
        let mut text = report
            .results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "{}. **{}**\n   {}\n   {}\n",
                    i + 1,
                    r.title,
                    r.url,
                    r.snippet
                )
            })
            .collect::<String>();
        if req.include_diagnostics.unwrap_or(false) {
            text.push_str(&format_search_diagnostics(&report));
        }

        Ok(text)
    }

    pub async fn read_url(&self, req: ReadUrlRequest) -> Result<String> {
        let url = &req.url;
        if !url.starts_with("http://") && !url.starts_with("https://") {
            bail!("URL must start with http:// or https://");
        }

        let depth = req.depth.unwrap_or(1).min(3);
        let render_js = req.render_js.unwrap_or(false);
        let cache_mode: CrawlerCacheMode = req.cache_mode.unwrap_or(FetchCacheMode::Auto).into();
        let save = req.save_mode.unwrap_or(SaveMode::Full).should_save();

        // ── Check for YouTube video URLs ──
        if crate::tools::searchxyz::core::crawler::youtube::extract_video_id(url).is_some() {
            let transcript =
                crate::tools::searchxyz::core::crawler::youtube::fetch_youtube_transcript(&self.crawler, url).await?;
            let title = format!("YouTube Video Transcript - {}", url);
            let extracted = ExtractedContent {
                url: url.clone(),
                title: title.clone(),
                description: String::new(),
                content_markdown: transcript.clone(),
                links: Vec::new(),
            };

            if save {
                // Index the transcript
                if let Err(e) = self.index.add_document(&extracted, "youtube").await {
                    tracing::warn!(url = %url, error = %e, "Failed to index YouTube transcript (non-fatal)");
                }

                // Run automatic graph heuristics
                {
                    let mut graph = self.graph.lock().await;
                    graph.extract_heuristics(url, &title, &transcript);
                }
                self.persist_research_state_nonfatal("youtube transcript")
                    .await;
            }

            let text = format!(
                "# {}\n\n**Source:** {}\n\n---\n\n{}",
                title, url, transcript
            );
            return Ok(limit_output("read_url", &text, req.max_chars));
        }

        // ── Check for GitHub repository URLs ──
        if crate::tools::searchxyz::core::crawler::github::parse_github_url(url).is_some() {
            if save {
                let summary = crate::tools::searchxyz::core::crawler::github::clone_and_index_repo(
                    &self.index,
                    &self.graph,
                    url,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;
                self.persist_research_state_nonfatal("github repo ingestion")
                    .await;
                let mut text = summary;
                if req.include_diagnostics.unwrap_or(false) {
                    text.push_str(&format_read_url_diagnostics(
                        url,
                        cache_mode,
                        save,
                        "github_repo_ingestion",
                    ));
                }
                return Ok(limit_output("read_url", &text, req.max_chars));
            }
            bail!("GitHub repository ingestion requires save_mode=full; use regular GitHub pages for no-save reads.");
        }

        if depth > 1 {
            let spider =
                crate::tools::searchxyz::core::crawler::spider::Spider::new(self.crawler.clone(), self.extractor.clone());
            let crawled_pages = spider.crawl(url, depth, render_js).await?;

            if save {
                // Index successful crawled pages
                for page in &crawled_pages {
                    if let Err(e) = self.index.add_document(page, "spider").await {
                        tracing::warn!(url = %page.url, error = %e, "Failed to index page from spider (non-fatal)");
                    }
                    // Run automatic graph heuristics
                    {
                        let mut graph = self.graph.lock().await;
                        graph.extract_heuristics(&page.url, &page.title, &page.content_markdown);
                    }
                }
                self.persist_research_state_nonfatal("spider crawl").await;
            }

            let mut text = crawled_pages
                .iter()
                .map(|p| {
                    format!(
                        "---\n## {}\n**Source:** {}\n\n{}\n\n",
                        p.title, p.url, p.content_markdown
                    )
                })
                .collect::<String>();
            if req.include_diagnostics.unwrap_or(false) {
                text.push_str(&format_read_url_diagnostics(
                    url,
                    cache_mode,
                    save,
                    "spider_crawl",
                ));
            }
            Ok(limit_output("read_url", &text, req.max_chars))
        } else {
            let fetch_result = self
                .crawler
                .fetch_url_with_cache_mode(url, render_js, cache_mode)
                .await?;
            let content = self.extractor.extract(
                url,
                &fetch_result.body,
                Some(&fetch_result.content_type),
            )?;

            if save {
                // Index the single crawled page too.
                if let Err(e) = self.index.add_document(&content, "read_url").await {
                    tracing::warn!(url = %content.url, error = %e, "Failed to index page from read_url (non-fatal)");
                }

                // Run automatic graph heuristics
                {
                    let mut graph = self.graph.lock().await;
                    graph.extract_heuristics(url, &content.title, &content.content_markdown);
                }
                self.persist_research_state_nonfatal("read_url").await;
            }

            let text = format!(
                "# {}\n\n**Source:** {}\n\n---\n\n{}",
                content.title, content.url, content.content_markdown
            );
            Ok(limit_output("read_url", &text, req.max_chars))
        }
    }

    pub async fn search_and_read(&self, req: SearchAndReadRequest) -> Result<String> {
        let max = req.max_pages.unwrap_or(3).min(5);
        let render_js = req.render_js.unwrap_or(false);
        let cache_mode: CrawlerCacheMode = req.cache_mode.unwrap_or(FetchCacheMode::Auto).into();
        let save = req.save_mode.unwrap_or(SaveMode::Full).should_save();
        let pipeline = SearchAndReadPipeline::new(
            self.dispatcher.clone(),
            self.crawler.clone(),
            self.extractor.clone(),
            self.index.clone(),
        );

        let mut search_query = SearchQuery::new(req.query.clone(), max * 2);
        search_query.include_domains = req.include_domains.unwrap_or_default();
        search_query.exclude_domains = req.exclude_domains.unwrap_or_default();
        search_query.merge_backends = req.merge_backends.unwrap_or(false);

        let report = pipeline
            .run_with_options(search_query, max, render_js, cache_mode, save)
            .await?;
        if save {
            {
                let mut graph = self.graph.lock().await;
                for result in &report.pages {
                    graph.extract_heuristics(&result.url, &result.title, &result.content_markdown);
                }
            }
            self.persist_research_state_nonfatal("search_and_read")
                .await;
        }
        let mut text = report
            .pages
            .iter()
            .map(|r| {
                format!(
                    "---\n## {}\n**Source:** {}\n\n{}\n\n",
                    r.title, r.url, r.content_markdown
                )
            })
            .collect::<String>();
        if req.include_diagnostics.unwrap_or(false) {
            text.push_str(&format_search_and_read_diagnostics(
                &report, cache_mode, save,
            ));
        }
        Ok(limit_output("search_and_read", &text, req.max_chars))
    }

    pub async fn recall(&self, req: RecallRequest) -> Result<String> {
        let max = req.max_results.unwrap_or(5);
        let use_semantic = req.semantic.unwrap_or(true);
        let results = if use_semantic {
            self.index.search_semantic(&req.query, max).await?
        } else {
            self.index.search(&req.query, max)?
        };
        if results.is_empty() {
            return Ok("No matching documents found in the local index. Try search_and_read to fetch new content first.".to_string());
        }
        let text = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "{}. **{}** (score: {:.2})\n   {}\n   {}\n\n",
                    i + 1,
                    r.title,
                    r.score,
                    r.url,
                    r.snippet
                )
            })
            .collect::<String>();
        Ok(text)
    }

    pub async fn list_sources(&self, req: ListSourcesRequest) -> Result<String> {
        let source_filter = req.source.as_deref();
        let limit = req.limit.unwrap_or(50).min(100);
        let offset = req.offset.unwrap_or(0);

        let (entries, total_count) = self.index.list_documents(source_filter, limit, offset)?;

        if entries.is_empty() {
            return Ok("No documents found in the local index matching your filters.".to_string());
        }

        let mut output = format!("### Cached Sources (Total indexed: {})\n\n", total_count);
        for (i, entry) in entries.iter().enumerate() {
            output.push_str(&format!(
                "{}. **{}**\n   - **URL:** {}\n   - **Indexed At:** {}\n   - **Source:** {}\n\n",
                offset + i + 1,
                entry.title,
                entry.url,
                entry.indexed_at,
                entry.source
            ));
        }

        Ok(output)
    }

    pub async fn deep_research(&self, req: DeepResearchRequest) -> Result<String> {
        let query = &req.query;
        let breadth = req.breadth.unwrap_or(3).min(5);
        let max_pages = req.max_pages_per_query.unwrap_or(2).min(4);
        let render_js = req.render_js.unwrap_or(false);
        let cache_mode: CrawlerCacheMode = req.cache_mode.unwrap_or(FetchCacheMode::Auto).into();
        let save = req.save_mode.unwrap_or(SaveMode::Full).should_save();

        // 1. Expand query.
        let expanded_queries = expand_query(query, breadth);

        // 2. Instantiate pipeline.
        let pipeline = SearchAndReadPipeline::new(
            self.dispatcher.clone(),
            self.crawler.clone(),
            self.extractor.clone(),
            self.index.clone(),
        );

        let mut output = format!("# Deep Research Dossier: {}\n\n", query);
        output.push_str(&format!("*Executed query expansion with breadth {}, crawling up to {} top pages per query.*\n\n", breadth, max_pages));

        // We can execute all pipelines concurrently.
        use futures_util::future::join_all;
        let mut futures = Vec::new();
        for q in &expanded_queries {
            let mut search_query = SearchQuery::new(q.clone(), max_pages * 2);
            search_query.include_domains = req.include_domains.clone().unwrap_or_default();
            search_query.exclude_domains = req.exclude_domains.clone().unwrap_or_default();
            search_query.merge_backends = req.merge_backends.unwrap_or(false);
            futures.push(pipeline.run_with_options(
                search_query,
                max_pages,
                render_js,
                cache_mode,
                save,
            ));
        }

        let results: Vec<_> = join_all(futures).await;

        let include_diagnostics = req.include_diagnostics.unwrap_or(false);
        let mut diagnostics = String::new();
        let mut all_pages = std::collections::HashMap::new();
        let mut executed_count = 0;

        for (i, res) in results.into_iter().enumerate() {
            let sub_q = &expanded_queries[i];
            match res {
                Ok(pages) => {
                    output.push_str(&format!("## Sub-Query: `{}`\n", sub_q));
                    if pages.pages.is_empty() {
                        output.push_str("   *No new pages crawled successfully.*\n\n");
                    } else {
                        output.push_str(&format!(
                            "   *Successfully retrieved {} pages.*\n\n",
                            pages.pages.len()
                        ));
                        if include_diagnostics {
                            diagnostics.push_str(&format!(
                                "
### Diagnostics for `{}`
{}",
                                sub_q,
                                format_search_and_read_diagnostics(&pages, cache_mode, save)
                            ));
                        }
                        for page in pages.pages {
                            // Avoid duplicate display by grouping/storing globally in a map.
                            all_pages.insert(page.url.clone(), page);
                        }
                        executed_count += 1;
                    }
                }
                Err(e) => {
                    output.push_str(&format!("## Sub-Query: `{}`\n", sub_q));
                    output.push_str(&format!("   *Failed to search/crawl: {}*\n\n", e));
                }
            }
        }

        if all_pages.is_empty() {
            return Ok(format!(
                "Deep Research failed to retrieve any results for the topic `{}`.",
                query
            ));
        }

        output.push_str(&format!("*Summary: Executed {} sub-queries successfully, retrieving a total of {} unique pages.*\n\n", executed_count, all_pages.len()));

        if save {
            {
                let mut graph = self.graph.lock().await;
                for page in all_pages.values() {
                    graph.extract_heuristics(&page.url, &page.title, &page.content_markdown);
                }
            }
            self.persist_research_state_nonfatal("deep_research").await;
        }

        let evidence_pages: Vec<_> = all_pages.values().cloned().collect();
        output.push_str(&build_evidence_summary(query, &evidence_pages));
        if include_diagnostics && !diagnostics.is_empty() {
            output.push_str("---\n## Diagnostics\n");
            output.push_str(&diagnostics);
            output.push('\n');
        }

        output.push_str("---\n## Compiled Research Documents\n\n");
        for (url, page) in all_pages {
            output.push_str(&format!(
                "### {}\n- **Source URL:** {}\n\n{}\n\n",
                page.title, url, page.content_markdown
            ));
        }

        Ok(limit_output("deep_research", &output, req.max_chars))
    }

    pub async fn index_content(&self, req: IndexContentRequest) -> Result<String> {
        let extracted = ExtractedContent {
            url: req.url.clone(),
            title: req.title.clone(),
            description: String::new(),
            content_markdown: req.content.clone(),
            links: Vec::new(),
        };

        self.index.add_document(&extracted, "manual").await?;

        // Run automatic graph heuristics
        {
            let mut graph = self.graph.lock().await;
            graph.extract_heuristics(&req.url, &req.title, &req.content);
        }
        self.persist_research_state_nonfatal("index_content").await;

        Ok(format!("Successfully indexed content for `{}`", req.url))
    }

    pub async fn site_map(&self, req: SiteMapRequest) -> Result<String> {
        let url = &req.url;
        if !url.starts_with("http://") && !url.starts_with("https://") {
            bail!("URL must start with http:// or https://");
        }

        let use_sitemap = req.use_sitemap.unwrap_or(true);
        let crawl_links = req.crawl_links.unwrap_or(true);
        let max_links = req.max_links.unwrap_or(100).min(500);

        let mut discovered_urls = std::collections::HashSet::new();

        if use_sitemap {
            let allowed_host = url::Url::parse(url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()));
            match crate::tools::searchxyz::core::crawler::sitemap::discover_sitemap_urls(&self.crawler, url).await {
                Ok(urls) => {
                    for u in urls {
                        if let Ok(parsed_u) = url::Url::parse(&u) {
                            if let Some(ref host) = allowed_host {
                                if parsed_u.host_str() == Some(host) {
                                    discovered_urls.insert(u);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(url, error = %e, "Sitemap discovery failed");
                }
            }
        }

        if crawl_links && (discovered_urls.is_empty() || discovered_urls.len() < max_links) {
            let spider = crate::tools::searchxyz::core::crawler::fast_spider::LinkSpider::new(self.crawler.clone());
            match spider.discover_links(url, max_links).await {
                Ok(urls) => {
                    for u in urls {
                        discovered_urls.insert(u);
                    }
                }
                Err(e) => {
                    tracing::warn!(url, error = %e, "Link spider crawling failed");
                }
            }
        }

        let mut urls: Vec<String> = discovered_urls.into_iter().collect();
        urls.sort();

        if urls.is_empty() {
            return Ok(format!("No pages could be discovered for URL: {}", url));
        }

        let mut output = format!("### Site Map for {}\n\n", url);
        output.push_str(&format!("Found {} pages:\n", urls.len()));
        for u in urls {
            output.push_str(&format!("- {}\n", u));
        }

        Ok(output)
    }

    pub async fn index_relationship(&self, req: IndexRelationshipRequest) -> Result<String> {
        {
            let mut graph = self.graph.lock().await;
            graph.add_edge(
                req.source.clone(),
                req.source_type.clone(),
                req.target.clone(),
                req.target_type.clone(),
                req.relationship.clone(),
            );
        }

        Ok(format!(
            "Successfully indexed relationship: **{}** ({}) -[{}]-> **{}** ({})",
            req.source, req.source_type, req.relationship, req.target, req.target_type
        ))
    }

    pub async fn query_graph(&self, req: QueryGraphRequest) -> Result<String> {
        let start = &req.entity;
        let depth = req.max_depth.unwrap_or(2).min(4);

        let (nodes, edges) = {
            let graph = self.graph.lock().await;
            graph.query_neighbors(start, depth)
        };

        if nodes.is_empty() {
            return Ok(format!(
                "Entity `{}` not found in the knowledge graph.",
                start
            ));
        }

        let mut output = format!(
            "### Knowledge Graph Query for `{}` (Depth: {})\n\n",
            start, depth
        );

        output.push_str("#### Entities:\n");
        for n in &nodes {
            output.push_str(&format!("- **{}** ({})\n", n.name, n.entity_type));
        }

        output.push_str("\n#### Connections:\n");
        if edges.is_empty() {
            output.push_str("No active connections found.\n");
        } else {
            for e in &edges {
                output.push_str(&format!(
                    "- **{}** -[{}]-> **{}**\n",
                    e.source, e.relationship_type, e.target
                ));
            }
        }

        Ok(output)
    }

    pub async fn read_github_repo(&self, req: ReadGithubRepoRequest) -> Result<String> {
        let include_exts = req.include_extensions.as_deref();
        let exclude_paths = req.exclude_paths.as_deref();
        let limits = crate::tools::searchxyz::core::crawler::github::GithubIngestLimits::from_options(
            req.max_files,
            req.max_total_bytes,
            req.git_timeout_secs,
        );
        let summary = crate::tools::searchxyz::core::crawler::github::clone_and_index_repo(
            &self.index,
            &self.graph,
            &req.repo_url,
            req.branch.as_deref(),
            include_exts,
            exclude_paths,
            Some(limits),
        )
        .await?;
        Ok(limit_output("read_github_repo", &summary, req.max_chars))
    }

    pub async fn export_research(&self, req: ExportResearchRequest) -> Result<String> {
        let limit = req.limit.unwrap_or(50).min(200);
        let documents = self.index.export_documents(req.query.as_deref(), limit)?;

        let graph = {
            let g = self.graph.lock().await;
            g.clone()
        };

        let bundle = ResearchBundle {
            version: "1.0".to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            documents,
            graph,
        };

        let json = serde_json::to_string_pretty(&bundle).map_err(|e| {
            anyhow!(format!("Failed to serialize research bundle: {}", e))
        })?;

        Ok(limit_output("export_research", &json, req.max_chars))
    }

    pub async fn import_research(&self, req: ImportResearchRequest) -> Result<String> {
        let bundle: ResearchBundle = serde_json::from_str(&req.payload).map_err(|e| {
            anyhow!(format!("Invalid research bundle payload: {}", e))
        })?;

        let doc_count = bundle.documents.len();

        for doc in &bundle.documents {
            // Index the document locally (this also generates local semantic vector embeddings!)
            if let Err(e) = self.index.add_document(doc, "imported").await {
                tracing::warn!(url = %doc.url, error = %e, "Failed to index imported document (non-fatal)");
            }
        }

        // Merge the imported nodes and edges into the local knowledge graph
        let mut graph_edges_count = 0;
        {
            let mut g = self.graph.lock().await;
            for edge in &bundle.graph.edges {
                // Find node types from bundle node map if available
                let source_type = bundle
                    .graph
                    .nodes
                    .get(&edge.source)
                    .map(|n| n.entity_type.clone())
                    .unwrap_or_else(|| "Concept".to_string());
                let target_type = bundle
                    .graph
                    .nodes
                    .get(&edge.target)
                    .map(|n| n.entity_type.clone())
                    .unwrap_or_else(|| "Concept".to_string());

                g.add_edge(
                    edge.source.clone(),
                    source_type,
                    edge.target.clone(),
                    target_type,
                    edge.relationship_type.clone(),
                );
                graph_edges_count += 1;
            }

            // Also merge any standalone nodes
            for (name, node) in &bundle.graph.nodes {
                g.add_node(name.clone(), node.entity_type.clone());
            }
        }

        self.reload_index_nonfatal("import_research");
        self.persist_graph_nonfatal("import_research").await;

        Ok(format!(
            "### Import Summary\n\n\
            - **Documents Imported:** {}\n\
            - **Knowledge Graph Connections Merged:** {}\n\n\
            Research bundle successfully imported and fully indexed locally for instant search and recall.",
            doc_count, graph_edges_count
        ))
    }

    pub async fn delete_source(&self, req: DeleteSourceRequest) -> Result<String> {
        if req.confirm != Some(true) {
            bail!("searchxyz_delete_source requires confirm=true");
        }
        self.index.delete_document(&req.url).await?;
        {
            let mut g = self.graph.lock().await;
            g.prune_node(&req.url);
        }
        self.reload_index_nonfatal("delete_source");
        self.persist_graph_nonfatal("delete_source").await;
        Ok(format!("Successfully deleted source `{}`", req.url))
    }

    pub async fn clear_index(&self, req: ClearIndexRequest) -> Result<String> {
        if req.confirm != Some(true) {
            bail!("searchxyz_clear_index requires confirm=true");
        }
        let mut writer = self.index.writer.lock().await;
        writer
            .delete_all_documents()
            .map_err(crate::tools::searchxyz::core::error::SearchXyzError::from)?;
        writer
            .commit()
            .map_err(crate::tools::searchxyz::core::error::SearchXyzError::from)?;
        {
            let mut g = self.graph.lock().await;
            g.clear();
        }
        self.reload_index_nonfatal("clear_index");
        self.persist_graph_nonfatal("clear_index").await;
        Ok("Successfully cleared search index and knowledge graph.".to_string())
    }
}

fn expand_query(query: &str, breadth: usize) -> Vec<String> {
    let modifiers = [
        "",
        "documentation libraries",
        "examples tutorials guide",
        "comparison review github",
        "advanced pattern best practices",
    ];

    let mut expanded = Vec::new();
    for (i, modif) in modifiers.iter().enumerate() {
        if i >= breadth {
            break;
        }
        if modif.is_empty() {
            expanded.push(query.to_string());
        } else {
            expanded.push(format!("{} {}", query, modif));
        }
    }
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::searchxyz::core::config::{Config, IndexConfig};
    use crate::tools::searchxyz::core::graph::KnowledgeGraph;
    use crate::tools::searchxyz::core::index::SearchIndex;
    use crate::tools::searchxyz::core::search::{SearchBackend, SearchQuery, SearchResult};
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct StaticBackend {
        url: String,
    }

    #[async_trait]
    impl SearchBackend for StaticBackend {
        fn name(&self) -> &str {
            "static"
        }

        fn is_available(&self) -> bool {
            true
        }

        async fn search(
            &self,
            _query: &SearchQuery,
        ) -> Result<Vec<SearchResult>, crate::tools::searchxyz::core::error::SearchXyzError> {
            Ok(vec![SearchResult {
                title: "Static test page".to_string(),
                url: self.url.clone(),
                snippet: "A local page about Rust and Tokio".to_string(),
                source: "static".to_string(),
            }])
        }
    }

    async fn serve_one_html_page(body: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0_u8; 1024];
            let _ = stream.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        format!("http://{addr}/test")
    }

    fn test_server_with_config(
        index: SearchIndex,
        cache: Arc<Mutex<crate::tools::searchxyz::core::cache::Cache>>,
        graph: Arc<Mutex<KnowledgeGraph>>,
        config: Config,
        dispatcher: crate::tools::searchxyz::core::search::SearchDispatcher,
    ) -> SearchXyzServer {
        SearchXyzServer::new(
            dispatcher,
            crate::tools::searchxyz::core::crawler::Crawler::new(
                config.crawler.clone(),
                crate::tools::searchxyz::core::config::HeadlessConfig::default(),
                crate::tools::searchxyz::core::config::ProxyConfig::default(),
                cache.clone(),
            ),
            crate::tools::searchxyz::core::extractor::ExtractionPipeline::new(crate::tools::searchxyz::core::config::ExtractorConfig::default()),
            index,
            cache,
            graph,
            config,
        )
    }

    #[tokio::test]
    pub async fn test_index_content_persists_graph_and_reload_makes_recall_immediate() {
        let test_dir =
            std::env::temp_dir().join(format!("searchxyz_test_persist_{}", rand::random::<u64>()));
        let _ = std::fs::remove_dir_all(&test_dir);

        let mut config = Config::default();
        config.index = IndexConfig {
            path: test_dir.clone(),
            writer_heap_bytes: 15_000_000,
            embedding: Default::default(),
        };
        config.cache.path = test_dir.join("cache.json");
        config.crawler.allow_private_network = true;

        let index = SearchIndex::open(&config.index).unwrap();
        let graph = Arc::new(Mutex::new(KnowledgeGraph::new()));
        let cache = Arc::new(Mutex::new(crate::tools::searchxyz::core::cache::Cache::new(10, 60)));
        let server = test_server_with_config(
            index,
            cache,
            graph,
            config,
            crate::tools::searchxyz::core::search::SearchDispatcher::new(vec![]),
        );

        server
            .index_content(IndexContentRequest {
                url: "https://example.com/rust-tokio".to_string(),
                title: "Rust Tokio Notes".to_string(),
                content: "Rust and Tokio power async AI agent search memory.".to_string(),
            })
            .await
            .unwrap();

        let results = server.index.search("Tokio", 5).unwrap();
        assert_eq!(
            results.len(),
            1,
            "indexed content should be searchable immediately"
        );

        let graph_path = test_dir.join("graph.json");
        let persisted = KnowledgeGraph::load_from_file(&graph_path).unwrap();
        assert!(
            persisted
                .nodes
                .contains_key("https://example.com/rust-tokio"),
            "graph mutations should be persisted to graph.json"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[tokio::test]
    pub async fn test_search_and_read_enriches_and_persists_graph() {
        let html = r#"
            <html>
              <head><title>Rust Tokio Research</title></head>
              <body><main>Rust and Tokio are useful for async search agents, crawling, and vector memory.</main></body>
            </html>
        "#;
        let url = serve_one_html_page(html).await;
        let test_dir = std::env::temp_dir().join(format!(
            "searchxyz_test_research_graph_{}",
            rand::random::<u64>()
        ));
        let _ = std::fs::remove_dir_all(&test_dir);

        let mut config = Config::default();
        config.index = IndexConfig {
            path: test_dir.clone(),
            writer_heap_bytes: 15_000_000,
            embedding: Default::default(),
        };
        config.cache.path = test_dir.join("cache.json");
        config.crawler.allow_private_network = true;

        let index = SearchIndex::open(&config.index).unwrap();
        let graph = Arc::new(Mutex::new(KnowledgeGraph::new()));
        let cache = Arc::new(Mutex::new(crate::tools::searchxyz::core::cache::Cache::new(10, 60)));
        let server = test_server_with_config(
            index,
            cache,
            graph,
            config,
            crate::tools::searchxyz::core::search::SearchDispatcher::new(vec![Box::new(StaticBackend {
                url: url.clone(),
            })]),
        );

        server
            .search_and_read(SearchAndReadRequest {
                query: "rust tokio".to_string(),
                max_pages: Some(1),
                include_domains: None,
                exclude_domains: None,
                merge_backends: None,
                render_js: Some(false),
                cache_mode: None,
                save_mode: None,
                include_diagnostics: None,
                max_chars: None,
            })
            .await
            .unwrap();

        let graph_path = test_dir.join("graph.json");
        let persisted = KnowledgeGraph::load_from_file(&graph_path).unwrap();
        assert!(
            persisted.nodes.contains_key(&url),
            "search_and_read should enrich and persist graph entries for fetched pages"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_limit_output_truncates_with_metadata() {
        let limited = limit_output("deep_research", "abcdef", Some(3));
        assert!(limited.starts_with("abc"));
        assert!(limited.contains("Output truncated by SearchXyz"));
        assert!(limited.contains("original_chars=6"));
        assert!(limited.contains("returned_chars=3"));
    }

    #[tokio::test]
    pub async fn test_export_import_research() {
        let test_dir =
            std::env::temp_dir().join(format!("searchxyz_test_tools_{}", rand::random::<u64>()));
        let _ = std::fs::remove_dir_all(&test_dir);

        let index_config = IndexConfig {
            path: test_dir.clone(),
            writer_heap_bytes: 15_000_000,
            embedding: Default::default(),
        };

        let index = SearchIndex::open(&index_config).unwrap();
        let graph = Arc::new(Mutex::new(KnowledgeGraph::new()));

        // Add dummy document
        let doc = ExtractedContent {
            url: "https://example.com/sharing".to_string(),
            title: "Sharing Content".to_string(),
            description: "".to_string(),
            content_markdown: "Shared research between agents is useful.".to_string(),
            links: vec![],
        };
        index.add_document(&doc, "manual").await.unwrap();
        index.reload().unwrap();

        // Add graph connection
        {
            let mut g = graph.lock().await;
            g.add_edge(
                "AgentA".to_string(),
                "Agent".to_string(),
                "AgentB".to_string(),
                "Agent".to_string(),
                "shares_with".to_string(),
            );
        }

        // Create server
        let cache = Arc::new(Mutex::new(crate::tools::searchxyz::core::cache::Cache::new(10, 60)));
        let server = SearchXyzServer::new(
            crate::tools::searchxyz::core::search::SearchDispatcher::new(vec![]),
            crate::tools::searchxyz::core::crawler::Crawler::new(
                crate::tools::searchxyz::core::config::CrawlerConfig::default(),
                crate::tools::searchxyz::core::config::HeadlessConfig::default(),
                crate::tools::searchxyz::core::config::ProxyConfig::default(),
                cache.clone(),
            ),
            crate::tools::searchxyz::core::extractor::ExtractionPipeline::new(crate::tools::searchxyz::core::config::ExtractorConfig::default()),
            index,
            cache,
            graph.clone(),
            Config::default(),
        );

        // Test export
        let json_payload = server
            .export_research(ExportResearchRequest {
                query: None,
                limit: None,
                max_chars: None,
            })
            .await
            .unwrap();

        // Verify json payload structure
        let bundle: ResearchBundle = serde_json::from_str(&json_payload).unwrap();
        assert_eq!(bundle.documents.len(), 1);
        assert_eq!(bundle.documents[0].title, "Sharing Content");
        assert_eq!(bundle.graph.edges.len(), 1);
        assert_eq!(bundle.graph.edges[0].relationship_type, "shares_with");

        // Clean database/graph for import test
        let clean_dir = std::env::temp_dir().join(format!(
            "searchxyz_test_tools_clean_{}",
            rand::random::<u64>()
        ));
        let _ = std::fs::remove_dir_all(&clean_dir);

        let clean_index_config = IndexConfig {
            path: clean_dir.clone(),
            writer_heap_bytes: 15_000_000,
            embedding: Default::default(),
        };
        let clean_index = SearchIndex::open(&clean_index_config).unwrap();
        let clean_graph = Arc::new(Mutex::new(KnowledgeGraph::new()));
        let clean_cache = Arc::new(Mutex::new(crate::tools::searchxyz::core::cache::Cache::new(10, 60)));

        let clean_server = SearchXyzServer::new(
            crate::tools::searchxyz::core::search::SearchDispatcher::new(vec![]),
            crate::tools::searchxyz::core::crawler::Crawler::new(
                crate::tools::searchxyz::core::config::CrawlerConfig::default(),
                crate::tools::searchxyz::core::config::HeadlessConfig::default(),
                crate::tools::searchxyz::core::config::ProxyConfig::default(),
                clean_cache.clone(),
            ),
            crate::tools::searchxyz::core::extractor::ExtractionPipeline::new(crate::tools::searchxyz::core::config::ExtractorConfig::default()),
            clean_index,
            clean_cache,
            clean_graph.clone(),
            Config::default(),
        );

        // Import payload
        let result = clean_server
            .import_research(ImportResearchRequest {
                payload: json_payload,
            })
            .await
            .unwrap();

        assert!(result.contains("Documents Imported:** 1"));
        assert!(result.contains("Knowledge Graph Connections Merged:** 1"));

        // Verify clean index has document
        clean_server.index.reload().unwrap();
        let list = clean_server.index.search("sharing", 5).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Sharing Content");

        // Verify clean graph has edges
        {
            let g = clean_graph.lock().await;
            assert_eq!(g.edges.len(), 1);
            assert_eq!(g.edges[0].source, "AgentA");
        }

        let _ = std::fs::remove_dir_all(&test_dir);
        let _ = std::fs::remove_dir_all(&clean_dir);
    }

    #[tokio::test]
    pub async fn test_db_maintenance_tools() {
        let test_dir = std::env::temp_dir().join(format!(
            "searchxyz_test_tools_maint_{}",
            rand::random::<u64>()
        ));
        let _ = std::fs::remove_dir_all(&test_dir);

        let index_config = IndexConfig {
            path: test_dir.clone(),
            writer_heap_bytes: 15_000_000,
            embedding: Default::default(),
        };

        let index = SearchIndex::open(&index_config).unwrap();
        let graph = Arc::new(Mutex::new(KnowledgeGraph::new()));

        // Add dummy document
        let doc1 = ExtractedContent {
            url: "https://example.com/doc1".to_string(),
            title: "Doc 1".to_string(),
            description: "".to_string(),
            content_markdown: "Rust programming language.".to_string(),
            links: vec![],
        };
        index.add_document(&doc1, "manual").await.unwrap();

        let doc2 = ExtractedContent {
            url: "https://example.com/doc2".to_string(),
            title: "Doc 2".to_string(),
            description: "".to_string(),
            content_markdown: "Python programming language.".to_string(),
            links: vec![],
        };
        index.add_document(&doc2, "manual").await.unwrap();
        index.reload().unwrap();

        // Add graph connections
        {
            let mut g = graph.lock().await;
            g.add_edge(
                "https://example.com/doc1".to_string(),
                "Document".to_string(),
                "Rust".to_string(),
                "Concept".to_string(),
                "mentions".to_string(),
            );
            g.add_edge(
                "https://example.com/doc2".to_string(),
                "Document".to_string(),
                "Python".to_string(),
                "Concept".to_string(),
                "mentions".to_string(),
            );
        }

        // Create server
        let cache = Arc::new(Mutex::new(crate::tools::searchxyz::core::cache::Cache::new(10, 60)));
        let server = SearchXyzServer::new(
            crate::tools::searchxyz::core::search::SearchDispatcher::new(vec![]),
            crate::tools::searchxyz::core::crawler::Crawler::new(
                crate::tools::searchxyz::core::config::CrawlerConfig::default(),
                crate::tools::searchxyz::core::config::HeadlessConfig::default(),
                crate::tools::searchxyz::core::config::ProxyConfig::default(),
                cache.clone(),
            ),
            crate::tools::searchxyz::core::extractor::ExtractionPipeline::new(crate::tools::searchxyz::core::config::ExtractorConfig::default()),
            index,
            cache,
            graph.clone(),
            Config::default(),
        );

        // Verify initial state
        assert_eq!(server.index.search("programming", 5).unwrap().len(), 2);
        {
            let g = server.graph.lock().await;
            assert_eq!(g.nodes.len(), 4); // doc1, doc2, Rust, Python
            assert_eq!(g.edges.len(), 2);
        }

        // Test delete_source requires explicit confirmation
        let denied_delete = server
            .delete_source(DeleteSourceRequest {
                url: "https://example.com/doc1".to_string(),
                confirm: None,
            })
            .await;
        assert!(denied_delete.is_err());

        // Test delete_source for doc1
        let delete_res = server
            .delete_source(DeleteSourceRequest {
                url: "https://example.com/doc1".to_string(),
                confirm: Some(true),
            })
            .await
            .unwrap();
        assert!(delete_res.contains("Successfully deleted source"));

        // Verify doc1 is gone, doc2 remains
        server.index.reload().unwrap();
        let search_res = server.index.search("programming", 5).unwrap();
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].url, "https://example.com/doc2");

        {
            let g = server.graph.lock().await;
            // doc1 node and its edges should be pruned
            assert!(!g.nodes.contains_key("https://example.com/doc1"));
            assert!(g.nodes.contains_key("https://example.com/doc2"));
            // The edge from doc1 to Rust should be gone, only edge from doc2 to Python remains
            assert_eq!(g.edges.len(), 1);
            assert_eq!(g.edges[0].source, "https://example.com/doc2");
        }

        // Test clear_index requires explicit confirmation
        let denied_clear = server
            .clear_index(ClearIndexRequest { confirm: None })
            .await;
        assert!(denied_clear.is_err());

        // Test clear_index
        let clear_res = server
            .clear_index(ClearIndexRequest {
                confirm: Some(true),
            })
            .await
            .unwrap();
        assert!(clear_res.contains("Successfully cleared search index"));

        // Verify index and graph are empty
        server.index.reload().unwrap();
        assert_eq!(server.index.search("programming", 5).unwrap().len(), 0);
        {
            let g = server.graph.lock().await;
            assert_eq!(g.nodes.len(), 0);
            assert_eq!(g.edges.len(), 0);
        }

        let _ = std::fs::remove_dir_all(&test_dir);
    }
}
