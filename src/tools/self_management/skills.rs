use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

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
        let action_str = if let Some(s) = arguments.as_str() {
            Some(s)
        } else if let Some(obj) = arguments.as_object() {
            obj.get("action")
                .or_else(|| obj.get("act"))
                .or_else(|| obj.get("command"))
                .or_else(|| obj.get("cmd"))
                .or_else(|| obj.get("op"))
                .and_then(|v| v.as_str())
        } else {
            None
        };

        let normalized_action = action_str
            .map(|a| a.trim().to_lowercase())
            .unwrap_or_else(|| "list".to_string());
        let action = match normalized_action.as_str() {
            "" | "list" | "ls" | "view" | "show" => "list",
            "add" | "save" | "create" | "set" => "add",
            "delete" | "remove" | "rm" => "delete",
            other => other,
        };

        match action {
            "list" => {
                let profile = arguments
                    .get("profile")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let skills = crate::agent::skills::load_skills_with_profile(profile)?;
                Ok(serde_json::json!({
                    "success": true,
                    "skills": skills
                }))
            }
            "add" => {
                let skill_name = arguments
                    .get("skill_name")
                    .or_else(|| arguments.get("skillName"))
                    .or_else(|| arguments.get("name"))
                    .or_else(|| arguments.get("skill"))
                    .or_else(|| arguments.get("id"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'add'"))?;
                let content = arguments
                    .get("content")
                    .or_else(|| arguments.get("instructions"))
                    .or_else(|| arguments.get("body"))
                    .or_else(|| arguments.get("text"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing content for action 'add'"))?;
                let profile = arguments
                    .get("profile")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());

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
                let skill_name = arguments
                    .get("skill_name")
                    .or_else(|| arguments.get("skillName"))
                    .or_else(|| arguments.get("name"))
                    .or_else(|| arguments.get("skill"))
                    .or_else(|| arguments.get("id"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'delete'"))?;
                let profile = arguments
                    .get("profile")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());

                crate::agent::skills::delete_skill_with_profile(skill_name, profile)?;
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Skill '{}' successfully deleted", skill_name)
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action '{}'", action)),
        }
    }
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;

