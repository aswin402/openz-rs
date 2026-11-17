use super::{extract_first_str, extract_trimmed_str, ACTION_KEYS};
use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub struct CurateSkillTool;

const SKILL_NAME_KEYS: &[&str] = &["skill_name", "skillName", "name", "skill", "id"];
const SKILL_CONTENT_KEYS: &[&str] = &[
    "file_content",
    "fileContent",
    "content",
    "instructions",
    "body",
    "text",
    "new_text",
    "new_string",
];
const OLD_TEXT_KEYS: &[&str] = &["old_text", "old_string", "search", "target"];
const REF_PATH_KEYS: &[&str] = &["ref_path", "file_path", "filePath", "reference", "path", "file"];
const SEARCH_QUERY_KEYS: &[&str] = &["query", "search_query", "q", "keyword"];

fn notify_skill_curation(title: &str, detail: &str) {
    let _ = crate::agent::style::spinner::get_current_session_key().map(|session_key| {
        let _ = (title == "Skill used").then(|| {
            crate::channels::send_session_notice(
                &session_key,
                &format!("◇ Skill used: {}", detail),
            );
        });
        crate::channels::websocket::publish_activity_notice(&session_key, "skill", title, detail);
    });
}

#[async_trait::async_trait]
impl Tool for CurateSkillTool {
    fn name(&self) -> &str {
        "curate_skill"
    }

