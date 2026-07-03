use std::sync::{Arc, OnceLock};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use crate::tools::Tool;

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
    tools::{
        SearchXyzServer,
        SearchWebRequest, ReadUrlRequest, SearchAndReadRequest, RecallRequest,
        ListSourcesRequest, DeepResearchRequest, IndexContentRequest, SiteMapRequest,
        IndexRelationshipRequest, QueryGraphRequest, ReadGithubRepoRequest,
        ExportResearchRequest, ImportResearchRequest, DeleteSourceRequest, ClearIndexRequest,
    },
};

use rmcp::handler::server::wrapper::Parameters;

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

fn map_mcp_err(err: rmcp::ErrorData) -> anyhow::Error {
    anyhow!("MCP Error {:?}: {}", err.code, err.message)
}

// ── 1. Search Web ─────────────────────────────────────────────

pub struct SearchXyzSearchWebTool;

#[async_trait::async_trait]
impl Tool for SearchXyzSearchWebTool {
    fn name(&self) -> &str {
        "searchxyz_search_web"
    }

    fn description(&self) -> &str {
        "Search the web using searchxyz. Returns titles, URLs, and snippets. Useful for keyless searches."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query string."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Max results to return (default: 10)."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: SearchWebRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().search_web(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 2. Read URL ───────────────────────────────────────────────

pub struct SearchXyzReadUrlTool;

#[async_trait::async_trait]
impl Tool for SearchXyzReadUrlTool {
    fn name(&self) -> &str {
        "searchxyz_read_url"
    }

    fn description(&self) -> &str {
        "Fetch a URL and extract its content as clean Markdown. Also handles PDFs, YouTube transcripts, and Git repos."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The full URL to fetch."
                },
                "depth": {
                    "type": "integer",
                    "description": "Crawl depth for recursive crawling (default: 1)."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Enable headless JS rendering (default: false)."
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: ReadUrlRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().read_url(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 3. Search and Read ─────────────────────────────────────────

pub struct SearchXyzSearchAndReadTool;

#[async_trait::async_trait]
impl Tool for SearchXyzSearchAndReadTool {
    fn name(&self) -> &str {
        "searchxyz_search_and_read"
    }

    fn description(&self) -> &str {
        "Search the web AND crawl the top results in a single call. Returns full Markdown for each result page."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query string."
                },
                "max_pages": {
                    "type": "integer",
                    "description": "How many top results to crawl (default: 3)."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Enable JS rendering (default: false)."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: SearchAndReadRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().search_and_read(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 4. Recall ─────────────────────────────────────────────────

pub struct SearchXyzRecallTool;

#[async_trait::async_trait]
impl Tool for SearchXyzRecallTool {
    fn name(&self) -> &str {
        "searchxyz_recall"
    }

    fn description(&self) -> &str {
        "Search previously crawled documents in your local knowledge base using keyword or semantic vector search."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query term."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Max results to return (default: 5)."
                },
                "semantic": {
                    "type": "boolean",
                    "description": "Perform semantic vector search instead of BM25 (default: true)."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: RecallRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().recall(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 5. List Sources ───────────────────────────────────────────

pub struct SearchXyzListSourcesTool;

#[async_trait::async_trait]
impl Tool for SearchXyzListSourcesTool {
    fn name(&self) -> &str {
        "searchxyz_list_sources"
    }

    fn description(&self) -> &str {
        "List all indexed sources and cached pages in the local knowledge base."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "Filter by source name (e.g. 'read_url', 'manual')."
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return (default: 50)."
                },
                "offset": {
                    "type": "integer",
                    "description": "Offset for pagination (default: 0)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: ListSourcesRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().list_sources(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 6. Deep Research ──────────────────────────────────────────

pub struct SearchXyzDeepResearchTool;

#[async_trait::async_trait]
impl Tool for SearchXyzDeepResearchTool {
    fn name(&self) -> &str {
        "searchxyz_deep_research"
    }

    fn description(&self) -> &str {
        "Perform iterative multi-query web crawls and compile a deep research markdown report."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The research query or topic."
                },
                "breadth": {
                    "type": "integer",
                    "description": "Number of expanded sub-queries to execute (default: 3)."
                },
                "max_pages_per_query": {
                    "type": "integer",
                    "description": "How many top pages to crawl per sub-query (default: 2)."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Enable headless JS rendering (default: false)."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: DeepResearchRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().deep_research(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 7. Index Content ──────────────────────────────────────────

pub struct SearchXyzIndexContentTool;

#[async_trait::async_trait]
impl Tool for SearchXyzIndexContentTool {
    fn name(&self) -> &str {
        "searchxyz_index_content"
    }

    fn description(&self) -> &str {
        "Manually index custom text documents into the searchxyz local database."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Source URL or identifier."
                },
                "title": {
                    "type": "string",
                    "description": "Title of the content."
                },
                "content": {
                    "type": "string",
                    "description": "The text content to index."
                }
            },
            "required": ["url", "title", "content"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: IndexContentRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().index_content(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 8. Sitemap ────────────────────────────────────────────────

pub struct SearchXyzSiteMapTool;

#[async_trait::async_trait]
impl Tool for SearchXyzSiteMapTool {
    fn name(&self) -> &str {
        "searchxyz_site_map"
    }

    fn description(&self) -> &str {
        "Discover sitemap URLs or map domain structure via fast recursive link crawling."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Domain root to map."
                },
                "use_sitemap": {
                    "type": "boolean",
                    "description": "Try parsing sitemap.xml (default: true)."
                },
                "crawl_links": {
                    "type": "boolean",
                    "description": "Fallback to internal link spiders (default: true)."
                },
                "max_links": {
                    "type": "integer",
                    "description": "Max discovered links to return (default: 100)."
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: SiteMapRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().site_map(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 9. Index Relationship ─────────────────────────────────────

pub struct SearchXyzIndexRelationshipTool;

#[async_trait::async_trait]
impl Tool for SearchXyzIndexRelationshipTool {
    fn name(&self) -> &str {
        "searchxyz_index_relationship"
    }

    fn description(&self) -> &str {
        "Manually index entity-relationship connections into the Knowledge Graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "Source entity name."
                },
                "source_type": {
                    "type": "string",
                    "description": "Source entity type/label."
                },
                "target": {
                    "type": "string",
                    "description": "Target entity name."
                },
                "target_type": {
                    "type": "string",
                    "description": "Target entity type/label."
                },
                "relationship": {
                    "type": "string",
                    "description": "Relationship verb (e.g. 'depends_on')."
                }
            },
            "required": ["source", "source_type", "target", "target_type", "relationship"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: IndexRelationshipRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().index_relationship(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 10. Query Graph ───────────────────────────────────────────

pub struct SearchXyzQueryGraphTool;

#[async_trait::async_trait]
impl Tool for SearchXyzQueryGraphTool {
    fn name(&self) -> &str {
        "searchxyz_query_graph"
    }

    fn description(&self) -> &str {
        "Query connections and traverse relationships inside the local Knowledge Graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entity": {
                    "type": "string",
                    "description": "Entity name to traverse."
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Max traversal depth (default: 2)."
                }
            },
            "required": ["entity"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: QueryGraphRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().query_graph(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 11. Read GitHub Repo ───────────────────────────────────────

pub struct SearchXyzReadGithubRepoTool;

#[async_trait::async_trait]
impl Tool for SearchXyzReadGithubRepoTool {
    fn name(&self) -> &str {
        "searchxyz_read_github_repo"
    }

    fn description(&self) -> &str {
        "Clone, recursively index, and map a GitHub repository codebase."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "repo_url": {
                    "type": "string",
                    "description": "GitHub repository URL."
                },
                "branch": {
                    "type": "string",
                    "description": "Branch name (defaults to default branch)."
                },
                "include_extensions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "File extensions to include."
                },
                "exclude_paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Folder/file paths to ignore."
                }
            },
            "required": ["repo_url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: ReadGithubRepoRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().read_github_repo(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 12. Export Research ────────────────────────────────────────

pub struct SearchXyzExportResearchTool;

#[async_trait::async_trait]
impl Tool for SearchXyzExportResearchTool {
    fn name(&self) -> &str {
        "searchxyz_export_research"
    }

    fn description(&self) -> &str {
        "Export research documents and graph metrics into a portable JSON bundle."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Query to filter exported documents."
                },
                "limit": {
                    "type": "integer",
                    "description": "Max documents to export (default: 50)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: ExportResearchRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().export_research(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 13. Import Research ────────────────────────────────────────

pub struct SearchXyzImportResearchTool;

#[async_trait::async_trait]
impl Tool for SearchXyzImportResearchTool {
    fn name(&self) -> &str {
        "searchxyz_import_research"
    }

    fn description(&self) -> &str {
        "Import a research bundle JSON payload into the local index and graph."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "payload": {
                    "type": "string",
                    "description": "Serialized JSON research bundle payload."
                }
            },
            "required": ["payload"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: ImportResearchRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().import_research(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 14. Delete Source ─────────────────────────────────────────

pub struct SearchXyzDeleteSourceTool;

#[async_trait::async_trait]
impl Tool for SearchXyzDeleteSourceTool {
    fn name(&self) -> &str {
        "searchxyz_delete_source"
    }

    fn description(&self) -> &str {
        "Delete a document and its relationships from the database by URL/prefix."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to delete."
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: DeleteSourceRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server().delete_source(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

// ── 15. Clear Index ───────────────────────────────────────────

pub struct SearchXyzClearIndexTool;

#[async_trait::async_trait]
impl Tool for SearchXyzClearIndexTool {
    fn name(&self) -> &str {
        "searchxyz_clear_index"
    }

    fn description(&self) -> &str {
        "Clear all documents and Knowledge Graph data from the local database."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let req = ClearIndexRequest {};
        let res = get_server().clear_index(Parameters(req)).await.map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_searchxyz_tools_metadata() {
        assert_eq!(SearchXyzSearchWebTool.name(), "searchxyz_search_web");
        assert_eq!(SearchXyzReadUrlTool.name(), "searchxyz_read_url");
        assert_eq!(SearchXyzSearchAndReadTool.name(), "searchxyz_search_and_read");
        assert_eq!(SearchXyzRecallTool.name(), "searchxyz_recall");
        assert_eq!(SearchXyzListSourcesTool.name(), "searchxyz_list_sources");
        assert_eq!(SearchXyzDeepResearchTool.name(), "searchxyz_deep_research");
        assert_eq!(SearchXyzIndexContentTool.name(), "searchxyz_index_content");
        assert_eq!(SearchXyzSiteMapTool.name(), "searchxyz_site_map");
        assert_eq!(SearchXyzIndexRelationshipTool.name(), "searchxyz_index_relationship");
        assert_eq!(SearchXyzQueryGraphTool.name(), "searchxyz_query_graph");
        assert_eq!(SearchXyzReadGithubRepoTool.name(), "searchxyz_read_github_repo");
        assert_eq!(SearchXyzExportResearchTool.name(), "searchxyz_export_research");
        assert_eq!(SearchXyzImportResearchTool.name(), "searchxyz_import_research");
        assert_eq!(SearchXyzDeleteSourceTool.name(), "searchxyz_delete_source");
        assert_eq!(SearchXyzClearIndexTool.name(), "searchxyz_clear_index");
    }
}

