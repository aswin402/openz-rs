use super::{extract_first_str, extract_trimmed_str, ACTION_KEYS};
use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub struct CurateSkillTool;

const SKILL_NAME_KEYS: &[&str] = &["skill_name", "skillName", "name", "skill", "id"];
const SKILL_CONTENT_KEYS: &[&str] = &["content", "instructions", "body", "text", "new_text", "new_string"];
const OLD_TEXT_KEYS: &[&str] = &["old_text", "old_string", "search", "target"];
const REF_PATH_KEYS: &[&str] = &["ref_path", "reference", "path", "file"];

fn notify_skill_curation(title: &str, detail: &str) {
    let _ = crate::agent::style::spinner::get_current_session_key().map(|session_key| {
        crate::channels::websocket::publish_activity_notice(&session_key, "skill", title, detail);
    });
}

#[async_trait::async_trait]
impl Tool for CurateSkillTool {
    fn name(&self) -> &str {
        "curate_skill"
    }

    fn description(&self) -> &str {
        "Progressive skill manager: list compact catalog, view full SKILL.md on demand, read bundled references/, add, patch, delete, inspect history, or roll back procedural skills."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "view", "read_ref", "add", "patch", "delete", "history", "rollback"],
                    "description": "Skill action: 'view' loads one SKILL.md on demand; 'read_ref' loads a file under references/ or scripts/; 'patch' applies targeted edits."
                },
                "skill_name": {
                    "type": "string",
                    "description": "Name/identifier of the skill (e.g. 'rust_compilation_tricks')."
                },
                "content": {
                    "type": "string",
                    "description": "Markdown instructions for 'add', or replacement/appended text for 'patch'."
                },
                "old_text": {
                    "type": "string",
                    "description": "Exact existing snippet to replace when action='patch' (omit to append to the skill)."
                },
                "ref_path": {
                    "type": "string",
                    "description": "Relative path inside the skill package (e.g. 'references/api.md' or 'scripts/check.sh') when action='read_ref'."
                },
                "compact": {
                    "type": "boolean",
                    "description": "When action='list', return lightweight Level-1 metadata (name, description, triggers) without full Markdown bodies."
                },
                "profile": {
                    "type": "string",
                    "description": "Optional subagent profile name to restrict this skill to."
                },
                "revision_id": {
                    "type": "integer",
                    "description": "Optional snapshot revision_id to restore when action='rollback' (defaults to the most recent snapshot)."
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
        let has_target_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS).is_some();
        let action = match (normalized_action.as_str(), has_target_name) {
            ("" | "list" | "ls" | "show", _) => "list",
            ("view" | "inspect" | "get" | "read", false) => "list",
            ("view" | "inspect" | "get" | "read", true) => "view",
            ("read_ref" | "read_reference" | "ref", _) => "read_ref",
            ("add" | "save" | "create" | "set" | "update", _) => "add",
            ("patch" | "edit", _) => "patch",
            ("delete" | "remove" | "rm", _) => "delete",
            ("history" | "revisions" | "versions" | "snapshots", _) => "history",
            ("rollback" | "restore" | "revert" | "undo", _) => "rollback",
            (other, _) => other,
        };

        let profile = extract_trimmed_str(arguments, &["profile"]);

        match action {
            "list" => {
                let skills =
                    crate::agent::skills::load_skills_with_profile(profile.as_deref())?;
                let compact = arguments
                    .get("compact")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let formatted: Vec<Value> = skills
                    .into_iter()
                    .map(|s| match compact {
                        true => serde_json::json!({
                            "name": s.name,
                            "description": s.description(),
                            "triggers": s.triggers(),
                        }),
                        false => serde_json::json!({
                            "name": s.name,
                            "description": s.description(),
                            "triggers": s.triggers(),
                            "content": s.content,
                        }),
                    })
                    .collect();
                Ok(serde_json::json!({
                    "success": true,
                    "skills": formatted
                }))
            }
            "view" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'view'"))?;
                match crate::agent::skills::inspect_skill_package(&skill_name, profile.as_deref())? {
                    Some((skill, references, scripts)) => {
                        crate::agent::skills::record_skill_activation(
                            &skill.name,
                            profile.as_deref(),
                        );
                        notify_skill_curation("Skill used", &skill.name);
                        Ok(serde_json::json!({
                            "success": true,
                            "skill_name": skill.name,
                            "description": skill.description(),
                            "triggers": skill.triggers(),
                            "content": skill.content,
                            "references": references,
                            "scripts": scripts
                        }))
                    }
                    None => Err(anyhow::anyhow!("Skill '{}' not found", skill_name)),
                }
            }
            "read_ref" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'read_ref'"))?;
                let ref_path = extract_trimmed_str(arguments, REF_PATH_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing ref_path for action 'read_ref'"))?;
                let content = crate::agent::skills::read_skill_reference(
                    &skill_name,
                    &ref_path,
                    profile.as_deref(),
                )?;
                Ok(serde_json::json!({
                    "success": true,
                    "skill_name": skill_name,
                    "ref_path": ref_path,
                    "content": content
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
                notify_skill_curation("Skill curated", &skill_name);

                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Skill '{}' successfully saved", skill_name)
                }))
            }
            "patch" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'patch'"))?;
                let new_text = extract_first_str(arguments, SKILL_CONTENT_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing content/new_text for action 'patch'"))?;
                let old_text = extract_first_str(arguments, OLD_TEXT_KEYS);
                let patched = crate::agent::skills::patch_skill(
                    &skill_name,
                    old_text,
                    new_text,
                    profile.as_deref(),
                )?;
                notify_skill_curation("Skill patched", &patched.name);
                Ok(serde_json::json!({
                    "success": true,
                    "skill_name": patched.name,
                    "content": patched.content,
                    "message": format!("Skill '{}' patched cleanly", patched.name)
                }))
            }
            "delete" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'delete'"))?;

                crate::agent::skills::delete_skill_with_profile(&skill_name, profile.as_deref())?;
                notify_skill_curation("Skill deleted", &skill_name);
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Skill '{}' successfully deleted", skill_name)
                }))
            }
            "history" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'history'"))?;
                let revisions = crate::agent::skills::list_skill_revisions(
                    &skill_name,
                    profile.as_deref(),
                )?;
                Ok(serde_json::json!({
                    "success": true,
                    "skill_name": skill_name,
                    "profile": profile,
                    "revision_count": revisions.len(),
                    "revisions": revisions
                }))
            }
            "rollback" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'rollback'"))?;
                let revision_id = arguments
                    .get("revision_id")
                    .or_else(|| arguments.get("revisionId"))
                    .and_then(|v| v.as_i64());
                let restored = crate::agent::skills::rollback_skill(
                    &skill_name,
                    profile.as_deref(),
                    revision_id,
                )?;
                notify_skill_curation("Skill rolled back", &restored.name);
                Ok(serde_json::json!({
                    "success": true,
                    "skill_name": restored.name,
                    "profile": profile,
                    "content": restored.content,
                    "message": format!("Skill '{}' rolled back to previous snapshot", restored.name)
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action '{}'", action)),
        }
    }
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
