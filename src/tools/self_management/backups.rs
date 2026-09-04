use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub struct ManageBackupsTool;

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
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'action'"))?;
        let openz_dir = crate::config::loader::runtime_data_dir();
        let backups_dir = openz_dir.join("backups");

        match action {
            "create" => {
                std::fs::create_dir_all(&backups_dir)?;

                let config_file = openz_dir.join("config.json");
                let subagents_file = openz_dir.join("subagents.json");
                let skills_dir = openz_dir.join("skills");

                let config_val = if config_file.exists() {
                    let content = std::fs::read_to_string(&config_file)?;
                    serde_json::from_str::<Value>(&content).unwrap_or(Value::Null)
                } else {
                    Value::Null
                };

                let subagents_val = if subagents_file.exists() {
                    let content = std::fs::read_to_string(&subagents_file)?;
                    serde_json::from_str::<Value>(&content).unwrap_or(Value::Null)
                } else {
                    Value::Null
                };

                let mut skills_map = serde_json::Map::new();
                if skills_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file()
                                && path.extension().and_then(|s| s.to_str()) == Some("md")
                            {
                                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                        skills_map.insert(name.to_string(), Value::String(content));
                                    }
                                }
                            }
                        }
                    }
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
                let mut backups_list = Vec::new();
                if backups_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&backups_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let name = path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();
                            if path.is_file()
                                && name.starts_with("backup_")
                                && name.ends_with(".json")
                            {
                                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                                let created_at = entry
                                    .metadata()
                                    .and_then(|m| m.modified())
                                    .map(|t| {
                                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                                        dt.to_rfc3339()
                                    })
                                    .unwrap_or_default();

                                backups_list.push(serde_json::json!({
                                    "backup_name": name,
                                    "size_bytes": size_bytes,
                                    "created_at": created_at
                                }));
                            }
                        }
                    }
                }
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
                let backup_name = arguments
                    .get("backup_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'backup_name' for restore action"))?;

                if backup_name.contains('/')
                    || backup_name.contains('\\')
                    || backup_name.contains("..")
                {
                    return Err(anyhow::anyhow!("Invalid backup_name specified."));
                }

                let backup_path = backups_dir.join(backup_name);
                if !backup_path.exists() {
                    return Err(anyhow::anyhow!(
                        "Backup file '{}' does not exist.",
                        backup_name
                    ));
                }

                let content = std::fs::read_to_string(&backup_path)?;
                let backup_data: Value = serde_json::from_str(&content)?;

                if let Some(config_val) = backup_data.get("config") {
                    if !config_val.is_null() {
                        let config_file = openz_dir.join("config.json");
                        std::fs::write(&config_file, serde_json::to_string_pretty(config_val)?)?;
                    }
                }

                if let Some(subagents_val) = backup_data.get("subagents") {
                    if !subagents_val.is_null() {
                        let subagents_file = openz_dir.join("subagents.json");
                        std::fs::write(
                            &subagents_file,
                            serde_json::to_string_pretty(subagents_val)?,
                        )?;
                    }
                }

                if let Some(skills_val) = backup_data.get("skills").and_then(|v| v.as_object()) {
                    let skills_dir = openz_dir.join("skills");
                    std::fs::create_dir_all(&skills_dir)?;
                    for (name, content_val) in skills_val {
                        if let Some(content_str) = content_val.as_str() {
                            let skill_path = skills_dir.join(name);
                            std::fs::write(&skill_path, content_str)?;
                        }
                    }
                }

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Backup '{}' successfully restored.", backup_name)
                }))
            }
            "delete" => {
                let backup_name = arguments
                    .get("backup_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'backup_name' for delete action"))?;

                if backup_name.contains('/')
                    || backup_name.contains('\\')
                    || backup_name.contains("..")
                {
                    return Err(anyhow::anyhow!("Invalid backup_name specified."));
                }

                let backup_path = backups_dir.join(backup_name);
                if !backup_path.exists() {
                    return Err(anyhow::anyhow!(
                        "Backup file '{}' does not exist.",
                        backup_name
                    ));
                }

                std::fs::remove_file(&backup_path)?;

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Backup '{}' successfully deleted.", backup_name)
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action")),
        }
    }
}

