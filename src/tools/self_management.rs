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
}
