use super::{
    extract_first_str, extract_trimmed_str, parse_u64_value, to_non_empty_trimmed, ACTION_KEYS,
};
use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct ManageSessionsTool;

const SESSION_KEY_ALIASES: &[&str] = &[
    "session_key",
    "sessionKey",
    "key",
    "id",
    "session_id",
    "sessionId",
    "session",
    "target",
];

fn resolve_session_path(
    sessions_dir: &Path,
    session_key: &str,
) -> (PathBuf, PathBuf, String) {
    let key = session_key.trim();
    let safe_key = crate::session::SessionManager::safe_key(key);
    let direct_json = sessions_dir.join(format!("{}.json", key));
    let safe_json = sessions_dir.join(format!("{}.json", safe_key));
    let direct_lock = sessions_dir.join(format!("{}.lock", key));
    let safe_lock = sessions_dir.join(format!("{}.lock", safe_key));

    match (safe_json.exists(), direct_json.exists()) {
        (true, _) => (safe_json, safe_lock, safe_key),
        (false, true) => (direct_json, direct_lock, key.to_string()),
        (false, false) => (safe_json, safe_lock, safe_key),
    }
}

fn extract_session_key(arguments: &Value, direct_key: Option<&str>) -> Option<String> {
    to_non_empty_trimmed(direct_key)
        .or_else(|| extract_trimmed_str(arguments, SESSION_KEY_ALIASES))
}

fn resolve_existing_session(
    arguments: &Value,
    direct_key: Option<&str>,
    sessions_dir: &Path,
    action_label: &str,
) -> Result<(String, PathBuf, PathBuf, String)> {
    let session_key = extract_session_key(arguments, direct_key)
        .ok_or_else(|| anyhow::anyhow!("Missing 'session_key' for action '{}'", action_label))?;
    let (session_file, lock_file, safe_key) = resolve_session_path(sessions_dir, &session_key);
    session_file.exists().then_some(()).ok_or_else(|| {
        anyhow::anyhow!("Session '{}' does not exist.", session_key)
    })?;
    Ok((session_key, session_file, lock_file, safe_key))
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
        let (action_str, direct_key) = match arguments {
            Value::String(s) => {
                let s_trim = s.trim();
                match s_trim.to_lowercase().as_str() {
                    "list" | "ls" | "show" | "view" => (Some("list"), None),
                    "prune" | "clean" | "cleanup" => (Some("prune"), None),
                    _ => (Some("export"), Some(s_trim)),
                }
            }
            Value::Object(_) => (extract_first_str(arguments, ACTION_KEYS), None),
            _ => (None, None),
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
                let sessions_list: Vec<Value> = std::fs::read_dir(&sessions_dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|entry| {
                        let path = entry.path();
                        let is_json = path.is_file()
                            && path.extension().and_then(|s| s.to_str()) == Some("json");
                        is_json.then(|| {
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
                            let msg_count = std::fs::read_to_string(&path)
                                .ok()
                                .and_then(|content| serde_json::from_str::<Value>(&content).ok())
                                .and_then(|val| {
                                    val.get("messages").and_then(|v| v.as_array()).map(Vec::len)
                                })
                                .unwrap_or(0);

                            serde_json::json!({
                                "session_key": session_key,
                                "size_bytes": size_bytes,
                                "message_count": msg_count,
                                "last_updated": last_updated
                            })
                        })
                    })
                    .collect();

                Ok(serde_json::json!({
                    "status": "success",
                    "sessions": sessions_list
                }))
            }
            "prune" => {
                crate::tools::subagent::cleanup_stale_resources();

                let older_than_days = arguments
                    .get("older_than_days")
                    .and_then(parse_u64_value)
                    .unwrap_or(7);
                let outputs_dir = openz_dir.join("tool_outputs");
                let mut files_removed = 0usize;
                let mut bytes_reclaimed = 0u64;

                for entry in std::fs::read_dir(&outputs_dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    let path = entry.path();
                    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    let eligible_size = (path.is_file()
                        && name.starts_with("output_")
                        && name.ends_with(".json"))
                    .then(|| entry.metadata().ok())
                    .flatten()
                    .and_then(|meta| {
                        let elapsed = meta.modified().ok()?.elapsed().ok()?;
                        (elapsed.as_secs() > older_than_days * 86400).then_some(meta.len())
                    });

                    match eligible_size {
                        Some(size) => match std::fs::remove_file(&path).is_ok() {
                            true => {
                                files_removed += 1;
                                bytes_reclaimed += size;
                            }
                            false => {}
                        },
                        None => {}
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
                let (session_key, session_file, lock_file, safe_key) =
                    resolve_existing_session(arguments, direct_key, &sessions_dir, "archive")?;

                let archives_dir = openz_dir.join("archives");
                std::fs::create_dir_all(&archives_dir)?;

                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let archive_file = archives_dir.join(format!("{}_{}.json", safe_key, timestamp));

                let size = session_file.metadata().map(|m| m.len()).unwrap_or(0);
                std::fs::copy(&session_file, &archive_file)?;
                std::fs::remove_file(&session_file)?;
                let _ = std::fs::remove_file(&lock_file);

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
                let (session_key, session_file, _, _) =
                    resolve_existing_session(arguments, direct_key, &sessions_dir, "export")?;
                let content = std::fs::read_to_string(&session_file)?;
                let session: Value = serde_json::from_str(&content)?;
                Ok(serde_json::json!({
                    "status": "success",
                    "session_key": session_key,
                    "session": session
                }))
            }
            "delete" => {
                let (session_key, session_file, lock_file, _) =
                    resolve_existing_session(arguments, direct_key, &sessions_dir, "delete")?;

                let size = session_file.metadata().map(|m| m.len()).unwrap_or(0);
                std::fs::remove_file(&session_file)?;
                let _ = std::fs::remove_file(&lock_file);

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
