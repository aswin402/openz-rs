use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use crate::config::{load_config, save_config};
use crate::config::schema::McpServerConfig;
use crate::tools::Tool;

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
        let action = arguments.get("action").and_then(|v| v.as_str())
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
                let name = arguments.get("name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for action 'add'"))?.trim().to_string();
                let command = arguments.get("command").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'command' parameter for action 'add'"))?.trim().to_string();
                
                let mut args = Vec::new();
                if let Some(arr) = arguments.get("args").and_then(|v| v.as_array()) {
                    for arg in arr {
                        if let Some(s) = arg.as_str() {
                            args.push(s.to_string());
                        }
                    }
                }

                let enabled = arguments.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

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
                let name = arguments.get("name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for action 'remove'"))?.trim().to_string();

                if !config.mcp_servers.contains_key(&name) {
                    return Err(anyhow!("MCP server '{}' not found in configuration", name));
                }

                let confirm = arguments.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);
                if !confirm {
                    return Ok(json!({
                        "status": "requires_confirmation",
                        "message": format!("MCP server '{}' exists. Pass 'confirm: true' to permanently remove it.", name)
                    }));
                }

                config.mcp_servers.remove(&name);
                save_config(&config)?;
                Ok(json!({
                    "status": "success",
                    "message": format!("Successfully removed MCP server '{}'", name)
                }))
            }
            "enable" => {
                let name = arguments.get("name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for action 'enable'"))?.trim().to_string();

                if let Some(server) = config.mcp_servers.get_mut(&name) {
                    server.enabled = true;
                    save_config(&config)?;
                    Ok(json!({
                        "status": "success",
                        "message": format!("Successfully enabled MCP server '{}'", name)
                    }))
                } else {
                    Err(anyhow!("MCP server '{}' not found in configuration", name))
                }
            }
            "disable" => {
                let name = arguments.get("name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for action 'disable'"))?.trim().to_string();

                if let Some(server) = config.mcp_servers.get_mut(&name) {
                    server.enabled = false;
                    save_config(&config)?;
                    Ok(json!({
                        "status": "success",
                        "message": format!("Successfully disabled MCP server '{}'", name)
                    }))
                } else {
                    Err(anyhow!("MCP server '{}' not found in configuration", name))
                }
            }
            _ => Err(anyhow!("Invalid action '{}'", action))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manage_mcp_actions() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("openz_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        let tool = ManageMcpTool;

        crate::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async move {
            // Test action: add
            let add_args = json!({
                "action": "add",
                "name": "test-mcp",
                "command": "node",
                "args": ["test-server.js"],
                "enabled": true
            });
            let res = tool.call(&add_args).await?;
            assert_eq!(res["status"], "success");

            // Test action: list
            let list_args = json!({
                "action": "list"
            });
            let res = tool.call(&list_args).await?;
            assert_eq!(res["status"], "success");
            assert!(res["mcp_servers"]["test-mcp"].is_object());
            assert_eq!(res["mcp_servers"]["test-mcp"]["command"], "node");
            assert_eq!(res["mcp_servers"]["test-mcp"]["enabled"], true);

            // Test action: disable
            let disable_args = json!({
                "action": "disable",
                "name": "test-mcp"
            });
            let res = tool.call(&disable_args).await?;
            assert_eq!(res["status"], "success");

            // Verify disabled state via list
            let res = tool.call(&list_args).await?;
            assert_eq!(res["mcp_servers"]["test-mcp"]["enabled"], false);

            // Test action: enable
            let enable_args = json!({
                "action": "enable",
                "name": "test-mcp"
            });
            let res = tool.call(&enable_args).await?;
            assert_eq!(res["status"], "success");

            // Verify enabled state via list
            let res = tool.call(&list_args).await?;
            assert_eq!(res["mcp_servers"]["test-mcp"]["enabled"], true);

            // Test action: remove (requires confirmation)
            let remove_args = json!({
                "action": "remove",
                "name": "test-mcp"
            });
            let res = tool.call(&remove_args).await?;
            assert_eq!(res["status"], "requires_confirmation");

            // Test action: remove without confirm param should fail
            let remove_args = json!({
                "action": "remove",
                "name": "test-mcp",
                "confirm": true
            });
            let res = tool.call(&remove_args).await?;
            assert_eq!(res["status"], "success");

            // Verify removed state via list
            let res = tool.call(&list_args).await?;
            assert!(!res["mcp_servers"].as_object().unwrap().contains_key("test-mcp"));

            // Clean up
            let _ = std::fs::remove_dir_all(&temp_dir);

            Ok(())
        }).await
    }
}
