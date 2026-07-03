use std::sync::{Arc, OnceLock};
use anyhow::{anyhow, Result};
use searchxyz::{
    config::Config,
    cache::Cache,
    crawler::Crawler,
    extractor::ExtractionPipeline,
    index::SearchIndex,
    graph::KnowledgeGraph,
    search::{
        SearchDispatcher, SearchBackend,
        duckduckgo::DuckDuckGoBackend,
        google::GoogleBackend,
        bing::BingBackend,
        brave::BraveBackend,
        searxng::SearXngBackend,
    },
    tools::SearchXyzServer,
};

pub mod web;
pub mod index;
pub mod graph;

pub use web::{SearchXyzSearchWebTool, SearchXyzReadUrlTool, SearchXyzSearchAndReadTool, SearchXyzDeepResearchTool, SearchXyzSiteMapTool};
pub use index::{SearchXyzRecallTool, SearchXyzListSourcesTool, SearchXyzIndexContentTool, SearchXyzExportResearchTool, SearchXyzImportResearchTool, SearchXyzDeleteSourceTool, SearchXyzClearIndexTool};
pub use graph::{SearchXyzIndexRelationshipTool, SearchXyzQueryGraphTool, SearchXyzReadGithubRepoTool};

pub fn get_server() -> &'static SearchXyzServer {
    static SERVER: OnceLock<SearchXyzServer> = OnceLock::new();
    SERVER.get_or_init(|| {
        let config = Config::load(None).unwrap_or_default();
        let cache = Arc::new(tokio::sync::Mutex::new(Cache::load_from_file(
            config.cache.max_entries,
            config.cache.ttl_secs,
            &config.cache.path,
        )));
        
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.crawler.timeout_secs))
            .user_agent(&config.crawler.user_agent)
            .build()
            .unwrap();

        let mut backends: Vec<Box<dyn SearchBackend>> = Vec::new();
        for name in &config.search.backends {
            match name.as_str() {
                "duckduckgo" => {
                    backends.push(Box::new(DuckDuckGoBackend::new(http_client.clone())));
                }
                "google" => {
                    backends.push(Box::new(GoogleBackend::new(http_client.clone())));
                }
                "bing" => {
                    backends.push(Box::new(BingBackend::new(http_client.clone())));
                }
                "brave" => {
                    backends.push(Box::new(BraveBackend::new(http_client.clone(), config.brave.clone())));
                }
                "searxng" => {
                    backends.push(Box::new(SearXngBackend::new(http_client.clone(), config.searxng.clone())));
                }
                _ => {}
            }
        }
        
        let dispatcher = SearchDispatcher::new(backends);
        let crawler = Crawler::new(
            config.crawler.clone(),
            config.headless.clone(),
            config.proxy.clone(),
            cache.clone(),
        );
        let extractor = ExtractionPipeline::new(config.extractor.clone());
        let index = SearchIndex::open(&config.index).unwrap();
        
        let graph_path = std::path::Path::new(&config.index.path).join("graph.json");
        let graph = Arc::new(tokio::sync::Mutex::new(KnowledgeGraph::load_from_file(&graph_path).unwrap_or_else(|_| {
            KnowledgeGraph::new()
        })));
        
        SearchXyzServer::new(
            dispatcher,
            crawler,
            extractor,
            index,
            cache,
            graph,
            config,
        )
    })
}

pub fn map_mcp_err(err: rmcp::ErrorData) -> anyhow::Error {
    anyhow!("MCP Error {:?}: {}", err.code, err.message)
}
