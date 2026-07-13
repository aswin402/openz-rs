use super::{get_server, map_mcp_err};
use crate::tools::Tool;
use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use searchxyz::tools::{
    ClearIndexRequest, DeleteSourceRequest, ExportResearchRequest, ImportResearchRequest,
    IndexContentRequest, ListSourcesRequest, RecallRequest,
};
use serde_json::{json, Value};

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
        let res = get_server()
            .recall(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
        let res = get_server()
            .list_sources(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
        let res = get_server()
            .index_content(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Optional output character budget. Truncates large JSON exports with metadata when exceeded."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: ExportResearchRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server()
            .export_research(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
        let res = get_server()
            .import_research(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
                },
                "confirm": {
                    "type": "boolean",
                    "description": "Must be true to confirm this destructive deletion."
                }
            },
            "required": ["url", "confirm"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: DeleteSourceRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server()
            .delete_source(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
            "properties": {
                "confirm": {
                    "type": "boolean",
                    "description": "Must be true to confirm wiping all SearchXyz documents and graph data."
                }
            },
            "required": ["confirm"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: ClearIndexRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server()
            .clear_index(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}
