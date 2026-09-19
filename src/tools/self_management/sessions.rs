use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;

pub struct ManageSessionsTool;

fn resolve_session_path(
    sessions_dir: &std::path::Path,
    session_key: &str,
) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let key = session_key.trim();
    let safe_key = crate::session::SessionManager::safe_key(key);
    let direct_json = sessions_dir.join(format!("{}.json", key));
    let safe_json = sessions_dir.join(format!("{}.json", safe_key));
    let direct_lock = sessions_dir.join(format!("{}.lock", key));
    let safe_lock = sessions_dir.join(format!("{}.lock", safe_key));

    if safe_json.exists() {
        (safe_json, safe_lock, safe_key)
    } else if direct_json.exists() {
        (direct_json, direct_lock, key.to_string())
    } else {
        (safe_json, safe_lock, safe_key)
    }
}

fn extract_session_key(arguments: &Value, direct_key: Option<&str>) -> Option<String> {
    if let Some(key) = direct_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    arguments
        .get("session_key")
        .or_else(|| arguments.get("sessionKey"))
        .or_else(|| arguments.get("key"))
        .or_else(|| arguments.get("id"))
        .or_else(|| arguments.get("session_id"))
        .or_else(|| arguments.get("sessionId"))
        .or_else(|| arguments.get("session"))
        .or_else(|| arguments.get("target"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

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
        let (action_str, direct_key) = if let Some(s) = arguments.as_str() {
            let s_trim = s.trim();
            match s_trim.to_lowercase().as_str() {
                "list" | "ls" | "show" | "view" => (Some("list"), None),
                "prune" | "clean" | "cleanup" => (Some("prune"), None),
                _ => (Some("export"), Some(s_trim)),
            }
        } else if let Some(obj) = arguments.as_object() {
            let act = obj
                .get("action")
                .or_else(|| obj.get("act"))
                .or_else(|| obj.get("command"))
                .or_else(|| obj.get("cmd"))
                .or_else(|| obj.get("op"))
                .or_else(|| obj.get("mode"))
                .and_then(|v| v.as_str());
            (act, None)
        } else {
            (None, None)
        };

        let normalized_action = action_str
            .map(|a| a.trim().to_lowercase())
            .unwrap_or_else(|| "list".to_string());
        let action = match normalized_action.as_str() {
            "" | "list" | "ls" | "show" | "view" => "list",
            "prune" | "clean" | "cleanup" => "prune",
            "archive" => "archive",
            "export" | "dump" | "get" | "read" => "export",
            "delete" | "remove" | "rm" => "delete",
            other => other,
        };
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
                    .and_then(|v| {
                        v.as_u64().or_else(|| {
                            v.as_str()
                                .and_then(|s| s.trim().parse::<u64>().ok())
                        })
                    })
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
                let session_key = extract_session_key(arguments, direct_key)
                    .ok_or_else(|| anyhow::anyhow!("Missing 'session_key' for action 'archive'"))?;
                let (session_file, lock_file, safe_key) =
                    resolve_session_path(&sessions_dir, &session_key);

                if !session_file.exists() {
                    return Err(anyhow::anyhow!("Session '{}' does not exist.", session_key));
                }

                let archives_dir = openz_dir.join("archives");
                std::fs::create_dir_all(&archives_dir)?;

                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let archive_file = archives_dir.join(format!("{}_{}.json", safe_key, timestamp));

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
                let session_key = extract_session_key(arguments, direct_key)
                    .ok_or_else(|| anyhow::anyhow!("Missing 'session_key' for action 'export'"))?;
                let (session_file, _, _) = resolve_session_path(&sessions_dir, &session_key);
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
                let session_key = extract_session_key(arguments, direct_key)
                    .ok_or_else(|| anyhow::anyhow!("Missing 'session_key' for action 'delete'"))?;
                let (session_file, lock_file, _) = resolve_session_path(&sessions_dir, &session_key);

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

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;

