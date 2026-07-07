use super::{get_server, map_mcp_err};
use crate::tools::Tool;
use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use searchxyz::tools::{
    DeepResearchRequest, ReadUrlRequest, SearchAndReadRequest, SearchWebRequest, SiteMapRequest,
};
use serde_json::{json, Value};

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
        let res = get_server()
            .search_web(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
        let res = get_server()
            .read_url(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
        let res = get_server()
            .search_and_read(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
        let res = get_server()
            .deep_research(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
        let res = get_server()
            .site_map(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}
