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

pub(crate) fn tool_repetition_block_threshold(tool_name: &str, args: &serde_json::Value) -> usize {
    if is_progress_sensitive_repeat(tool_name, args) {
        1
    } else {
        2
    }
}

fn is_progress_sensitive_repeat(tool_name: &str, args: &serde_json::Value) -> bool {
    if matches!(
        tool_name,
        "web_fetch"
            | "web_search"
            | "crawl"
            | "crawl_site"
            | "social_search"
            | "searchxyz_search_web"
            | "searchxyz_read_url"
            | "searchxyz_search_and_read"
            | "searchxyz_deep_research"
            | "searchxyz_site_map"
            | "searchxyz_read_github_repo"
            | "read_file"
            | "list_dir"
            | "find_files"
            | "grep_search"
            | "ast_grep"
            | "code_outline"
            | "doc_reader"
            | "system_info"
            | "check_port"
            | "browser_status"
            | "git_manager"
            | "open_path"
            | "device_inventory"
            | "retrieve_original"
            | "scope_context"
            | "compress_content"
            | "search_nodes"
            | "open_nodes"
            | "read_graph"
            | "recall_memory"
            | "proactive_recall"
            | "semantic_search"
            | "rust_docs"
            | "get_working_memory"
            | "list_jobs"
    ) {
        return true;
    }

    if tool_name == "gsd_browser" {
        return matches!(
            args.get("action").and_then(|v| v.as_str()),
            Some("snapshot" | "page_source" | "accessibility_tree" | "screenshot" | "eval")
        );
    }

    if tool_name == "obscura_browser" || tool_name == "firefox_browser" {
        return matches!(
            args.get("action").and_then(|v| v.as_str()),
            Some("render" | "screenshot" | "eval" | "page_source" | "accessibility_tree")
        );
    }

    false
}

fn tool_call_id(call: &serde_json::Value) -> Option<&str> {
    call.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| call.get("tool_call_id").and_then(|v| v.as_str()))
}

fn tool_result_signature(
    messages: &[Message],
    call_id: Option<&str>,
    tool_name: &str,
) -> Option<String> {
    messages.iter().find_map(|msg| {
        if msg.role != "tool" {
            return None;
        }
        let same_call = call_id
            .is_some_and(|id| msg.extra.get("tool_call_id").and_then(|v| v.as_str()) == Some(id));
        let same_name = msg.extra.get("name").and_then(|v| v.as_str()) == Some(tool_name);
        if same_call || (call_id.is_none() && same_name) {
            let mut hasher = Sha256::new();
            hasher.update(msg.content.as_bytes());
            Some(format!("sha256:{:x}", hasher.finalize()))
        } else {
            None
        }
    })
}

