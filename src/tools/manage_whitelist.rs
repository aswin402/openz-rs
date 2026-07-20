use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct ManageWhitelistTool;

#[async_trait::async_trait]
impl Tool for ManageWhitelistTool {
    fn name(&self) -> &str {
        "manage_whitelist"
    }

    fn description(&self) -> &str {
        "Configure the security whitelist of paths or command prefixes that bypass interactive user approval. Users can ask the agent to add/remove directories or command prefixes to streamline execution."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add_command", "remove_command", "add_path", "remove_path", "list"],
                    "description": "The configuration action to perform."
                },
                "value": {
                    "type": "string",
                    "description": "The command prefix or path string to add or remove (not needed for 'list')."
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
        let value = arguments.get("value").and_then(|v| v.as_str());

        use crate::config::loader::{load_config, save_config};
        let mut config = load_config()?;

        match action {
            "list" => {
                Ok(json!({
                    "status": "success",
                    "whitelisted_command_prefixes": config.agents.defaults.whitelisted_command_prefixes,
                    "whitelisted_paths": config.agents.defaults.whitelisted_paths
                }))
            }
            "add_command" => {
                let val = value.ok_or_else(|| anyhow!("Missing 'value' parameter for add_command"))?.trim().to_string();
                if val.is_empty() {
                    return Err(anyhow!("Whitelisted command prefix cannot be empty"));
                }
                if !config.agents.defaults.whitelisted_command_prefixes.contains(&val) {
                    config.agents.defaults.whitelisted_command_prefixes.push(val.clone());
                    save_config(&config)?;
                }
                Ok(json!({
                    "status": "success",
                    "message": format!("Added '{}' to whitelisted command prefixes.", val),
                    "whitelisted_command_prefixes": config.agents.defaults.whitelisted_command_prefixes
                }))
            }
            "remove_command" => {
                let val = value.ok_or_else(|| anyhow!("Missing 'value' parameter for remove_command"))?.trim();
                let len_before = config.agents.defaults.whitelisted_command_prefixes.len();
                config.agents.defaults.whitelisted_command_prefixes.retain(|v| v != val);
                if config.agents.defaults.whitelisted_command_prefixes.len() < len_before {
                    save_config(&config)?;
                }
                Ok(json!({
                    "status": "success",
                    "message": format!("Removed '{}' from whitelisted command prefixes.", val),
                    "whitelisted_command_prefixes": config.agents.defaults.whitelisted_command_prefixes
                }))
            }
            "add_path" => {
                let val = value.ok_or_else(|| anyhow!("Missing 'value' parameter for add_path"))?.trim().to_string();
                if val.is_empty() {
                    return Err(anyhow!("Whitelisted path cannot be empty"));
                }
                let resolved = crate::config::resolve_path(&val);
                let val_resolved = resolved.to_string_lossy().to_string();
                if !config.agents.defaults.whitelisted_paths.contains(&val_resolved) {
                    config.agents.defaults.whitelisted_paths.push(val_resolved.clone());
                    save_config(&config)?;
                }
                Ok(json!({
                    "status": "success",
                    "message": format!("Added '{}' to whitelisted paths.", val_resolved),
                    "whitelisted_paths": config.agents.defaults.whitelisted_paths
                }))
            }
            "remove_path" => {
                let val = value.ok_or_else(|| anyhow!("Missing 'value' parameter for remove_path"))?.trim();
                let resolved = crate::config::resolve_path(val);
                let val_resolved = resolved.to_string_lossy();
                let len_before = config.agents.defaults.whitelisted_paths.len();
                config.agents.defaults.whitelisted_paths.retain(|v| v != val && v != &*val_resolved);
                if config.agents.defaults.whitelisted_paths.len() < len_before {
                    save_config(&config)?;
                }
                Ok(json!({
                    "status": "success",
                    "message": format!("Removed '{}' from whitelisted paths.", val),
                    "whitelisted_paths": config.agents.defaults.whitelisted_paths
                }))
            }
            other => Err(anyhow!("Unknown action: {}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manage_whitelist_tool() {
        let temp_dir = std::env::temp_dir().join(format!("openz_whitelist_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let config_path = temp_dir.join("config.json");
        let initial_config = crate::config::schema::Config::default();
        std::fs::write(&config_path, serde_json::to_string_pretty(&initial_config).unwrap()).unwrap();

        crate::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
            let tool = ManageWhitelistTool;

            let add_cmd_args = json!({
                "action": "add_command",
                "value": "cargo build"
            });
            let res = tool.call(&add_cmd_args).await.unwrap();
            let prefixes = res.get("whitelisted_command_prefixes").unwrap().as_array().unwrap();
            assert!(prefixes.iter().any(|v| v.as_str() == Some("cargo build")));

            let add_path_args = json!({
                "action": "add_path",
                "value": "/tmp/test_whitelist_path"
            });
            let res = tool.call(&add_path_args).await.unwrap();
            let paths = res.get("whitelisted_paths").unwrap().as_array().unwrap();
            assert!(paths.iter().any(|v| v.as_str().unwrap().contains("test_whitelist_path")));

            let list_args = json!({
                "action": "list"
            });
            let res = tool.call(&list_args).await.unwrap();
            assert!(res.get("whitelisted_command_prefixes").is_some());
            assert!(res.get("whitelisted_paths").is_some());

            let remove_cmd_args = json!({
                "action": "remove_command",
                "value": "cargo build"
            });
            let res = tool.call(&remove_cmd_args).await.unwrap();
            let prefixes = res.get("whitelisted_command_prefixes").unwrap().as_array().unwrap();
            assert!(!prefixes.iter().any(|v| v.as_str() == Some("cargo build")));

            let remove_path_args = json!({
                "action": "remove_path",
                "value": "/tmp/test_whitelist_path"
            });
            let res = tool.call(&remove_path_args).await.unwrap();
            let paths = res.get("whitelisted_paths").unwrap().as_array().unwrap();
            assert!(!paths.iter().any(|v| v.as_str().unwrap().contains("test_whitelist_path")));
        }).await;

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
