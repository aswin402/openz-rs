use std::sync::Arc;

use anyhow::{bail, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::tools::searchxyz::core::config::Config;
use crate::tools::searchxyz::core::crawler::{Crawler, FetchCacheMode as CrawlerCacheMode};
use crate::tools::searchxyz::core::diagnostics::{
    format_read_url_diagnostics, format_search_and_read_diagnostics, format_search_diagnostics,
};
use crate::tools::searchxyz::core::evidence::build_evidence_summary;
use crate::tools::searchxyz::core::extractor::{ExtractedContent, ExtractionPipeline};
use crate::tools::searchxyz::core::graph::extract_and_persist_relations;
use crate::tools::searchxyz::core::index::SearchIndex;
use crate::tools::searchxyz::core::pipeline::SearchAndReadPipeline;
use crate::tools::searchxyz::core::search::{SearchDispatcher, SearchQuery};

// ── Request parameter schemas ──────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SaveMode {
    Full,
    None,
}

impl SaveMode {
    pub fn should_save(self) -> bool {
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

impl From<FetchCacheMode> for CrawlerCacheMode {
    fn from(value: FetchCacheMode) -> Self {
        match value {
            FetchCacheMode::Auto => CrawlerCacheMode::Auto,
            FetchCacheMode::PreferCache => CrawlerCacheMode::PreferCache,
            FetchCacheMode::Revalidate => CrawlerCacheMode::Revalidate,
            FetchCacheMode::Bypass => CrawlerCacheMode::Bypass,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchWebRequest {
    pub query: String,
    pub max_results: Option<usize>,
    pub include_domains: Option<Vec<String>>,
    pub exclude_domains: Option<Vec<String>>,
    pub merge_backends: Option<bool>,
    pub include_diagnostics: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadUrlRequest {
    pub url: String,
    pub depth: Option<usize>,
    pub render_js: Option<bool>,
    pub cache_mode: Option<FetchCacheMode>,
    pub save_mode: Option<SaveMode>,
    pub include_diagnostics: Option<bool>,
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchAndReadRequest {
    pub query: String,
    pub max_pages: Option<usize>,
    pub include_domains: Option<Vec<String>>,
    pub exclude_domains: Option<Vec<String>>,
    pub merge_backends: Option<bool>,
    pub render_js: Option<bool>,
    pub cache_mode: Option<FetchCacheMode>,
    pub save_mode: Option<SaveMode>,
    pub include_diagnostics: Option<bool>,
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeepResearchRequest {
    pub query: String,
    pub breadth: Option<usize>,
    pub max_pages_per_query: Option<usize>,
    pub include_domains: Option<Vec<String>>,
    pub exclude_domains: Option<Vec<String>>,
    pub merge_backends: Option<bool>,
    pub render_js: Option<bool>,
    pub cache_mode: Option<FetchCacheMode>,
    pub save_mode: Option<SaveMode>,
    pub include_diagnostics: Option<bool>,
    pub max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SiteMapRequest {
    pub url: String,
    pub use_sitemap: Option<bool>,
    pub crawl_links: Option<bool>,
    pub max_links: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadGithubRepoRequest {
    pub repo_url: String,
    pub branch: Option<String>,
    pub include_extensions: Option<Vec<String>>,
    pub exclude_paths: Option<Vec<String>>,
    pub max_files: Option<usize>,
    pub max_total_bytes: Option<u64>,
    pub git_timeout_secs: Option<u64>,
    pub max_chars: Option<usize>,
}

fn ensure_http_scheme(url: &str) -> Result<()> {
    match url.starts_with("http://") || url.starts_with("https://") {
        true => Ok(()),
        false => bail!("URL must start with http:// or https://"),
    }
}

fn format_crawled_pages_markdown(pages: &[ExtractedContent]) -> String {
    pages
        .iter()
        .map(|p| {
            format!(
                "---\n## {}\n**Source:** {}\n\n{}\n\n",
                p.title, p.url, p.content_markdown
            )
        })
        .collect()
}

pub(crate) fn limit_output(tool_name: &str, output: &str, max_chars: Option<usize>) -> String {
    let max_chars = match max_chars {
        Some(limit) => limit,
        None => return output.to_string(),
    };

    let original_chars = output.chars().count();
    match original_chars <= max_chars {
        true => return output.to_string(),
        false => {}
    }

    let end_byte = output
        .char_indices()
        .nth(max_chars)
        .map(|(idx, _)| idx)
        .unwrap_or(output.len());

    let mut limited = output[..end_byte].to_string();
    limited.push_str(&format!(
        "\n\n---\nOutput truncated by SearchXyz: tool={tool_name}, original_chars={original_chars}, returned_chars={max_chars}. Re-run with a higher max_chars value for more content."
    ));
    limited
}

// ─────────────────────────────────────────────────────────────
// Unified Native SearchXyz Engine Server
// ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SearchXyzServer {
    pub dispatcher: Arc<SearchDispatcher>,
    pub crawler: Arc<Crawler>,
    pub extractor: Arc<ExtractionPipeline>,
    pub index: Arc<SearchIndex>,
    pub config: Arc<Config>,
}

impl SearchXyzServer {
    pub fn new(
        dispatcher: SearchDispatcher,
        crawler: Crawler,
        extractor: ExtractionPipeline,
        index: SearchIndex,
        config: Config,
    ) -> Self {
        Self {
            dispatcher: Arc::new(dispatcher),
            crawler: Arc::new(crawler),
            extractor: Arc::new(extractor),
            index: Arc::new(index),
            config: Arc::new(config),
        }
    }

    async fn index_and_extract_nonfatal(&self, page: &ExtractedContent, source: &str) {
        match self.index.add_document(page, source).await {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(url = %page.url, source, error = %e, "Failed to index document (non-fatal)");
            }
        }
        let _ = extract_and_persist_relations(&page.url, &page.title, &page.content_markdown);
    }

    pub async fn search_web(&self, req: SearchWebRequest) -> Result<String> {
        let max = req.max_results.unwrap_or(self.config.search.max_results).min(20);
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
        match req.include_diagnostics.unwrap_or(false) {
            true => text.push_str(&format_search_diagnostics(&report)),
            false => {}
        }

        Ok(text)
    }

    pub async fn read_url(&self, req: ReadUrlRequest) -> Result<String> {
        let url = &req.url;
        ensure_http_scheme(url)?;

        let depth = req.depth.unwrap_or(1).clamp(1, 3);
        let render_js = req.render_js.unwrap_or(false);
        let cache_mode: CrawlerCacheMode = req.cache_mode.unwrap_or(FetchCacheMode::Auto).into();
        let save = req.save_mode.unwrap_or(SaveMode::Full).should_save();

        // 1. Check for YouTube video URLs
        match crate::tools::searchxyz::core::crawler::youtube::extract_video_id(url).is_some() {
            true => {
                let transcript =
                    crate::tools::searchxyz::core::crawler::youtube::fetch_youtube_transcript(
                        &self.crawler,
                        url,
                    )
                    .await?;
                let title = format!("YouTube Video Transcript - {}", url);
                let extracted = ExtractedContent {
                    url: url.clone(),
                    title: title.clone(),
                    description: String::new(),
                    content_markdown: transcript.clone(),
                    links: Vec::new(),
                };

                match save {
                    true => self.index_and_extract_nonfatal(&extracted, "youtube").await,
                    false => {}
                }

                let text = format!(
                    "# {}\n\n**Source:** {}\n\n---\n\n{}",
                    title, url, transcript
                );
                return Ok(limit_output("read_url", &text, req.max_chars));
            }
            false => {}
        }

        // 2. Check for GitHub repository root URLs when persistence is requested
        match crate::tools::searchxyz::core::crawler::github::parse_github_url(url).is_some() {
            true => match save {
                true => {
                    let limits =
                        crate::tools::searchxyz::core::crawler::github::GithubIngestLimits::from_options(
                            Some(self.config.github_max_files),
                            Some(self.config.github_max_bytes),
                            Some(self.config.github_clone_timeout_secs),
                        );
                    let mut text =
                        crate::tools::searchxyz::core::crawler::github::clone_and_index_repo(
                            &self.index,
                            url,
                            None,
                            None,
                            None,
                            Some(limits),
                        )
                        .await?;
                    match req.include_diagnostics.unwrap_or(false) {
                        true => {
                            text.push_str(&format_read_url_diagnostics(
                                url,
                                cache_mode,
                                save,
                                "github_repo_ingestion",
                            ));
                        }
                        false => {}
                    }
                    return Ok(limit_output("read_url", &text, req.max_chars));
                }
                false => {}
            },
            false => {}
        }

        match depth > 1 {
            true => {
                let spider = crate::tools::searchxyz::core::crawler::spider::Spider::new(
                    self.crawler.clone(),
                    self.extractor.clone(),
                );
                let crawled_pages = spider.crawl(url, depth, render_js).await?;

                match save {
                    true => {
                        for page in &crawled_pages {
                            self.index_and_extract_nonfatal(page, "spider").await;
                        }
                    }
                    false => {}
                }

                let mut text = format_crawled_pages_markdown(&crawled_pages);
                match req.include_diagnostics.unwrap_or(false) {
                    true => {
                        text.push_str(&format_read_url_diagnostics(
                            url,
                            cache_mode,
                            save,
                            "spider_crawl",
                        ));
                    }
                    false => {}
                }
                Ok(limit_output("read_url", &text, req.max_chars))
            }
            false => {
                let fetch_result = self
                    .crawler
                    .fetch_url_with_cache_mode(url, render_js, cache_mode)
                    .await?;
                let content = self.extractor.extract(
                    &fetch_result.url,
                    &fetch_result.body,
                    Some(&fetch_result.content_type),
                )?;

                match save {
                    true => self.index_and_extract_nonfatal(&content, "read_url").await,
                    false => {}
                }

                let text = format!(
                    "# {}\n\n**Source:** {}\n\n---\n\n{}",
                    content.title, content.url, content.content_markdown
                );
                Ok(limit_output("read_url", &text, req.max_chars))
            }
        }
    }

    pub async fn search_and_read(&self, req: SearchAndReadRequest) -> Result<String> {
        let max = req
            .max_pages
            .unwrap_or(self.config.auto_read_top_results_limit)
            .clamp(1, 5);
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
        match save {
            true => {
                for result in &report.pages {
                    let _ = extract_and_persist_relations(
                        &result.url,
                        &result.title,
                        &result.content_markdown,
                    );
                }
            }
            false => {}
        }
        let mut text = format_crawled_pages_markdown(&report.pages);
        match req.include_diagnostics.unwrap_or(false) {
            true => {
                text.push_str(&format_search_and_read_diagnostics(
                    &report, cache_mode, save,
                ));
            }
            false => {}
        }
        Ok(limit_output("search_and_read", &text, req.max_chars))
    }

    pub async fn deep_research(&self, req: DeepResearchRequest) -> Result<String> {
        let query = &req.query;
        let breadth = req.breadth.unwrap_or(3).clamp(1, 5);
        let max_pages = req.max_pages_per_query.unwrap_or(2).clamp(1, 4);
        let render_js = req.render_js.unwrap_or(false);
        let cache_mode: CrawlerCacheMode = req.cache_mode.unwrap_or(FetchCacheMode::Auto).into();
        let save = req.save_mode.unwrap_or(SaveMode::Full).should_save();

        let expanded_queries = expand_query(query, breadth);
        let pipeline = SearchAndReadPipeline::new(
            self.dispatcher.clone(),
            self.crawler.clone(),
            self.extractor.clone(),
            self.index.clone(),
        );

        let mut output = format!("# Deep Research Dossier: {}\n\n", query);
        output.push_str(&format!(
            "*Executed query expansion with breadth {}, crawling up to {} top pages per query.*\n\n",
            breadth, max_pages
        ));

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
                    match pages.pages.is_empty() {
                        true => {
                            output.push_str("   *No new pages crawled successfully.*\n\n");
                        }
                        false => {
                            output.push_str(&format!(
                                "   *Successfully retrieved {} pages.*\n\n",
                                pages.pages.len()
                            ));
                            match include_diagnostics {
                                true => {
                                    diagnostics.push_str(&format!(
                                        "\n### Diagnostics for `{}`\n{}",
                                        sub_q,
                                        format_search_and_read_diagnostics(&pages, cache_mode, save)
                                    ));
                                }
                                false => {}
                            }
                            for page in pages.pages {
                                all_pages.insert(page.url.clone(), page);
                            }
                            executed_count += 1;
                        }
                    }
                }
                Err(e) => {
                    output.push_str(&format!("## Sub-Query: `{}`\n", sub_q));
                    output.push_str(&format!("   *Failed to search/crawl: {}*\n\n", e));
                }
            }
        }

        match all_pages.is_empty() {
            true => {
                return Ok(format!(
                    "Deep Research failed to retrieve any results for the topic `{}`.",
                    query
                ));
            }
            false => {}
        }

        output.push_str(&format!(
            "*Summary: Executed {} sub-queries successfully, retrieving a total of {} unique pages.*\n\n",
            executed_count,
            all_pages.len()
        ));

        match save {
            true => {
                for page in all_pages.values() {
                    let _ = extract_and_persist_relations(
                        &page.url,
                        &page.title,
                        &page.content_markdown,
                    );
                }
            }
            false => {}
        }

        let evidence_pages: Vec<_> = all_pages.values().cloned().collect();
        output.push_str(&build_evidence_summary(query, &evidence_pages));
        match include_diagnostics && !diagnostics.is_empty() {
            true => {
                output.push_str("---\n## Diagnostics\n");
                output.push_str(&diagnostics);
                output.push('\n');
            }
            false => {}
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

    pub async fn site_map(&self, req: SiteMapRequest) -> Result<String> {
        let url = &req.url;
        ensure_http_scheme(url)?;

        let use_sitemap = req.use_sitemap.unwrap_or(true);
        let crawl_links = req.crawl_links.unwrap_or(true);
        let max_links = req.max_links.unwrap_or(100).min(500);

        let mut discovered_urls = std::collections::HashSet::new();

        match use_sitemap {
            true => {
                let allowed_host = url::Url::parse(url)
                    .ok()
                    .and_then(|u| u.host_str().map(|h| h.to_string()));
                match crate::tools::searchxyz::core::crawler::sitemap::discover_sitemap_urls(
                    &self.crawler,
                    url,
                )
                .await
                {
                    Ok(urls) => {
                        for u in urls {
                            let same_host = url::Url::parse(&u)
                                .ok()
                                .zip(allowed_host.as_deref())
                                .is_some_and(|(parsed_u, host)| parsed_u.host_str() == Some(host));
                            match same_host {
                                true => {
                                    discovered_urls.insert(u);
                                }
                                false => {}
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(url, error = %e, "Sitemap discovery failed");
                    }
                }
            }
            false => {}
        }

        match crawl_links && (discovered_urls.is_empty() || discovered_urls.len() < max_links) {
            true => {
                let spider =
                    crate::tools::searchxyz::core::crawler::fast_spider::LinkSpider::new(
                        self.crawler.clone(),
                    );
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
            false => {}
        }

        let mut urls: Vec<String> = discovered_urls.into_iter().collect();
        urls.sort();

        match urls.is_empty() {
            true => return Ok(format!("No pages could be discovered for URL: {}", url)),
            false => {}
        }

        let mut output = format!("### Site Map for {}\n\n", url);
        output.push_str(&format!("Found {} pages:\n", urls.len()));
        for u in urls.into_iter().take(max_links) {
            output.push_str(&format!("- {}\n", u));
        }

        Ok(output)
    }

    pub async fn read_github_repo(&self, req: ReadGithubRepoRequest) -> Result<String> {
        let include_exts = req.include_extensions.as_deref();
        let exclude_paths = req.exclude_paths.as_deref();
        let limits =
            crate::tools::searchxyz::core::crawler::github::GithubIngestLimits::from_options(
                Some(req.max_files.unwrap_or(self.config.github_max_files)),
                Some(req.max_total_bytes.unwrap_or(self.config.github_max_bytes)),
                Some(
                    req.git_timeout_secs
                        .unwrap_or(self.config.github_clone_timeout_secs),
                ),
            );
        let summary = crate::tools::searchxyz::core::crawler::github::clone_and_index_repo(
            &self.index,
            &req.repo_url,
            req.branch.as_deref(),
            include_exts,
            exclude_paths,
            Some(limits),
        )
        .await?;
        Ok(limit_output("read_github_repo", &summary, req.max_chars))
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

    modifiers
        .iter()
        .take(breadth)
        .map(|modif| match modif.is_empty() {
            true => query.to_string(),
            false => format!("{} {}", query, modif),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::searchxyz::core::config::{Config, IndexConfig};
    use crate::tools::searchxyz::core::index::SearchIndex;
    use crate::tools::searchxyz::core::search::{SearchBackend, SearchQuery, SearchResult};
    use async_trait::async_trait;

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
        config: Config,
        dispatcher: SearchDispatcher,
    ) -> SearchXyzServer {
        SearchXyzServer::new(
            dispatcher,
            Crawler::new(
                config.crawler.clone(),
                crate::tools::searchxyz::core::config::HeadlessConfig::default(),
                crate::tools::searchxyz::core::config::ProxyConfig::default(),
            ),
            ExtractionPipeline::new(
                crate::tools::searchxyz::core::config::ExtractorConfig::default(),
            ),
            index,
            config,
        )
    }

    #[tokio::test]
    async fn test_read_url_indexes_in_sqlite_and_enriches_graph_memory() {
        let html = r#"
            <html>
              <head><title>Rust Tokio Notes</title></head>
              <body><main>Rust and Tokio power async AI agent search memory with SQLite.</main></body>
            </html>
        "#;
        let url = serve_one_html_page(html).await;
        let test_dir =
            std::env::temp_dir().join(format!("searchxyz_test_persist_{}", rand::random::<u64>()));

        let mut config = Config::default();
        config.index = IndexConfig {
            path: test_dir,
            writer_heap_bytes: 0,
            embedding: Default::default(),
        };
        config.crawler.allow_private_network = true;

        let index = SearchIndex::open(&config.index).unwrap();
        let server = test_server_with_config(index, config, SearchDispatcher::new(vec![]));

        let rendered = server
            .read_url(ReadUrlRequest {
                url: url.clone(),
                depth: Some(1),
                render_js: Some(false),
                cache_mode: Some(FetchCacheMode::Bypass),
                save_mode: Some(SaveMode::Full),
                include_diagnostics: Some(false),
                max_chars: None,
            })
            .await
            .unwrap();
        assert!(rendered.contains("Rust Tokio Notes"));

        let results = server.index.search("Tokio", 5).unwrap();
        assert_eq!(
            results.len(),
            1,
            "fetched page should be immediately searchable in SQLite research_archive"
        );
    }

    #[tokio::test]
    async fn test_search_and_read_enriches_sqlite_index_and_graph() {
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

        let mut config = Config::default();
        config.index = IndexConfig {
            path: test_dir,
            writer_heap_bytes: 0,
            embedding: Default::default(),
        };
        config.crawler.allow_private_network = true;

        let index = SearchIndex::open(&config.index).unwrap();
        let server = test_server_with_config(
            index,
            config,
            SearchDispatcher::new(vec![Box::new(StaticBackend { url: url.clone() })]),
        );

        let text = server
            .search_and_read(SearchAndReadRequest {
                query: "rust tokio".to_string(),
                max_pages: Some(1),
                include_domains: None,
                exclude_domains: None,
                merge_backends: None,
                render_js: Some(false),
                cache_mode: Some(FetchCacheMode::Bypass),
                save_mode: Some(SaveMode::Full),
                include_diagnostics: None,
                max_chars: None,
            })
            .await
            .unwrap();

        assert!(text.contains("Rust Tokio Research"));
        let results = server.index.search("Tokio", 5).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_limit_output_truncates_with_metadata() {
        let limited = limit_output("deep_research", "abcdef", Some(3));
        assert!(limited.starts_with("abc"));
        assert!(limited.contains("Output truncated by SearchXyz"));
        assert!(limited.contains("original_chars=6"));
        assert!(limited.contains("returned_chars=3"));
    }
}
