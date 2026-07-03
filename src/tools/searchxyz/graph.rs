use serde_json::{json, Value};
use anyhow::Result;
use crate::tools::Tool;
use searchxyz::tools::{IndexRelationshipRequest, QueryGraphRequest, ReadGithubRepoRequest};
use rmcp::handler::server::wrapper::Parameters;
use super::{get_server, map_mcp_err};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::searchxyz::{
        SearchXyzSearchWebTool, SearchXyzReadUrlTool, SearchXyzSearchAndReadTool,
        SearchXyzRecallTool, SearchXyzListSourcesTool, SearchXyzDeepResearchTool,
        SearchXyzIndexContentTool, SearchXyzSiteMapTool, SearchXyzIndexRelationshipTool,
        SearchXyzQueryGraphTool, SearchXyzReadGithubRepoTool, SearchXyzExportResearchTool,
        SearchXyzImportResearchTool, SearchXyzDeleteSourceTool, SearchXyzClearIndexTool,
    };

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
