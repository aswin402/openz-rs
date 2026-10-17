use super::{extract_first_str, extract_trimmed_str, ACTION_KEYS};
use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct ManageBackupsTool;

const BACKUP_NAME_KEYS: &[&str] = &[
    "backup_name",
    "backupName",
    "name",
    "target",
    "file",
    "filename",
];

fn extract_backup_name(
    arguments: &Value,
    direct_name: Option<&str>,
    backups_dir: &Path,
) -> Option<String> {
    let raw = direct_name
        .map(ToString::to_string)
        .or_else(|| extract_trimmed_str(arguments, BACKUP_NAME_KEYS))?;

    let candidates = match raw.ends_with(".json") {
        true => vec![raw.clone()],
        false => vec![
            raw.clone(),
            format!("{}.json", raw),
            format!("backup_{}.json", raw),
        ],
    };

    Some(
        candidates
            .into_iter()
            .find(|candidate| backups_dir.join(candidate).exists())
            .unwrap_or(raw),
    )
}

fn resolve_validated_backup_path(
    arguments: &Value,
    direct_name: Option<&str>,
    backups_dir: &Path,
    action_label: &str,
) -> Result<(String, PathBuf)> {
    let backup_name = extract_backup_name(arguments, direct_name, backups_dir)
        .ok_or_else(|| anyhow::anyhow!("Missing 'backup_name' for {} action", action_label))?;

    let has_traversal = backup_name.contains('/')
        || backup_name.contains('\\')
        || backup_name.contains("..");
    (!has_traversal)
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("Invalid backup_name specified."))?;

    let backup_path = backups_dir.join(&backup_name);
    backup_path.exists().then_some(()).ok_or_else(|| {
        anyhow::anyhow!("Backup file '{}' does not exist.", backup_name)
    })?;

    Ok((backup_name, backup_path))
}

fn read_optional_json_file(path: &Path) -> Result<Value> {
    match path.exists() {
        false => Ok(Value::Null),
        true => {
            let content = std::fs::read_to_string(path)?;
            Ok(serde_json::from_str::<Value>(&content).unwrap_or(Value::Null))
        }
    }
}

fn write_optional_json_backup_section(
    backup_data: &Value,
    key: &str,
    dest_path: &Path,
) -> Result<()> {
    match backup_data.get(key).filter(|v| !v.is_null()) {
        Some(val) => {
            std::fs::write(dest_path, serde_json::to_string_pretty(val)?)?;
            Ok(())
        }
        None => Ok(()),
    }
}

#[async_trait::async_trait]
impl Tool for ManageBackupsTool {
    fn name(&self) -> &str {
        "manage_backups"
    }

