use crate::config::schema::McpServerConfig;
use crate::config::{load_config, save_config};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct ManageMcpTool;

#[async_trait::async_trait]
impl Tool for ManageMcpTool {
    fn name(&self) -> &str {
        "manage_mcp"
    }

    fn description(&self) -> &str {
        "Manage MCP (Model Context Protocol) server configurations (add, remove, list, enable, disable). Modifies the openz config file (~/.openz/config.json)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "remove", "list", "enable", "disable"],
                    "description": "The action to perform on the MCP configurations."
                },
                "name": {
                    "type": "string",
                    "description": "The name/identifier of the MCP server (e.g. 'sqlite', 'memory'). Required for all actions except 'list'."
                },
                "command": {
                    "type": "string",
                    "description": "The executable command to run (e.g. 'npx', 'uvx', 'python'). Required for 'add'."
                },
                "args": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Command-line arguments to pass to the MCP command (e.g. ['-y', '@modelcontextprotocol/server-postgres']). Optional for 'add'."
                },
                "enabled": {
                    "type": "boolean",
                    "description": "Whether the MCP server should be active/enabled. Defaults to true on 'add'."
                },
                "confirm": {
                    "type": "boolean",
                    "description": "Required for 'remove' action. Set to true to confirm permanent deletion of the MCP server configuration."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let mut config = load_config()?;

        match action {
            "list" => {
                let list: Value = json!(config.mcp_servers);
                Ok(json!({
                    "status": "success",
                    "mcp_servers": list
                }))
            }
            "add" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for action 'add'"))?
                    .trim()
                    .to_string();
                let command = arguments
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'command' parameter for action 'add'"))?
                    .trim()
                    .to_string();

                let args = arguments
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let enabled = arguments
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let server_config = McpServerConfig {
                    command,
                    args,
                    enabled,
                };

                config.mcp_servers.insert(name.clone(), server_config);
                save_config(&config)?;

                Ok(json!({
                    "status": "success",
                    "message": format!("Successfully added/configured MCP server '{}'", name)
                }))
            }
            "remove" => {
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for action 'remove'"))?
                    .trim()
                    .to_string();

                match config.mcp_servers.contains_key(&name) {
                    true => {}
                    false => {
                        return Err(anyhow!("MCP server '{}' not found in configuration", name));
                    }
                }

                let confirm = arguments
                    .get("confirm")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                match confirm {
                    false => Ok(json!({
                        "status": "requires_confirmation",
                        "message": format!("MCP server '{}' exists. Pass 'confirm: true' to permanently remove it.", name)
                    })),
                    true => {
                        config.mcp_servers.remove(&name);
                        save_config(&config)?;
                        Ok(json!({
                            "status": "success",
                            "message": format!("Successfully removed MCP server '{}'", name)
                        }))
                    }
                }
            }
            "enable" | "disable" => {
                let enabled = action == "enable";
                let verb = match enabled {
                    true => "enabled",
                    false => "disabled",
                };
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for action '{}'", action))?
                    .trim()
                    .to_string();

                match config.mcp_servers.get_mut(&name) {
                    Some(server) => {
                        server.enabled = enabled;
                        save_config(&config)?;
                        Ok(json!({
                            "status": "success",
                            "message": format!("Successfully {} MCP server '{}'", verb, name)
                        }))
                    }
                    None => Err(anyhow!("MCP server '{}' not found in configuration", name)),
                }
            }
            _ => Err(anyhow!("Invalid action '{}'", action)),
        }
    }
}

#[cfg(test)]
#[path = "mcp_manager_tests.rs"]
mod tests;

