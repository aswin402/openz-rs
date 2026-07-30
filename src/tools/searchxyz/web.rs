use super::{get_server, map_mcp_err};
use crate::tools::Tool;
use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use searchxyz::tools::{
    DeepResearchRequest, ReadUrlRequest, SearchAndReadRequest, SearchWebRequest, SiteMapRequest,
};
use serde_json::{json, Value};

// ── 0. Doctor ────────────────────────────────────────────────
pub struct SearchXyzDoctorTool;

#[async_trait::async_trait]
impl Tool for SearchXyzDoctorTool {
    fn name(&self) -> &str {
        "searchxyz_doctor"
    }

    fn description(&self) -> &str {
        "Report SearchXyz native search health, configured backends, cache/index paths, and fallback policy without performing a web search."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "include_paths": {
                    "type": "boolean",
                    "description": "Include local cache/index/graph paths in the report (default: true)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let include_paths = arguments
            .get("include_paths")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let server = get_server();
        let config = server.config.as_ref();
        let cache_len = server.cache.lock().await.len();
        let (_, source_count) = server.index.list_documents(None, 1, 0)?;
        let graph = server.graph.lock().await;
        let graph_nodes = graph.nodes.len();
        let graph_edges = graph.edges.len();
        drop(graph);

        let native_policy = std::env::var("OPENZ_WEB_SEARCH_POLICY")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "native_only".to_string());

        let searxng_status = if config.searxng.instance_url.trim().is_empty() {
            "not_configured".to_string()
        } else {
            format!("configured ({})", config.searxng.instance_url)
        };
        let brave_status = if config
            .brave
            .api_key
            .as_ref()
            .is_some_and(|k| !k.trim().is_empty())
        {
            "configured".to_string()
        } else {
            "missing_api_key".to_string()
        };
        let headless_status = if config.headless.enabled {
            match &config.headless.chrome_path {
                Some(path) => format!("enabled ({path})"),
                None => "enabled (auto chrome path)".to_string(),
            }
        } else {
            "disabled".to_string()
        };

        let mut report = String::new();
        report.push_str("# SearchXyz Doctor\n\n");
        report.push_str("## Policy\n");
        report.push_str(&format!("- OpenZ web_search policy: `{}`\n", native_policy));
        report.push_str("- Default behavior: native SearchXyz only unless `search_policy=native_then_external` or `OPENZ_WEB_SEARCH_POLICY=native_then_external` is set.\n\n");

        report.push_str("## Search Backends\n");
        report.push_str(&format!(
            "- Configured order: `{}`\n",
            config.search.backends.join(", ")
        ));
        report.push_str(&format!("- Max results: {}\n", config.search.max_results));
        report.push_str(&format!("- SearXNG: {}\n", searxng_status));
        report.push_str(&format!("- Brave: {}\n", brave_status));
        report.push_str("- DuckDuckGo/Google/Bing: keyless scraper/native backends; availability depends on upstream blocking and network health.\n\n");

        report.push_str("## Fetch & Safety\n");
        report.push_str(&format!("- Headless rendering: {}\n", headless_status));
        report.push_str(&format!(
            "- Private network fetches allowed: {}\n",
            config.crawler.allow_private_network
        ));
        report.push_str(&format!(
            "- Request timeout: {}s\n",
            config.crawler.timeout_secs
        ));
        report.push_str(&format!(
            "- Max redirects: {}\n",
            config.crawler.max_redirects
        ));
        report.push_str(&format!(
            "- Max body bytes: {}\n\n",
            config.crawler.max_body_bytes
        ));

        report.push_str("## Local State\n");
        report.push_str(&format!("- Cache entries loaded: {}\n", cache_len));
        report.push_str(&format!("- Indexed sources: {}\n", source_count));
        report.push_str(&format!("- Graph nodes: {}\n", graph_nodes));
        report.push_str(&format!("- Graph edges: {}\n", graph_edges));
        if include_paths {
            report.push_str(&format!(
                "- Index path: `{}`\n",
                config.index.path.display()
            ));
            report.push_str(&format!(
                "- Cache path: `{}`\n",
                config.cache.path.display()
            ));
            report.push_str(&format!(
                "- Graph path: `{}`\n",
                config.index.path.join("graph.json").display()
            ));
        }

        report.push_str("\n## Hints\n");
        if config.searxng.instance_url.trim().is_empty() {
            report.push_str("- Configure `SEARCHXYZ_SEARXNG_URL` for the strongest private/native discovery path.\n");
        }
        if !config.headless.enabled {
            report.push_str("- Enable SearchXyz headless mode when scraper backends are blocked by JS or anti-bot pages.\n");
        }
        report.push_str("- For one-off fallback to external engines, call `web_search` with `search_policy=native_then_external`.\n");

        Ok(json!(report))
    }
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
                },
                "include_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only keep results from these domains."
                },
                "exclude_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Drop results from these domains."
                },
                "merge_backends": {
                    "type": "boolean",
                    "description": "Query all available backends and deduplicate results instead of stopping at the first success."
                },
                "include_diagnostics": {
                    "type": "boolean",
                    "description": "Append compact backend diagnostics to the output."
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
                },
                "cache_mode": {
                    "type": "string",
                    "enum": ["auto", "prefer_cache", "revalidate", "bypass"],
                    "description": "Cache behavior. Use revalidate/check-again or bypass for live refresh tasks."
                },
                "save_mode": {
                    "type": "string",
                    "enum": ["full", "none"],
                    "description": "Whether to persist fetched content into SearchXyz memory. Use none for one-off checks."
                },
                "include_diagnostics": {
                    "type": "boolean",
                    "description": "Append compact diagnostics to the output."
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Optional output character budget. Truncates large Markdown responses with metadata when exceeded."
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
                "include_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only keep search results from these domains before crawling."
                },
                "exclude_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Drop search results from these domains before crawling."
                },
                "merge_backends": {
                    "type": "boolean",
                    "description": "Query all available backends and deduplicate results before crawling."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Enable JS rendering (default: false)."
                },
                "cache_mode": {
                    "type": "string",
                    "enum": ["auto", "prefer_cache", "revalidate", "bypass"],
                    "description": "Cache behavior. Use revalidate/check-again or bypass for live refresh tasks."
                },
                "save_mode": {
                    "type": "string",
                    "enum": ["full", "none"],
                    "description": "Whether to persist fetched content into SearchXyz memory. Use none for one-off checks."
                },
                "include_diagnostics": {
                    "type": "boolean",
                    "description": "Append compact diagnostics to the output."
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Optional output character budget. Truncates large Markdown responses with metadata when exceeded."
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
                "include_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only keep search results from these domains before crawling."
                },
                "exclude_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Drop search results from these domains before crawling."
                },
                "merge_backends": {
                    "type": "boolean",
                    "description": "Query all available backends and deduplicate results before crawling."
                },
                "render_js": {
                    "type": "boolean",
                    "description": "Enable headless JS rendering (default: false)."
                },
                "cache_mode": {
                    "type": "string",
                    "enum": ["auto", "prefer_cache", "revalidate", "bypass"],
                    "description": "Cache behavior. Use revalidate/check-again or bypass for live refresh tasks."
                },
                "save_mode": {
                    "type": "string",
                    "enum": ["full", "none"],
                    "description": "Whether to persist fetched content into SearchXyz memory. Use none for one-off checks."
                },
                "include_diagnostics": {
                    "type": "boolean",
                    "description": "Append compact diagnostics to the output."
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Optional output character budget. Truncates large research reports with metadata when exceeded."
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
