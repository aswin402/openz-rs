use crate::session::Message;
use sha2::{Digest, Sha256};

fn read_scene_file_fingerprint(path: &str) -> Option<String> {
    let path = if let Some(stripped) = path.strip_prefix("file://") {
        std::path::PathBuf::from(stripped)
    } else {
        std::path::PathBuf::from(path)
    };
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(metadata.len().to_le_bytes());
    if let Ok(modified) = metadata.modified() {
        if let Ok(elapsed) = modified.duration_since(std::time::UNIX_EPOCH) {
            hasher.update(elapsed.as_nanos().to_le_bytes());
        }
    }
    hasher.update(&bytes);
    Some(format!("sha256:{:x}", hasher.finalize()))
}

fn scene_file_arg_path(args: &serde_json::Value) -> Option<&str> {
    let obj = args.as_object()?;
    obj.get("scene_path")
        .or_else(|| obj.get("scenePath"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            obj.get("scene").and_then(|v| {
                let s = v.as_str()?;
                if s.ends_with(".json") || s.starts_with("file://") {
                    Some(s)
                } else {
                    None
                }
            })
        })
}

pub(crate) fn tool_arg_fingerprint(args: &serde_json::Value) -> Option<String> {
    read_scene_file_fingerprint(scene_file_arg_path(args)?)
}

