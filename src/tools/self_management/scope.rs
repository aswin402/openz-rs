use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub struct RequestToolScopeTool {
    registry: crate::tools::ToolRegistry,
}

fn parse_string_list(val: Option<&Value>) -> Vec<String> {
    match val {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) => s
            .split(',')
            .map(|part| part.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_string_list_aliases(arguments: &Value, keys: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for key in keys {
        for item in parse_string_list(arguments.get(*key)) {
            match out.iter().any(|existing| existing == &item) {
                true => {}
                false => out.push(item),
            }
        }
    }
    out
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
        let reason = ["reason", "rationale", "why", "description"]
            .into_iter()
            .find_map(|key| arguments.get(key).and_then(|v| v.as_str()))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("no reason provided")
            .to_string();
        let needed_domains = parse_string_list_aliases(
            arguments,
            &[
                "needed_domains",
                "domains",
                "domain",
                "packs",
                "pack",
                "categories",
            ],
        );
        let needed_tools = parse_string_list_aliases(
            arguments,
            &["needed_tools", "tools", "tool", "tool_names", "names"],
        );
        self.registry
            .request_tool_scope(needed_tools.clone(), needed_domains.clone());

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
        let prefixes_vec = parse_string_list_aliases(
            arguments,
            &["active_prefixes", "prefixes", "prefix", "scope"],
        );

        match prefixes_vec.is_empty() {
            false => {
                self.registry.set_filter_scope(Some(prefixes_vec.clone()));
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Tool scope restricted to prefixes: {:?}", prefixes_vec)
                }))
            }
            true => {
                self.registry.set_filter_scope(None);
                Ok(serde_json::json!({
                    "success": true,
                    "message": "Tool scope filter reset; all tools are now active."
                }))
            }
        }
    }
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