pub(crate) fn count_previous_tool_calls(
    messages: &[Message],
    tool_name: &str,
    tool_args: &serde_json::Value,
) -> usize {
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let turn_messages = &messages[last_user_idx..];
    let progress_sensitive_repeat = is_progress_sensitive_repeat(tool_name, tool_args);
    let mut seen_signatures = std::collections::HashSet::new();
    let mut count = 0;
    for (idx, msg) in turn_messages.iter().enumerate() {
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
                            } else if let Some(args_str) = args_val.as_str() {
                                if let Ok(parsed) =
                                    serde_json::from_str::<serde_json::Value>(args_str)
                                {
                                    parsed == *tool_args
                                } else {
                                    false
                                }
                            } else {
                                args_val == tool_args
                            };
                            if match_args {
                                if progress_sensitive_repeat {
                                    let signature = tool_result_signature(
                                        &turn_messages[idx + 1..],
                                        tool_call_id(tc),
                                        tool_name,
                                    );
                                    if signature
                                        .as_ref()
                                        .is_some_and(|sig| seen_signatures.insert(sig.clone()))
                                    {
                                        continue;
                                    }
                                }
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
    fn research_browser_dependency_errors_get_actionable_hint() {
        let hint = generate_self_healing_hint(
            "searchxyz_search_and_read",
            "Failed to start geckodriver on port 4444",
        );
        assert!(hint.contains("inspect_browsers"));
        assert!(hint.contains("browser fallback"));
    }

    #[test]
    fn openmedia_svg_errors_get_schema_specific_hint() {
        let hint = generate_self_healing_hint(
            "openmedia_create_svg",
            "MCP Error: missing field `content`",
        );
        assert!(hint.contains("width"));
        assert!(hint.contains("elements"));
        assert!(hint.contains("content"));
        assert!(hint.contains("text_anchor"));
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

    fn assistant_named_tool_call(name: &str, id: &str, arguments: serde_json::Value) -> Message {
        let mut extra = serde_json::Map::new();
        extra.insert(
            "tool_calls".to_string(),
            json!([{ "id": id, "name": name, "arguments": arguments }]),
        );
        Message {
            role: "assistant".to_string(),
            content: String::new(),
            timestamp: None,
            extra,
        }
    }

    fn tool_result(id: &str, name: &str, content: &str) -> Message {
        let mut extra = serde_json::Map::new();
        extra.insert("tool_call_id".to_string(), json!(id));
        extra.insert("name".to_string(), json!(name));
        Message {
            role: "tool".to_string(),
            content: content.to_string(),
            timestamp: None,
            extra,
        }
    }

    #[test]
    fn repeated_read_only_tools_with_new_state_are_not_loops() {
        let args = json!({ "query": "openz" });
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "research openz".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            assistant_named_tool_call("web_search", "call_1", args.clone()),
            tool_result("call_1", "web_search", "result page 1"),
            assistant_named_tool_call("web_search", "call_2", args.clone()),
            tool_result("call_2", "web_search", "result page 2"),
        ];

        assert_eq!(count_previous_tool_calls(&messages, "web_search", &args), 0);
    }

    #[test]
    fn repeated_read_only_tools_with_same_state_count_as_stale() {
        let args = json!({ "query": "openz" });
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "research openz".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            assistant_named_tool_call("web_search", "call_1", args.clone()),
            tool_result("call_1", "web_search", "same result page"),
            assistant_named_tool_call("web_search", "call_2", args.clone()),
            tool_result("call_2", "web_search", "same result page"),
        ];

        assert_eq!(count_previous_tool_calls(&messages, "web_search", &args), 1);
    }

    #[test]
    fn read_only_stale_repeats_block_after_one_duplicate_signature() {
        let args = json!({ "query": "orchestrate_workflow" });

        assert_eq!(tool_repetition_block_threshold("grep_search", &args), 1);
        assert_eq!(
            tool_repetition_block_threshold("read_file", &json!({ "path": "src/lib.rs" })),
            1
        );
        assert_eq!(
            tool_repetition_block_threshold(
                "write_file",
                &json!({ "path": "/tmp/x", "content": "x" })
            ),
            2
        );
    }

    #[test]
    fn repeated_mutating_tools_still_count_even_with_different_outputs() {
        let args = json!({ "path": "/tmp/file.txt", "content": "x" });
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "write file".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            assistant_named_tool_call("write_file", "call_1", args.clone()),
            tool_result("call_1", "write_file", "ok 1"),
            assistant_named_tool_call("write_file", "call_2", args.clone()),
            tool_result("call_2", "write_file", "ok 2"),
        ];

        assert_eq!(count_previous_tool_calls(&messages, "write_file", &args), 2);
    }

    #[test]
    fn repeated_gsd_browser_snapshots_with_new_state_are_not_loops() {
        let args = json!({ "action": "snapshot" });
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "use browser and inspect page".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            assistant_named_tool_call("gsd_browser", "call_1", args.clone()),
            tool_result("call_1", "gsd_browser", "page state 1"),
            assistant_named_tool_call("gsd_browser", "call_2", args.clone()),
            tool_result("call_2", "gsd_browser", "page state 2"),
        ];

        assert_eq!(
            count_previous_tool_calls(&messages, "gsd_browser", &args),
            0
        );
    }

    #[test]
    fn repeated_gsd_browser_snapshots_with_same_state_count_as_stale() {
        let args = json!({ "action": "snapshot" });
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "use browser and inspect page".to_string(),
                timestamp: None,
                extra: serde_json::Map::new(),
            },
            assistant_named_tool_call("gsd_browser", "call_1", args.clone()),
            tool_result("call_1", "gsd_browser", "same page state"),
            assistant_named_tool_call("gsd_browser", "call_2", args.clone()),
            tool_result("call_2", "gsd_browser", "same page state"),
        ];

        assert_eq!(
            count_previous_tool_calls(&messages, "gsd_browser", &args),
            1
        );
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

fn is_research_or_browser_tool(tool_lower: &str) -> bool {
    tool_lower.contains("search")
        || tool_lower.contains("research")
        || tool_lower.contains("fetch")
        || tool_lower.contains("crawl")
        || tool_lower.contains("browser")
        || tool_lower.contains("obscura")
        || tool_lower.contains("firefox")
        || tool_lower.contains("gsd")
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

    if tool_lower == "openmedia_create_svg" {
        return "OpenMedia SVG schema: pass width, height, and elements (or alias shapes). Valid element type values are rect, circle, line, and text. Text uses content (alias text is accepted by OpenZ), x, y, fill, font_size, font_family, font_weight, and text_anchor=middle for centered logos. Use line with stroke, stroke_width, and stroke_linecap for diagonals instead of unsupported path-like guesses. Optional output_path copies the generated SVG to your requested location.".to_string();
    }

    if is_research_or_browser_tool(&tool_lower) {
        let failure = super::research_policy::classify_research_failure(error_str);
        match failure {
            super::research_policy::ResearchFailureKind::Captcha => {
                return "Research browser fallback hit an anti-bot or CAPTCHA page. Do not keep retrying the same browser path; switch to official URLs, cached sources, or return a partial answer with an unverified-source caveat.".to_string();
            }
            super::research_policy::ResearchFailureKind::BrowserDependencyMissing => {
                return "Research browser fallback cannot run because a browser dependency is missing. Use inspect_browsers to check Chrome CDP, gsd-browser, and geckodriver status before trying another browser tool.".to_string();
            }
            super::research_policy::ResearchFailureKind::BrowserSessionLost => {
                return "Research browser fallback lost its active browser session. Use inspect_browsers, restart the browser backend once if healthy, then stop retrying that backend if the same failure repeats.".to_string();
            }
            super::research_policy::ResearchFailureKind::SearchExhausted => {
                return "Search backends returned no usable results. Run searchxyz_doctor for backend health, then use configured fallback policy or answer from cached/official sources with a clear source caveat.".to_string();
            }
            _ if failure.is_retryable() => {
                return "Research lookup failed with a transient backend/network condition. Retry within the configured research budget; after budget exhaustion, return partial findings and cite which sources could not be verified.".to_string();
            }
            _ => {}
        }
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
