use anyhow::anyhow;
use searchxyz::{
    cache::Cache,
    config::Config,
    crawler::Crawler,
    extractor::ExtractionPipeline,
    graph::KnowledgeGraph,
    index::SearchIndex,
    search::{
        bing::BingBackend, brave::BraveBackend, duckduckgo::DuckDuckGoBackend,
        google::GoogleBackend, searxng::SearXngBackend, SearchBackend, SearchDispatcher,
    },
    tools::SearchXyzServer,
};
use std::sync::{Arc, OnceLock};

pub mod graph;
pub mod index;
pub mod web;

pub use graph::{
    SearchXyzIndexRelationshipTool, SearchXyzQueryGraphTool, SearchXyzReadGithubRepoTool,
};
pub use index::{
    SearchXyzClearIndexTool, SearchXyzDeleteSourceTool, SearchXyzExportResearchTool,
    SearchXyzImportResearchTool, SearchXyzIndexContentTool, SearchXyzListSourcesTool,
    SearchXyzRecallTool,
};
pub use web::{
    SearchXyzDeepResearchTool, SearchXyzReadUrlTool, SearchXyzSearchAndReadTool,
    SearchXyzSearchWebTool, SearchXyzSiteMapTool,
};

fn openz_config_dir() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("OPENZ_CONFIG_DIR") {
        return std::path::PathBuf::from(path);
    }

    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".openz")
}

fn apply_openz_embedded_paths(config: &mut Config) {
    let defaults = Config::default();
    let base = openz_config_dir().join("searchxyz");

    if std::env::var_os("SEARCHXYZ_INDEX_PATH").is_none()
        && config.index.path == defaults.index.path
    {
        config.index.path = base.join("index");
    }

    if std::env::var_os("SEARCHXYZ_CACHE_PATH").is_none()
        && config.cache.path == defaults.cache.path
    {
        config.cache.path = base.join("cache.json");
    }
}

pub fn get_server() -> &'static SearchXyzServer {
    static SERVER: OnceLock<SearchXyzServer> = OnceLock::new();
    SERVER.get_or_init(|| {
        let mut config = Config::load(None).unwrap_or_default();
        apply_openz_embedded_paths(&mut config);
        if cfg!(test) {
            let temp_dir =
                std::env::temp_dir().join(format!("searchxyz_test_index_{}", uuid::Uuid::new_v4()));
            config.index.path = temp_dir.clone();
            config.cache.path = temp_dir.join("cache.json");
        }
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

        let crawler = Crawler::new(
            config.crawler.clone(),
            config.headless.clone(),
            config.proxy.clone(),
            cache.clone(),
        );

        let mut backends: Vec<Box<dyn SearchBackend>> = Vec::new();
        for name in &config.search.backends {
            match name.as_str() {
                "duckduckgo" => {
                    let b = DuckDuckGoBackend::new(http_client.clone())
                        .with_proxies(crawler.clients().to_vec())
                        .with_headless(crawler.headless_browser().clone());
                    backends.push(Box::new(b));
                }
                "google" => {
                    let b = GoogleBackend::new(http_client.clone())
                        .with_proxies(crawler.clients().to_vec())
                        .with_headless(crawler.headless_browser().clone());
                    backends.push(Box::new(b));
                }
                "bing" => {
                    backends.push(Box::new(BingBackend::new(http_client.clone())));
                }
                "brave" => {
                    backends.push(Box::new(BraveBackend::new(
                        http_client.clone(),
                        config.brave.clone(),
                    )));
                }
                "searxng" => {
                    backends.push(Box::new(SearXngBackend::new(
                        http_client.clone(),
                        config.searxng.clone(),
                    )));
                }
                _ => {}
            }
        }

        let dispatcher = SearchDispatcher::new(backends);
        let extractor = ExtractionPipeline::new(config.extractor.clone());
        let index = SearchIndex::open(&config.index).unwrap();

        let graph_path = std::path::Path::new(&config.index.path).join("graph.json");
        let graph = Arc::new(tokio::sync::Mutex::new(
            KnowledgeGraph::load_from_file(&graph_path).unwrap_or_else(|_| KnowledgeGraph::new()),
        ));

        SearchXyzServer::new(dispatcher, crawler, extractor, index, cache, graph, config)
    })
}

pub fn map_mcp_err(err: rmcp::ErrorData) -> anyhow::Error {
    anyhow!("MCP Error {:?}: {}", err.code, err.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[test]
    fn test_openz_embedded_paths_use_openz_dir_for_defaults() {
        std::env::remove_var("SEARCHXYZ_INDEX_PATH");
        std::env::remove_var("SEARCHXYZ_CACHE_PATH");
        let mut config = Config::default();
        apply_openz_embedded_paths(&mut config);
        assert!(config.index.path.ends_with(".openz/searchxyz/index"));
        assert!(config.cache.path.ends_with(".openz/searchxyz/cache.json"));
    }

    #[test]
    fn test_searchxyz_tools_metadata() {
        assert_eq!(SearchXyzSearchWebTool.name(), "searchxyz_search_web");
        assert_eq!(SearchXyzReadUrlTool.name(), "searchxyz_read_url");
        assert_eq!(
            SearchXyzSearchAndReadTool.name(),
            "searchxyz_search_and_read"
        );
        assert_eq!(SearchXyzRecallTool.name(), "searchxyz_recall");
        assert_eq!(SearchXyzListSourcesTool.name(), "searchxyz_list_sources");
        assert_eq!(SearchXyzDeepResearchTool.name(), "searchxyz_deep_research");
        assert_eq!(SearchXyzIndexContentTool.name(), "searchxyz_index_content");
        assert_eq!(SearchXyzSiteMapTool.name(), "searchxyz_site_map");
        assert_eq!(
            SearchXyzIndexRelationshipTool.name(),
            "searchxyz_index_relationship"
        );
        assert_eq!(SearchXyzQueryGraphTool.name(), "searchxyz_query_graph");
        assert_eq!(
            SearchXyzReadGithubRepoTool.name(),
            "searchxyz_read_github_repo"
        );
        assert_eq!(
            SearchXyzExportResearchTool.name(),
            "searchxyz_export_research"
        );
        assert_eq!(
            SearchXyzImportResearchTool.name(),
            "searchxyz_import_research"
        );
        assert_eq!(SearchXyzDeleteSourceTool.name(), "searchxyz_delete_source");
        assert_eq!(SearchXyzClearIndexTool.name(), "searchxyz_clear_index");
    }
}