pub(crate) fn count_previous_tool_calls(
    messages: &[Message],
    tool_name: &str,
    tool_args: &serde_json::Value,
) -> usize {
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let mut count = 0;
    for msg in &messages[last_user_idx..] {
        if msg.role == "assistant" {
            if let Some(tool_calls) = msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tool_calls {
                    let name = tc.get("name").and_then(|v| v.as_str()).or_else(|| {
                        tc.get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str())
                    });
                    let args = tc
                        .get("arguments")
                        .or_else(|| tc.get("function").and_then(|f| f.get("arguments")));

                    if let (Some(name_str), Some(args_val)) = (name, args) {
                        if name_str == tool_name {
                            let current_fingerprint = tool_arg_fingerprint(tool_args);
                            let previous_fingerprint =
                                tc.get("_openz_arg_fingerprint").and_then(|v| v.as_str());

                            let match_args = if let Some(current) = current_fingerprint.as_deref() {
                                previous_fingerprint == Some(current)
                            } else if previous_fingerprint.is_some()
                                && scene_file_arg_path(tool_args).is_some()
                            {
                                false
                            } else if args_val.is_string() {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(
                                    args_val.as_str().unwrap(),
                                ) {
                                    parsed == *tool_args
                                } else {
                                    false
                                }
                            } else {
                                args_val == tool_args
                            };
                            if match_args {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    count
}

pub(crate) fn count_previous_text_responses(messages: &[Message], next_content: &str) -> usize {
    if next_content.trim().is_empty() {
        return 0;
    }
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let mut count = 0;
    let next_trimmed = next_content.trim();
    for msg in &messages[last_user_idx..] {
        if msg.role == "assistant"
            && !msg.content.trim().is_empty()
            && msg.content.trim() == next_trimmed
        {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assistant_tool_call(arguments: serde_json::Value) -> Message {
        let mut extra = serde_json::Map::new();
        extra.insert(
            "tool_calls".to_string(),
            json!([{
                "name": "openmedia_video_create",
                "arguments": arguments,
                "_openz_arg_fingerprint": "old-file-state"
            }]),
        );
        Message {
            role: "assistant".to_string(),
            content: String::new(),
            timestamp: None,
            extra,
        }
    }

    #[test]
    fn scene_path_calls_with_changed_file_fingerprint_are_not_counted_as_duplicate() {
        let args = json!({
            "scene_path": "/tmp/openz_scene.json",
            "output_path": "/tmp/out.mp4"
        });
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "make video".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            assistant_tool_call(args.clone()),
        ];

        assert_eq!(
            count_previous_tool_calls(&messages, "openmedia_video_create", &args),
            0
        );
    }

    #[test]
    fn unchanged_scene_path_fingerprint_counts_as_duplicate() {
        let path =
            std::env::temp_dir().join(format!("openz_loop_control_{}.json", std::process::id()));
        std::fs::write(&path, r#"{"width":1}"#).unwrap();
        let args = json!({
            "scene_path": path.to_string_lossy(),
            "output_path": "/tmp/out.mp4"
        });
        let fingerprint = tool_arg_fingerprint(&args).unwrap();
        let mut extra = serde_json::Map::new();
        extra.insert(
            "tool_calls".to_string(),
            json!([{
                "name": "openmedia_video_create",
                "arguments": args.clone(),
                "_openz_arg_fingerprint": fingerprint
            }]),
        );
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "make video".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            Message {
                role: "assistant".to_string(),
                content: String::new(),
                timestamp: None,
                extra,
            },
        ];

        assert_eq!(
            count_previous_tool_calls(&messages, "openmedia_video_create", &args),
            1
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn openmedia_video_errors_get_schema_specific_hint() {
        let hint = generate_self_healing_hint(
            "openmedia_video_create",
            "MCP Error: missing field `anchor`",
        );
        assert!(hint.contains("type=text"));
        assert!(hint.contains("style.font_weight"));
        assert!(hint.contains("scene_path"));
    }

    #[test]
    fn exact_non_file_tool_calls_are_still_counted_as_duplicates() {
        let args = json!({ "query": "openz" });
        let mut extra = serde_json::Map::new();
        extra.insert(
            "tool_calls".to_string(),
            json!([{ "name": "web_search", "arguments": args.clone() }]),
        );
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "search".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            Message {
                role: "assistant".to_string(),
                content: String::new(),
                timestamp: None,
                extra,
            },
        ];

        assert_eq!(count_previous_tool_calls(&messages, "web_search", &args), 1);
    }
}

pub(crate) fn generate_self_healing_hint(tool_name: &str, error_str: &str) -> String {
    let err_lower = error_str.to_lowercase();
    let tool_lower = tool_name.to_lowercase();

    if tool_lower.contains("read")
        || tool_lower.contains("write")
        || tool_lower.contains("patch")
        || tool_lower.contains("replace")
        || tool_lower.contains("file")
    {
        if err_lower.contains("notfound") || err_lower.contains("no such file") {
            return "The target path does not exist. Ensure the file path is correct and absolute. You can use 'list_dir' or 'find_files' to check the folder contents.".to_string();
        }
        if err_lower.contains("permission") || err_lower.contains("denied") {
            return "Permission denied. The agent process does not have access to read/write this path. Ensure you are targeting files within the permitted workspace folder.".to_string();
        }
    }

    if tool_lower.contains("exec") || tool_lower.contains("shell") || tool_lower.contains("command")
    {
        if err_lower.contains("permission") || err_lower.contains("denied") {
            return "Execution permission denied. You may need to make the file executable via 'chmod +x <path>' or run the script using an explicit interpreter (e.g. 'bash <script_path>').".to_string();
        }
        if err_lower.contains("not found")
            || err_lower.contains("127")
            || err_lower.contains("no such file")
        {
            return "Command or script executable not found. Verify the path or binary name is correct and check if the required tool is installed on the system.".to_string();
        }
        if err_lower.contains("seccomp")
            || err_lower.contains("sandbox")
            || err_lower.contains("operation not permitted")
        {
            return "Operation blocked by the seccomp BPF sandbox. Note that networking syscalls (e.g. curl, wget, git push), mount/umount, and other privileged actions are forbidden in the sandboxed environment. Please perform the action without network or sandbox-restricted system calls, or run locally via a different approved script if possible.".to_string();
        }
    }

    if tool_lower == "openmedia_video_create" || tool_lower == "openmedia_video_preview" {
        return "OpenMedia VideoScene schema: pass `scene` as a structured object or `scene_path` as a .json file. Top level requires width, height, fps, duration, background, scenes. Each scene requires id, start, end, elements. Text elements use type=text with content, position {x,y}, anchor, and style.font_family, style.font_size, style.font_weight (number), style.color, style.text_align. Valid transition types include crossfade, dissolve, blur, glitch, radial_wipe; do not use fade_in/fade_out as transitions or rect/circle as element types.".to_string();
    }

    if err_lower.contains("mcp")
        || err_lower.contains("connection")
        || err_lower.contains("broken pipe")
        || err_lower.contains("bridge")
    {
        return "MCP server connection error. The MCP server process might be offline or failed to initialize. Try using the 'manage_mcp' tool to list, configure, or restart the active MCP servers.".to_string();
    }

    if tool_lower.contains("delegate")
        || tool_lower.contains("research")
        || tool_lower.contains("optimizer")
        || tool_lower.contains("loop")
    {
        return "Subagent execution encountered an error. You can use the 'optimize_subagent' tool to refine the subagent system prompt/instructions to handle the issue better, or try breaking the goal down into smaller, simpler tasks for subagent delegation.".to_string();
    }

    "Please double-check the arguments format, verify the target file paths or command options exist, and try a different tool or approach if this error persists.".to_string()
}
