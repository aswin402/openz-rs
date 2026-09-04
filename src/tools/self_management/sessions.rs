use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub struct ManageSessionsTool;

#[async_trait::async_trait]
impl Tool for ManageSessionsTool {
    fn name(&self) -> &str {
        "manage_sessions"
    }

    fn description(&self) -> &str {
        "Manage, clean up, or archive session history files and temporary tool outputs to prevent disk space exhaustion."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "prune", "archive", "delete", "export"],
                    "description": "The curation action to perform."
                },
                "session_key": {
                    "type": "string",
                    "description": "Required for archive, delete, or export. Use the exact session_key returned by list."
                },
                "older_than_days": {
                    "type": "integer",
                    "description": "Optional for 'prune'. Delete tool output files older than this number of days. Default is 7."
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
        let sessions_dir = openz_dir.join("sessions");

        match action {
            "list" => {
                let mut sessions_list = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().and_then(|s| s.to_str()) == Some("json")
                        {
                            let session_key = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();
                            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            let last_updated = entry
                                .metadata()
                                .and_then(|m| m.modified())
                                .map(|t| {
                                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                                    dt.to_rfc3339()
                                })
                                .unwrap_or_default();

                            let mut msg_count = 0;
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                if let Ok(val) = serde_json::from_str::<Value>(&content) {
                                    if let Some(msgs) =
                                        val.get("messages").and_then(|v| v.as_array())
                                    {
                                        msg_count = msgs.len();
                                    }
                                }
                            }

                            sessions_list.push(serde_json::json!({
                                "session_key": session_key,
                                "size_bytes": size_bytes,
                                "message_count": msg_count,
                                "last_updated": last_updated
                            }));
                        }
                    }
                }
                Ok(serde_json::json!({
                    "status": "success",
                    "sessions": sessions_list
                }))
            }
            "prune" => {
                crate::tools::subagent::cleanup_stale_resources();

                let older_than_days = arguments
                    .get("older_than_days")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(7);
                let outputs_dir = openz_dir.join("tool_outputs");
                let mut files_removed = 0;
                let mut bytes_reclaimed = 0;

                if let Ok(entries) = std::fs::read_dir(&outputs_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                        if path.is_file() && name.starts_with("output_") && name.ends_with(".json")
                        {
                            if let Ok(metadata) = entry.metadata() {
                                if let Ok(modified) = metadata.modified() {
                                    if let Ok(elapsed) = modified.elapsed() {
                                        if elapsed.as_secs() > older_than_days * 86400 {
                                            let size = metadata.len();
                                            if std::fs::remove_file(&path).is_ok() {
                                                files_removed += 1;
                                                bytes_reclaimed += size;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Successfully pruned {} output files and ran stale worktree cleanup.", files_removed),
                    "details": {
                        "files_removed": files_removed,
                        "bytes_reclaimed": bytes_reclaimed,
                        "stale_worktree_cleanup_ran": true
                    }
                }))
            }
            "archive" => {
                let session_key = arguments
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'session_key' for action 'archive'"))?;
                let session_file = sessions_dir.join(format!("{}.json", session_key));
                let lock_file = sessions_dir.join(format!("{}.lock", session_key));

                if !session_file.exists() {
                    return Err(anyhow::anyhow!("Session '{}' does not exist.", session_key));
                }

                let archives_dir = openz_dir.join("archives");
                std::fs::create_dir_all(&archives_dir)?;

                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let archive_file = archives_dir.join(format!("{}_{}.json", session_key, timestamp));

                let size = session_file.metadata().map(|m| m.len()).unwrap_or(0);
                std::fs::copy(&session_file, &archive_file)?;
                std::fs::remove_file(&session_file)?;
                if lock_file.exists() {
                    let _ = std::fs::remove_file(&lock_file);
                }

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Session '{}' successfully archived.", session_key),
                    "details": {
                        "files_removed": 1,
                        "bytes_reclaimed": size
                    }
                }))
            }
            "export" => {
                let session_key = arguments
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing session_key for export"))?;
                let safe_key = session_key.replace(":", "_").replace("/", "_");
                let session_file = sessions_dir.join(format!("{}.json", safe_key));
                if !session_file.exists() {
                    return Err(anyhow::anyhow!("Session does not exist: {}", session_key));
                }
                let content = std::fs::read_to_string(&session_file)?;
                let session: Value = serde_json::from_str(&content)?;
                Ok(
                    serde_json::json!({ "status": "success", "session_key": session_key, "session": session }),
                )
            }
            "delete" => {
                let session_key = arguments
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'session_key' for action 'delete'"))?;
                let session_file = sessions_dir.join(format!("{}.json", session_key));
                let lock_file = sessions_dir.join(format!("{}.lock", session_key));

                if !session_file.exists() {
                    return Err(anyhow::anyhow!("Session '{}' does not exist.", session_key));
                }

                let size = session_file.metadata().map(|m| m.len()).unwrap_or(0);
                std::fs::remove_file(&session_file)?;
                if lock_file.exists() {
                    let _ = std::fs::remove_file(&lock_file);
                }

                Ok(serde_json::json!({
                    "status": "success",
                    "message": format!("Session '{}' permanently deleted.", session_key),
                    "details": {
                        "files_removed": 1,
                        "bytes_reclaimed": size
                    }
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action")),
        }
    }
}