    fn description(&self) -> &str {
        "Create, list, restore, or delete backups of the agent configuration, subagent profiles, and markdown skills."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "restore", "delete"],
                    "description": "The backup curation action to perform."
                },
                "backup_name": {
                    "type": "string",
                    "description": "Required for 'restore' or 'delete'. The filename of the backup target."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (action_str, direct_backup_name) = match arguments {
            Value::String(s) => {
                let s_trim = s.trim();
                match s_trim.to_lowercase().as_str() {
                    "create" | "backup" | "save" | "new" => (Some("create"), None),
                    "list" | "ls" | "view" | "show" => (Some("list"), None),
                    _ => (Some("restore"), Some(s_trim)),
                }
            }
            Value::Object(_) => (extract_first_str(arguments, &ACTION_KEYS[..5]), None),
            _ => (None, None),
        };

        let normalized_action = action_str
            .map(|a| a.trim().to_lowercase())
            .unwrap_or_else(|| "list".to_string());
        let action = match normalized_action.as_str() {
            "" | "list" | "ls" | "view" | "show" => "list",
            "create" | "backup" | "save" | "new" => "create",
            "restore" | "load" | "apply" => "restore",
            "delete" | "remove" | "rm" => "delete",
            other => other,
        };
        let openz_dir = crate::config::loader::runtime_data_dir();
        let backups_dir = openz_dir.join("backups");

        match action {
            "create" => {
                std::fs::create_dir_all(&backups_dir)?;

                let config_val = read_optional_json_file(&openz_dir.join("config.json"))?;
                let subagents_val = read_optional_json_file(&openz_dir.join("subagents.json"))?;
                let skills_dir = openz_dir.join("skills");

                let mut skills_map = serde_json::Map::new();
                for entry in skills_dir
                    .exists()
                    .then(|| std::fs::read_dir(&skills_dir).ok())
                    .flatten()
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    let path = entry.path();
                    let is_md = path.is_file()
                        && path.extension().and_then(|s| s.to_str()) == Some("md");
                    let name_and_content = is_md
                        .then(|| {
                            let name = path.file_name()?.to_str()?.to_string();
                            let content = std::fs::read_to_string(&path).ok()?;
                            Some((name, Value::String(content)))
                        })
                        .flatten();
                    skills_map.extend(name_and_content);
                }

                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let backup_filename = format!("backup_{}.json", timestamp);
                let backup_path = backups_dir.join(&backup_filename);

                let backup_data = serde_json::json!({
                    "version": "1.0",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "config": config_val,
                    "subagents": subagents_val,
                    "skills": skills_map
                });

                std::fs::write(&backup_path, serde_json::to_string_pretty(&backup_data)?)?;

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Backup successfully created: {}", backup_filename),
                    "backup_name": backup_filename
                }))
            }
            "list" => {
                let mut backups_list: Vec<Value> = backups_dir
                    .exists()
                    .then(|| std::fs::read_dir(&backups_dir).ok())
                    .flatten()
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|entry| {
                        let path = entry.path();
                        let name = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        let is_backup = path.is_file()
                            && name.starts_with("backup_")
                            && name.ends_with(".json");
                        is_backup.then(|| {
                            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            let created_at = entry
                                .metadata()
                                .and_then(|m| m.modified())
                                .map(|t| {
                                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                                    dt.to_rfc3339()
                                })
                                .unwrap_or_default();
                            serde_json::json!({
                                "backup_name": name,
                                "size_bytes": size_bytes,
                                "created_at": created_at
                            })
                        })
                    })
                    .collect();

                backups_list.sort_by(|a, b| {
                    b["backup_name"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(a["backup_name"].as_str().unwrap_or(""))
                });

                Ok(serde_json::json!({
                    "status": "success",
                    "backups": backups_list
                }))
            }
            "restore" => {
                let (backup_name, backup_path) = resolve_validated_backup_path(
                    arguments,
                    direct_backup_name,
                    &backups_dir,
                    "restore",
                )?;

                let content = std::fs::read_to_string(&backup_path)?;
                let backup_data: Value = serde_json::from_str(&content)?;

                write_optional_json_backup_section(
                    &backup_data,
                    "config",
                    &openz_dir.join("config.json"),
                )?;
                write_optional_json_backup_section(
                    &backup_data,
                    "subagents",
                    &openz_dir.join("subagents.json"),
                )?;

                match backup_data.get("skills").and_then(|v| v.as_object()) {
                    Some(skills_val) => {
                        let skills_dir = openz_dir.join("skills");
                        std::fs::create_dir_all(&skills_dir)?;
                        for (name, content_val) in skills_val {
                            match content_val.as_str() {
                                Some(content_str) => {
                                    std::fs::write(skills_dir.join(name), content_str)?;
                                }
                                None => {}
                            }
                        }
                    }
                    None => {}
                }

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Backup '{}' successfully restored.", backup_name)
                }))
            }
            "delete" => {
                let (backup_name, backup_path) = resolve_validated_backup_path(
                    arguments,
                    direct_backup_name,
                    &backups_dir,
                    "delete",
                )?;

                std::fs::remove_file(&backup_path)?;

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Backup '{}' successfully deleted.", backup_name)
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action '{}'", action)),
        }
    }
}

#[cfg(test)]
#[path = "backups_tests.rs"]
mod tests;
