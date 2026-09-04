use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub struct RequestToolScopeTool {
    registry: crate::tools::ToolRegistry,
}

impl RequestToolScopeTool {
    pub fn new(registry: crate::tools::ToolRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl Tool for RequestToolScopeTool {
    fn name(&self) -> &str {
        "request_tool_scope"
    }

    fn description(&self) -> &str {
        "Request additional tool domains or exact tools when the current scoped tool set is insufficient."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "description": "Why the current tool scope is insufficient."
                },
                "needed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tool domains needed for the next turn."
                },
                "needed_tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional exact tool names needed for the next turn."
                }
            },
            "required": ["reason"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let reason = arguments
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("no reason provided")
            .to_string();
        let needed_domains = arguments
            .get("needed_domains")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let needed_tools = arguments
            .get("needed_tools")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect::<Vec<_>>();
        let needed_domain_names = needed_domains
            .iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect::<Vec<_>>();
        self.registry
            .request_tool_scope(needed_tools.clone(), needed_domain_names);

        Ok(serde_json::json!({
            "status": "scope_request_recorded",
            "reason": reason,
            "needed_domains": needed_domains,
            "needed_tools": needed_tools,
            "message": "Requested tools and domains are available for the rest of this turn; they are cleared when the next user turn begins."
        }))
    }
}


pub struct OptimizeToolScopeTool {
    registry: crate::tools::ToolRegistry,
}

impl OptimizeToolScopeTool {
    pub fn new(registry: crate::tools::ToolRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl Tool for OptimizeToolScopeTool {
    fn name(&self) -> &str {
        "optimize_tool_scope"
    }

    fn description(&self) -> &str {
        "Restrict or reset the set of active tool prefixes exposed to the agent loop to reduce prompt size and prevent hallucinations."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "active_prefixes": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "List of prefix strings to restrict the scope to (e.g. ['fs_', 'opendoc_']). Pass null or empty list to reset."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let prefixes = arguments.get("active_prefixes").and_then(|v| v.as_array());

        match prefixes {
            Some(arr) if !arr.is_empty() => {
                let prefixes_vec: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                self.registry.set_filter_scope(Some(prefixes_vec.clone()));
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Tool scope restricted to prefixes: {:?}", prefixes_vec)
                }))
            }
            _ => {
                self.registry.set_filter_scope(None);
                Ok(serde_json::json!({
                    "success": true,
                    "message": "Tool scope filter reset; all tools are now active."
                }))
            }
        }
    }
}

