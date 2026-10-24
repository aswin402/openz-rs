use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct RustDocsTool;

impl Default for RustDocsTool {
    fn default() -> Self {
        Self::new()
    }
}

const MAX_RUST_DOCS_MARKDOWN_BYTES: usize = 15_000;

impl RustDocsTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Tool for RustDocsTool {
    fn name(&self) -> &str {
        "rust_docs"
    }

    fn description(&self) -> &str {
        "Search for crates on crates.io or read documentation for any third-party Rust crate from docs.rs."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search", "get_docs"],
                    "description": "The action to perform: 'search' for crates, or 'get_docs' to read documentation."
                },
                "crate_name": {
                    "type": "string",
                    "description": "The name of the Rust crate (e.g. 'tokio', 'serde', 'axum') - required for 'get_docs'."
                },
                "query": {
                    "type": "string",
                    "description": "The search query (required for 'search', or optional for searching inside a crate)."
                },
                "sub_path": {
                    "type": "string",
                    "description": "Optional specific sub-path or item (e.g. 'struct.HashMap.html', 'fn.spawn.html', 'index.html')."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let crate_arg = arguments
            .get("crate_name")
            .or_else(|| arguments.get("crateName"))
            .or_else(|| arguments.get("crate"))
            .and_then(|v| v.as_str());
        let query_arg = arguments
            .get("query")
            .or_else(|| arguments.get("q"))
            .and_then(|v| v.as_str());
        let action = match arguments
            .get("action")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(explicit) => explicit,
            None => match (crate_arg.is_some(), query_arg.is_some()) {
                (true, _) => "get_docs",
                (false, true) => "search",
                (false, false) => return Err(anyhow!("Missing 'action' parameter")),
            },
        };
        let service = crate::tools::docs_mcp::global_docs_service();

        match action {
            "search" | "search_crate" | "search_rust_crate" => {
                let query = query_arg
                    .ok_or_else(|| anyhow!("Missing 'query' parameter for search action"))?;
                let results = service.search_rust_crate(query).await?;
                Ok(json!({
                    "status": "success",
                    "results": results
                }))
            }
            "get_docs" | "read_docs" | "read_rust_docs" | "docs" => {
                let crate_name = crate_arg
                    .ok_or_else(|| anyhow!("Missing 'crate_name' parameter for get_docs action"))?;
                let sub_path = arguments
                    .get("sub_path")
                    .or_else(|| arguments.get("item_path"))
                    .or_else(|| arguments.get("path"))
                    .and_then(|v| v.as_str());
                let (url, markdown) = service
                    .read_rust_docs_with_url(crate_name, sub_path)
                    .await?;
                let truncated_md = match markdown.len() > MAX_RUST_DOCS_MARKDOWN_BYTES {
                    true => {
                        let end = markdown.floor_char_boundary(MAX_RUST_DOCS_MARKDOWN_BYTES);
                        format!(
                            "{}\n\n... (content truncated for size) ...",
                            &markdown[..end]
                        )
                    }
                    false => markdown,
                };
                Ok(json!({
                    "status": "success",
                    "url": url,
                    "markdown": truncated_md
                }))
            }
            _ => Err(anyhow!("Unsupported action: {}", action)),
        }
    }
}
