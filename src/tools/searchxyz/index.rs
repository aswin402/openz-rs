use super::get_server;
use crate::tools::Tool;
use anyhow::Result;
use crate::tools::searchxyz::server::{
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
        let mut normalized = super::normalize_query_arg(arguments);
        super::map_field_alias(&mut normalized, "max_results", &["limit", "count", "maxResults"]);
        super::map_field_alias(&mut normalized, "semantic", &["vector", "embedding"]);
        super::coerce_numeric_fields(&mut normalized, &["max_results"]);
        super::coerce_bool_fields(&mut normalized, &["semantic"]);
        let req: RecallRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .recall(req).await?;
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
        let mut normalized = super::wrap_string_arg(arguments, "source");
        super::map_field_alias(&mut normalized, "limit", &["max_results", "count"]);
        super::map_field_alias(&mut normalized, "offset", &["skip"]);
        super::map_field_alias(&mut normalized, "source", &["source_name", "type", "filter"]);
        super::coerce_numeric_fields(&mut normalized, &["limit", "offset"]);
        let req: ListSourcesRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .list_sources(req).await?;
        Ok(json!(res))
    }
}

pub(crate) fn normalize_index_content_url(normalized: &mut Value) {
    let maybe_norm = normalized
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|t| t.contains("://") || t.contains('.') || t.starts_with("//"))
        .map(crate::tools::web::normalize_web_url);
    match maybe_norm {
        Some(u) => normalized["url"] = json!(u),
        None => {}
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
        let mut normalized = arguments.clone();
        super::map_field_alias(&mut normalized, "url", &["uri", "link", "id", "source"]);
        normalize_index_content_url(&mut normalized);
        super::map_field_alias(&mut normalized, "title", &["name", "heading", "subject"]);
        super::map_field_alias(&mut normalized, "content", &["text", "body", "data"]);
        let req: IndexContentRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .index_content(req).await?;
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
        let mut normalized = super::wrap_string_arg(arguments, "query");
        super::map_field_alias(&mut normalized, "query", &["q", "search", "filter"]);
        super::map_field_alias(&mut normalized, "limit", &["max_results", "count"]);
        super::map_field_alias(&mut normalized, "max_chars", &["maxChars", "limit_chars", "maxLength"]);
        super::map_field_alias(&mut normalized, "file_path", &["filePath", "path", "output_path", "outputPath", "destination", "file", "target"]);
        super::coerce_numeric_fields(&mut normalized, &["limit", "max_chars"]);

        let file_path_opt = normalized.get("file_path").and_then(|v| v.as_str()).map(str::to_string);
        let req: ExportResearchRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .export_research(req).await?;

        match file_path_opt {
            Some(dest) => {
                let dest_path = std::path::Path::new(&dest);
                match dest_path.parent() {
                    Some(parent) => {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    None => {}
                }
                std::fs::write(dest_path, &res)?;
                Ok(json!({
                    "status": "success",
                    "file_path": dest,
                    "bytes_written": res.len(),
                    "message": format!("Successfully exported research bundle to {}", dest)
                }))
            }
            None => Ok(json!(res)),
        }
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
                    "description": "Serialized JSON research bundle payload or path to a JSON file."
                },
                "file_path": {
                    "type": "string",
                    "description": "Optional file path to import the research bundle from."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let mut normalized = super::wrap_string_arg(arguments, "payload");
        super::map_field_alias(&mut normalized, "payload", &["bundle", "data", "content"]);
        super::map_field_alias(&mut normalized, "file_path", &["filePath", "path", "file", "source", "bundle_path", "bundlePath"]);

        let file_content = normalized
            .get("file_path")
            .and_then(|v| v.as_str())
            .and_then(|fp| std::fs::read_to_string(fp).ok())
            .or_else(|| {
                let trimmed = normalized.get("payload")?.as_str()?.trim();
                let is_file = (trimmed.starts_with('/') || trimmed.ends_with(".json"))
                    && std::path::Path::new(trimmed).exists();
                is_file
                    .then(|| std::fs::read_to_string(trimmed).ok())
                    .flatten()
            });

        match file_content {
            Some(content) => normalized["payload"] = Value::String(content),
            None => {}
        }

        let payload_structured = normalized
            .get("payload")
            .filter(|v| v.is_object() || v.is_array())
            .map(ToString::to_string);
        let has_inline_bundle = normalized.get("payload").is_none()
            && (normalized.get("documents").is_some() || normalized.get("graph").is_some());

        match (payload_structured, has_inline_bundle) {
            (Some(s), _) => normalized["payload"] = Value::String(s),
            (None, true) => {
                normalized = json!({
                    "payload": arguments.to_string()
                });
            }
            (None, false) => {}
        }
        let req: ImportResearchRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .import_research(req).await?;
        Ok(json!(res))
    }
}

pub(crate) fn wrap_delete_source_arg(arguments: &Value) -> Value {
    match arguments.as_str() {
        Some(s) => json!({
            "url": crate::tools::web::normalize_web_url(s),
            "confirm": true
        }),
        None => arguments.clone(),
    }
}

pub(crate) fn wrap_clear_index_arg(arguments: &Value) -> Value {
    match arguments.is_string() {
        true => json!({ "confirm": true }),
        false => arguments.clone(),
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
        let mut normalized = wrap_delete_source_arg(arguments);
        super::map_field_alias(&mut normalized, "url", &["uri", "link", "source", "target"]);
        super::map_field_alias(&mut normalized, "confirm", &["force", "yes", "confirmed"]);
        super::normalize_url_field(&mut normalized, "url");
        super::coerce_bool_fields(&mut normalized, &["confirm"]);
        let req: DeleteSourceRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .delete_source(req).await?;
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
        let mut normalized = wrap_clear_index_arg(arguments);
        super::map_field_alias(&mut normalized, "confirm", &["force", "yes", "confirmed"]);
        super::coerce_bool_fields(&mut normalized, &["confirm"]);
        let req: ClearIndexRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .clear_index(req).await?;
        Ok(json!(res))
    }
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;