    fn description(&self) -> &str {
        "Progressive AgentSkills bundle manager: list compact catalog, view full SKILL.md on demand, read/write/remove bundled references/scripts/templates/assets, add, patch, consolidate, delete, inspect history, or roll back procedural skills."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "search", "list_archive", "view", "read_ref", "write_file", "remove_file", "add", "patch", "delete", "consolidate", "history", "rollback", "restore"],
                    "description": "Skill action: 'view' loads one SKILL.md + bundle manifest; 'search' ranks native + external ~/.agents/skills packages by query; 'read_ref'/'write_file'/'remove_file' manage files under references/, scripts/, templates/, or assets/; 'patch' applies surgical edits; 'consolidate' deduplicates and quarantines broken skills; 'list_archive' and 'restore' inspect and recover skills from ~/.openz/skills/archive/."
                },
                "query": {
                    "type": "string",
                    "description": "Search query when action='search' to discover relevant native or third-party ~/.agents/skills packages."
                },
                "skill_name": {
                    "type": "string",
                    "description": "Name/identifier of the skill (e.g. 'rust_compilation_tricks')."
                },
                "content": {
                    "type": "string",
                    "description": "Markdown instructions for 'add', replacement/appended text for 'patch', or file text for 'write_file'."
                },
                "file_content": {
                    "type": "string",
                    "description": "Alias for supporting file text when action='write_file'."
                },
                "old_text": {
                    "type": "string",
                    "description": "Exact existing snippet to replace when action='patch' (omit to append to the skill)."
                },
                "ref_path": {
                    "type": "string",
                    "description": "Relative path inside the skill package (e.g. 'references/api.md', 'scripts/check.sh', 'templates/config.toml') for 'read_ref', 'write_file', or 'remove_file'."
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
        let has_target_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS).is_some();
        let has_ref_path = extract_trimmed_str(arguments, REF_PATH_KEYS).is_some();
        let action_str = match arguments {
            Value::String(s) => Some(s.as_str()),
            Value::Object(obj) => extract_first_str(arguments, &ACTION_KEYS[..5]).or_else(|| {
                let has_content = SKILL_CONTENT_KEYS.iter().any(|&k| obj.contains_key(k));
                match (has_content, has_ref_path, has_target_name) {
                    (true, true, _) => Some("write_file"),
                    (true, false, _) => Some("add"),
                    (false, true, true) => Some("read_ref"),
                    (false, false, true) => Some("view"),
                    _ => None,
                }
            }),
            _ => None,
        };

        let normalized_action = action_str
            .map(|a| a.trim().to_lowercase())
            .unwrap_or_else(|| "list".to_string());
        let action = match (normalized_action.as_str(), has_target_name) {
            ("" | "list" | "ls" | "show", _) => "list",
            ("search" | "find" | "query", _) => "search",
            ("view" | "inspect" | "get" | "read", false) => "list",
            ("view" | "inspect" | "get" | "read", true) => "view",
            ("read_ref" | "read_reference" | "ref", _) => "read_ref",
            ("write_file" | "save_file" | "put_file" | "write_ref", _) => "write_file",
            ("remove_file" | "delete_file" | "rm_file", _) => "remove_file",
            ("add" | "save" | "create" | "set" | "update", _) => "add",
            ("patch" | "edit", _) => "patch",
            ("delete" | "remove" | "rm", _) => "delete",
            ("consolidate" | "dedupe" | "maintain", _) => "consolidate",
            ("list_archive" | "archive_list" | "archived" | "archive", _) => "list_archive",
            ("unarchive" | "restore", _) => "restore",
            ("history" | "revisions" | "versions" | "snapshots", _) => "history",
            ("rollback" | "revert" | "undo", _) => "rollback",
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
            "search" => {
                let query = extract_trimmed_str(arguments, SEARCH_QUERY_KEYS)
                    .or_else(|| extract_trimmed_str(arguments, SKILL_NAME_KEYS))
                    .ok_or_else(|| anyhow::anyhow!("Missing query for action 'search'"))?;
                let matches = crate::agent::skills::load_ranked_skill_matches_with_profile(
                    &query,
                    &[],
                    profile.as_deref(),
                )?;
                let hits: Vec<Value> = matches
                    .into_iter()
                    .take(15)
                    .map(|m| {
                        serde_json::json!({
                            "name": m.skill.name,
                            "description": m.skill.description(),
                            "score": (m.score * 100.0).round() / 100.0,
                        })
                    })
                    .collect();
                Ok(serde_json::json!({
                    "success": true,
                    "query": query,
                    "count": hits.len(),
                    "matches": hits
                }))
            }
            "view" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'view'"))?;
                match crate::agent::skills::inspect_skill_bundle(&skill_name, profile.as_deref())? {
                    Some(bundle) => {
                        crate::agent::skills::record_skill_activation(
                            &bundle.skill.name,
                            profile.as_deref(),
                        );
                        notify_skill_curation("Skill used", &bundle.skill.name);
                        Ok(serde_json::json!({
                            "success": true,
                            "skill_name": bundle.skill.name,
                            "description": bundle.skill.description(),
                            "triggers": bundle.skill.triggers(),
                            "content": bundle.skill.content,
                            "references": bundle.references,
                            "scripts": bundle.scripts,
                            "templates": bundle.templates,
                            "assets": bundle.assets
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
            "write_file" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'write_file'"))?;
                let ref_path = extract_trimmed_str(arguments, REF_PATH_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing ref_path for action 'write_file'"))?;
                let file_content = extract_first_str(arguments, SKILL_CONTENT_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing file_content/content for action 'write_file'"))?;
                let saved_path = crate::agent::skills::write_skill_package_file(
                    &skill_name,
                    &ref_path,
                    file_content,
                    profile.as_deref(),
                )?;
                notify_skill_curation("Skill bundle updated", &format!("{}:{}", skill_name, ref_path));
                Ok(serde_json::json!({
                    "success": true,
                    "skill_name": skill_name,
                    "ref_path": ref_path,
                    "path": saved_path.display().to_string(),
                    "message": format!("Supporting file '{}' saved inside skill '{}'", ref_path, skill_name)
                }))
            }
            "remove_file" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'remove_file'"))?;
                let ref_path = extract_trimmed_str(arguments, REF_PATH_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing ref_path for action 'remove_file'"))?;
                crate::agent::skills::remove_skill_package_file(
                    &skill_name,
                    &ref_path,
                    profile.as_deref(),
                )?;
                notify_skill_curation("Skill bundle file removed", &format!("{}:{}", skill_name, ref_path));
                Ok(serde_json::json!({
                    "success": true,
                    "skill_name": skill_name,
                    "ref_path": ref_path,
                    "message": format!("Supporting file '{}' removed from skill '{}'", ref_path, skill_name)
                }))
            }
            "consolidate" => {
                let actions = crate::agent::skills::consolidate_learned_skills()?;
                Ok(serde_json::json!({
                    "success": true,
                    "consolidated_or_quarantined": actions,
                    "message": format!("Consolidated or quarantined {} redundant/broken skill(s)", actions)
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
            "list_archive" => {
                let archived = crate::agent::skills::list_archived_skills()?;
                Ok(serde_json::json!({
                    "success": true,
                    "count": archived.len(),
                    "archived_skills": archived
                }))
            }
            "restore" => {
                let skill_name = extract_trimmed_str(arguments, SKILL_NAME_KEYS)
                    .ok_or_else(|| anyhow::anyhow!("Missing skill_name for action 'restore'"))?;
                let revision_id = arguments
                    .get("revision_id")
                    .or_else(|| arguments.get("revisionId"))
                    .and_then(|v| v.as_i64());
                let restored = match revision_id {
                    Some(rev_id) => crate::agent::skills::rollback_skill(
                        &skill_name,
                        profile.as_deref(),
                        Some(rev_id),
                    )?,
                    None => crate::agent::skills::restore_archived_skill(&skill_name)
                        .or_else(|_| {
                            crate::agent::skills::rollback_skill(
                                &skill_name,
                                profile.as_deref(),
                                None,
                            )
                        })?,
                };
                notify_skill_curation("Skill restored", &restored.name);
                Ok(serde_json::json!({
                    "success": true,
                    "skill_name": restored.name,
                    "profile": profile,
                    "content": restored.content,
                    "message": format!("Skill '{}' restored cleanly", restored.name)
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
                )
                .or_else(|_| crate::agent::skills::restore_archived_skill(&skill_name))?;
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
