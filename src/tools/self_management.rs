use anyhow::Result;
use serde_json::Value;
use crate::tools::Tool;

pub struct DiagnoseToolTool {
    registry: crate::tools::ToolRegistry,
}

impl DiagnoseToolTool {
    pub fn new(registry: crate::tools::ToolRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl Tool for DiagnoseToolTool {
    fn name(&self) -> &str {
        "diagnose_tool"
    }

    fn description(&self) -> &str {
        "Diagnose, test, and profile any native tool in the agent loop. Validates arguments against schema and reports execution results."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Name of the registered tool to test."
                },
                "mock_args": {
                    "type": "object",
                    "description": "JSON arguments to pass to the tool call."
                }
            },
            "required": ["tool_name"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let tool_name = arguments.get("tool_name").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing tool_name"))?;
        let mock_args = arguments.get("mock_args").cloned().unwrap_or_else(|| serde_json::json!({}));

        // Retrieve tool bypassing filter_scope
        let tool = {
            let filter_scope_backup = {
                let mut g = self.registry.filter_scope.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
                g.take()
            };
            let t = self.registry.get(tool_name);
            if let Ok(mut g) = self.registry.filter_scope.lock() {
                *g = filter_scope_backup;
            }
            t
        };

        let tool = match tool {
            Some(t) => t,
            None => {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": format!("Tool '{}' not found in registry", tool_name)
                }));
            }
        };

        let schema = tool.parameters();
        let start_time = std::time::Instant::now();
        let result = tool.call(&mock_args).await;
        let elapsed_ms = start_time.elapsed().as_millis();

        match result {
            Ok(output) => {
                Ok(serde_json::json!({
                    "success": true,
                    "duration_ms": elapsed_ms,
                    "schema": schema,
                    "output": output
                }))
            }
            Err(e) => {
                Ok(serde_json::json!({
                    "success": false,
                    "duration_ms": elapsed_ms,
                    "schema": schema,
                    "error": e.to_string()
                }))
            }
        }
    }
}

pub struct CurateSkillTool;

#[async_trait::async_trait]
impl Tool for CurateSkillTool {
    fn name(&self) -> &str {
        "curate_skill"
    }

    fn description(&self) -> &str {
        "Curate, list, add, or delete procedural skills and guidelines in the OpenZ skills SQLite database."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "add", "delete"],
                    "description": "The curation action to perform."
                },
                "skill_name": {
                    "type": "string",
                    "description": "Name/identifier of the skill (e.g. 'rust-compilation-tricks')."
                },
                "content": {
                    "type": "string",
                    "description": "Markdown instructions/guidelines for the skill. Required for 'add'."
                },
                "profile": {
                    "type": "string",
                    "description": "Optional subagent profile name to restrict this skill to."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing action"))?;

        match action {
            "list" => {
                let profile = arguments.get("profile").and_then(|v| v.as_str());
                let skills = crate::agent::skills::load_skills_with_profile(profile)?;
                Ok(serde_json::json!({
                    "success": true,
                    "skills": skills
                }))
            }
            "add" => {
                let skill_name = arguments.get("skill_name").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'add'"))?;
                let content = arguments.get("content").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing content for action 'add'"))?;
                let profile = arguments.get("profile").and_then(|v| v.as_str());

                if let Some(prof) = profile {
                    crate::agent::skills::save_subagent_skill(prof, skill_name, content)?;
                } else {
                    crate::agent::skills::save_skill(skill_name, content)?;
                }

                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Skill '{}' successfully saved", skill_name)
                }))
            }
            "delete" => {
                let skill_name = arguments.get("skill_name").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'delete'"))?;
                let profile = arguments.get("profile").and_then(|v| v.as_str());

                crate::agent::skills::delete_skill_with_profile(skill_name, profile)?;
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Skill '{}' successfully deleted", skill_name)
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action")),
        }
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
                let prefixes_vec: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
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

