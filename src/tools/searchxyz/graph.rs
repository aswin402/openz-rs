use super::{get_server, map_mcp_err};
use crate::tools::Tool;
use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use searchxyz::tools::{IndexRelationshipRequest, QueryGraphRequest, ReadGithubRepoRequest};
use serde_json::{json, Value};

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
        let res = get_server()
            .index_relationship(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
        let res = get_server()
            .query_graph(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
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
                },
                "max_files": {
                    "type": "integer",
                    "description": "Maximum files to ingest from the repository (default: 2000, capped at 10000)."
                },
                "max_total_bytes": {
                    "type": "integer",
                    "description": "Maximum total bytes to ingest from selected files (default: 20MB, capped at 200MB)."
                },
                "git_timeout_secs": {
                    "type": "integer",
                    "description": "Timeout for each git command in seconds (default: 60, capped at 600)."
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Optional output character budget. Truncates large repository summaries with metadata when exceeded."
                }
            },
            "required": ["repo_url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let req: ReadGithubRepoRequest = serde_json::from_value(arguments.clone())?;
        let res = get_server()
            .read_github_repo(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}
