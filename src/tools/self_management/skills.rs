use super::{extract_first_str, extract_trimmed_str, ACTION_KEYS};
use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub struct CurateSkillTool;

const SKILL_NAME_KEYS: &[&str] = &["skill_name", "skillName", "name", "skill", "id"];
const SKILL_CONTENT_KEYS: &[&str] = &["content", "instructions", "body", "text"];

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
        let action_str = match arguments {
            Value::String(s) => Some(s.as_str()),
            Value::Object(obj) => extract_first_str(arguments, &ACTION_KEYS[..5]).or_else(|| {
                SKILL_CONTENT_KEYS
                    .iter()
                    .any(|&k| obj.contains_key(k))
                    .then_some("add")
            }),
            _ => None,
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

        let profile = extract_trimmed_str(arguments, &["profile"]);

        match action {
            "list" => {
                let skills =
                    crate::agent::skills::load_skills_with_profile(profile.as_deref())?;
                Ok(serde_json::json!({
                    "success": true,
                    "skills": skills
                }))
            }
            "add" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'add'"))?;
                let content = extract_first_str(arguments, SKILL_CONTENT_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing content for action 'add'"))?;

                match profile.as_deref() {
                    Some(prof) => {
                        crate::agent::skills::save_subagent_skill(prof, &skill_name, content)?
                    }
                    None => crate::agent::skills::save_skill(&skill_name, content)?,
                }

                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Skill '{}' successfully saved", skill_name)
                }))
            }
            "delete" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'delete'"))?;

                crate::agent::skills::delete_skill_with_profile(&skill_name, profile.as_deref())?;
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