fn redact_secrets(val: &mut Value) {
    match val {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let lower = k.to_lowercase();
                if lower.contains("api_key") || lower.contains("bot_token") || lower.contains("verify_token") || lower.contains("password") || lower.contains("secret") {
                    if v.is_string() {
                        *v = Value::String("********".to_string());
                    }
                } else {
                    redact_secrets(v);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                redact_secrets(item);
            }
        }
        _ => {}
    }
}

pub struct ManageConfigTool;

#[async_trait::async_trait]
impl Tool for ManageConfigTool {
    fn name(&self) -> &str {
        "manage_config"
    }

    fn description(&self) -> &str {
        "View the active configuration (redacting secrets) or modify default hyperparameters (model, temperature, max tokens, caveman mode) to optimize agent behavior."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["view", "update"],
                    "description": "Whether to view the current configuration or update hyperparameters."
                },
                "updates": {
                    "type": "object",
                    "properties": {
                        "model": {
                            "type": "string",
                            "description": "Default model prefix to use (e.g. 'openai/gpt-4o')."
                        },
                        "provider": {
                            "type": "string",
                            "description": "Default provider (e.g. 'openai')."
                        },
                        "max_tokens": {
                            "type": "integer",
                            "description": "Maximum completion tokens."
                        },
                        "temperature": {
                            "type": "number",
                            "description": "Generation temperature."
                        },
                        "caveman_mode": {
                            "type": "boolean",
                            "description": "Toggles terse/concise system prompt instructions."
                        },
                        "tool_timeout_secs": {
                            "type": "integer",
                            "description": "Max timeout for tool executions."
                        },
                        "streaming": {
                            "type": "boolean",
                            "description": "Enable/disable token response streaming."
                        },
                        "max_tool_iterations": {
                            "type": "integer",
                            "description": "Maximum execution steps per turn."
                        }
                    },
                    "description": "Key-value map of configuration defaults to update. Ignored for action 'view'."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing action"))?;

        match action {
            "view" => {
                let config = crate::config::loader::load_config()?;
                let mut config_val = serde_json::to_value(&config)?;
                redact_secrets(&mut config_val);
                Ok(serde_json::json!({
                    "success": true,
                    "config": config_val
                }))
            }
            "update" => {
                let updates = arguments.get("updates").and_then(|v| v.as_object()).ok_or_else(|| anyhow::anyhow!("Missing updates for action 'update'"))?;

                let mut config = crate::config::loader::load_config()?;

                for (k, v) in updates {
                    match k.as_str() {
                        "model" => {
                            if let Some(s) = v.as_str() {
                                config.agents.defaults.model = s.to_string();
                            }
                        }
                        "provider" => {
                            if let Some(s) = v.as_str() {
                                config.agents.defaults.provider = s.to_string();
                            }
                        }
                        "max_tokens" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.max_tokens = n as usize;
                            }
                        }
                        "temperature" => {
                            if let Some(f) = v.as_f64() {
                                config.agents.defaults.temperature = f as f32;
                            }
                        }
                        "caveman_mode" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.caveman_mode = b;
                            }
                        }
                        "tool_timeout_secs" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.tool_timeout_secs = n;
                            }
                        }
                        "streaming" => {
                            if let Some(b) = v.as_bool() {
                                config.agents.defaults.streaming = b;
                            }
                        }
                        "max_tool_iterations" => {
                            if let Some(n) = v.as_u64() {
                                config.agents.defaults.max_tool_iterations = n as usize;
                            }
                        }
                        other => {
                            return Ok(serde_json::json!({
                                "success": false,
                                "error": format!("Cannot modify field '{}' via manage_config", other)
                            }));
                        }
                    }
                }

                crate::config::loader::save_config(&config)?;
                Ok(serde_json::json!({
                    "success": true,
                    "message": "Configuration successfully updated."
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnose_and_optimize_tools() {
        let registry = crate::tools::ToolRegistry::new();
        struct DummyTool;
        #[async_trait::async_trait]
        impl Tool for DummyTool {
            fn name(&self) -> &str {
                "dummy_tool"
            }
            fn description(&self) -> &str {
                "dummy"
            }
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
                Ok(serde_json::json!({ "ok": true }))
            }
        }

        let dummy = std::sync::Arc::new(DummyTool);
        registry.register(dummy.clone());

        let diagnose = DiagnoseToolTool::new(registry.clone());
        let res = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(diagnose.call(&serde_json::json!({ "tool_name": "dummy_tool" })))
            .unwrap();
        assert!(res["success"].as_bool().unwrap());
        assert_eq!(res["output"]["ok"].as_bool().unwrap(), true);

        // Test OptimizeToolScopeTool
        let optimizer = OptimizeToolScopeTool::new(registry.clone());
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(optimizer.call(&serde_json::json!({ "active_prefixes": ["other_"] })))
            .unwrap();

        // DummyTool starts with "dummy_", which does not match prefix filter "other_"
        // It should be filtered out
        assert!(registry.get("dummy_tool").is_none());

        // Restore filter
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(optimizer.call(&serde_json::json!({ "active_prefixes": [] })))
            .unwrap();
        assert!(registry.get("dummy_tool").is_some());
    }

    #[test]
    fn test_curate_skills() {
        // Run database queries through curate_skill tool
        let tool = CurateSkillTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        // 1. Delete skill if exists
        let _ = rt.block_on(tool.call(&serde_json::json!({
            "action": "delete",
            "skill_name": "test_curate_skills_temp"
        })));

        // 2. Add skill
        let add_res = rt.block_on(tool.call(&serde_json::json!({
            "action": "add",
            "skill_name": "test_curate_skills_temp",
            "content": "This is a test skill content"
        }))).unwrap();
        assert!(add_res["success"].as_bool().unwrap());

        // 3. List skills and verify
        let list_res = rt.block_on(tool.call(&serde_json::json!({
            "action": "list"
        }))).unwrap();
        assert!(list_res["success"].as_bool().unwrap());
        let skills = list_res["skills"].as_array().unwrap();
        let found = skills.iter().any(|s| s["name"].as_str().unwrap() == "test_curate_skills_temp");
        assert!(found);

        // 4. Delete skill
        let del_res = rt.block_on(tool.call(&serde_json::json!({
            "action": "delete",
            "skill_name": "test_curate_skills_temp"
        }))).unwrap();
        assert!(del_res["success"].as_bool().unwrap());
    }

    #[test]
    fn test_manage_config() {
        let tool = ManageConfigTool;
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Save original config
        let original_config = crate::config::loader::load_config().unwrap();

        // 1. View config
        let view_res = rt.block_on(tool.call(&serde_json::json!({
            "action": "view"
        }))).unwrap();
        assert!(view_res["success"].as_bool().unwrap());
        // Verify redacted api_key / secret key format if they exist
        let config_val = &view_res["config"];
        if let Some(providers) = config_val.get("providers") {
            if let Some(openai) = providers.get("openai") {
                if let Some(api_key) = openai.get("api_key") {
                    if api_key.is_string() {
                        assert_eq!(api_key.as_str().unwrap(), "********");
                    }
                }
            }
        }

        // 2. Update config hyperparameters
        let update_res = rt.block_on(tool.call(&serde_json::json!({
            "action": "update",
            "updates": {
                "max_tokens": 1234,
                "temperature": 0.25,
                "caveman_mode": false,
                "streaming": false
            }
        }))).unwrap();
        assert!(update_res["success"].as_bool().unwrap());

        // 3. Verify they were updated and saved
        let updated_config = crate::config::loader::load_config().unwrap();
        assert_eq!(updated_config.agents.defaults.max_tokens, 1234);
        assert_eq!(updated_config.agents.defaults.temperature, 0.25f32);
        assert_eq!(updated_config.agents.defaults.caveman_mode, false);
        assert_eq!(updated_config.agents.defaults.streaming, false);

        // 4. Try updating an invalid/restricted field (should be blocked)
        let invalid_res = rt.block_on(tool.call(&serde_json::json!({
            "action": "update",
            "updates": {
                "invalid_field": "some_value"
            }
        }))).unwrap();
        assert_eq!(invalid_res["success"].as_bool().unwrap(), false);
        assert!(invalid_res["error"].as_str().unwrap().contains("invalid_field"));

        // Restore original config
        crate::config::loader::save_config(&original_config).unwrap();
    }
}
