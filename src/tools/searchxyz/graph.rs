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
        let mut normalized = arguments.clone();
        super::map_field_alias(&mut normalized, "source", &["from", "src", "source_entity", "sourceEntity"]);
        super::map_field_alias(&mut normalized, "source_type", &["sourceType", "from_type", "src_type"]);
        super::map_field_alias(&mut normalized, "target", &["to", "dst", "target_entity", "targetEntity"]);
        super::map_field_alias(&mut normalized, "target_type", &["targetType", "to_type", "dst_type"]);
        super::map_field_alias(&mut normalized, "relationship", &["rel", "relation", "type", "predicate", "edge"]);
        let req: IndexRelationshipRequest = serde_json::from_value(normalized)?;
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
        let mut normalized = if let Some(s) = arguments.as_str() {
            json!({ "entity": s.trim() })
        } else {
            arguments.clone()
        };
        super::map_field_alias(&mut normalized, "entity", &["name", "node", "query", "target"]);
        super::map_field_alias(&mut normalized, "max_depth", &["depth", "maxDepth"]);
        super::coerce_numeric_fields(&mut normalized, &["max_depth"]);
        let req: QueryGraphRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .query_graph(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(json!(res))
    }
}

fn parse_github_file_limit_error(message: &str) -> Option<(u64, u64)> {
    if !message.contains("GitHub ingest file count limit exceeded") {
        return None;
    }
    let files = message
        .split("files=")
        .nth(1)?
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse::<u64>()
        .ok()?;
    let max_files = message
        .split("max_files=")
        .nth(1)?
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse::<u64>()
        .ok()?;
    Some((files, max_files))
}

fn github_file_limit_error_value(message: &str) -> Option<Value> {
    let (files, max_files) = parse_github_file_limit_error(message)?;
    Some(json!({
        "status": "limit_exceeded",
        "error_kind": "repo_file_limit_exceeded",
        "retryable": true,
        "files": files,
        "max_files": max_files,
        "recommended_max_files": files,
        "next_step": format!("Retry searchxyz_read_github_repo with max_files >= {files}, narrower include_extensions, or exclude_paths for generated/vendor folders."),
        "raw_error": message,
    }))
}

fn github_file_limit_should_auto_retry(arguments: &Value, files: u64, max_files: u64) -> bool {
    let auto_expand = arguments
        .get("auto_expand_max_files")
        .or_else(|| arguments.get("autoExpandMaxFiles"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    auto_expand && files > max_files && files <= 2_000
}

fn map_github_repo_err(err: rmcp::ErrorData) -> anyhow::Error {
    if let Some(payload) = github_file_limit_error_value(&err.message) {
        return anyhow::anyhow!(payload.to_string());
    }
    map_mcp_err(err)
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
                "auto_expand_max_files": {
                    "type": "boolean",
                    "description": "Automatically retry with the discovered file count when max_files is too low for a small repository (default: true, capped at 2000 files)."
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
        let mut normalized = if let Some(s) = arguments.as_str() {
            json!({ "repo_url": crate::tools::web::normalize_web_url(s) })
        } else {
            arguments.clone()
        };
        super::map_field_alias(&mut normalized, "repo_url", &["repoUrl", "url", "repo", "repository", "target", "link"]);
        if let Some(u) = normalized.get("repo_url").and_then(|v| v.as_str()) {
            normalized["repo_url"] = json!(crate::tools::web::normalize_web_url(u));
        }
        super::map_field_alias(&mut normalized, "branch", &["ref", "tag"]);
        super::map_field_alias(&mut normalized, "include_extensions", &["includeExtensions", "extensions"]);
        super::map_field_alias(&mut normalized, "exclude_paths", &["excludePaths", "ignore_paths", "ignore"]);
        super::map_field_alias(&mut normalized, "max_files", &["maxFiles", "files_limit", "limit"]);
        super::map_field_alias(&mut normalized, "max_total_bytes", &["maxTotalBytes", "max_bytes", "bytes_limit"]);
        super::map_field_alias(&mut normalized, "auto_expand_max_files", &["autoExpandMaxFiles", "auto_expand"]);
        super::map_field_alias(&mut normalized, "git_timeout_secs", &["gitTimeoutSecs", "timeout_secs", "timeout"]);
        super::map_field_alias(&mut normalized, "max_chars", &["maxChars", "limit_chars", "maxLength"]);
        super::coerce_numeric_fields(
            &mut normalized,
            &["max_files", "max_total_bytes", "git_timeout_secs", "max_chars"],
        );
        super::coerce_bool_fields(&mut normalized, &["auto_expand_max_files"]);
        let req: ReadGithubRepoRequest = serde_json::from_value(normalized.clone())?;
        let res = match get_server().read_github_repo(Parameters(req)).await {
            Ok(res) => res,
            Err(err) => {
                if let Some((files, max_files)) = parse_github_file_limit_error(&err.message) {
                    if github_file_limit_should_auto_retry(&normalized, files, max_files) {
                        let mut retry_args = normalized.clone();
                        retry_args["max_files"] = json!(files);
                        tracing::info!(
                            files,
                            max_files,
                            "GitHub ingest max_files was too low; retrying automatically"
                        );
                        let retry_req: ReadGithubRepoRequest = serde_json::from_value(retry_args)?;
                        let retry_res = get_server()
                            .read_github_repo(Parameters(retry_req))
                            .await
                            .map_err(map_github_repo_err)?;
                        return Ok(json!({
                            "status": "success_after_auto_max_files_retry",
                            "auto_retry": {
                                "reason": "repo_file_limit_exceeded",
                                "original_max_files": max_files,
                                "retried_max_files": files
                            },
                            "result": retry_res
                        }));
                    }
                }
                return Err(map_github_repo_err(err));
            }
        };
        Ok(json!(res))
    }
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;

