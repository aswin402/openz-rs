use crate::session::Message;
use crate::tools::arguments::{PATH_KEYS, QUERY_KEYS};
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
    crate::tools::metadata::tool_repetition_block_threshold(tool_name, args)
}

fn is_progress_sensitive_repeat(tool_name: &str, args: &serde_json::Value) -> bool {
    crate::tools::metadata::is_progress_sensitive_repeat(tool_name, args)
}

fn tool_call_id(call: &serde_json::Value) -> Option<&str> {
    call.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| call.get("tool_call_id").and_then(|v| v.as_str()))
}

fn exact_arg_repeat_counts_without_result_signature(tool_name: &str) -> bool {
    crate::tools::metadata::exact_arg_repeat_counts_without_result_signature(tool_name)
}

fn object_arg_value<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<&'a serde_json::Value> {
    keys.iter().find_map(|key| map.get(*key))
}

fn canonical_tool_args_key(tool_name: &str, args: &serde_json::Value) -> Option<String> {
    let parsed;
    let args = if let Some(raw) = args.as_str() {
        parsed = serde_json::from_str::<serde_json::Value>(raw).ok()?;
        &parsed
    } else {
        args
    };
    let map = args.as_object()?;

    match tool_name {
        "grep_search" => {
            let query = object_arg_value(map, QUERY_KEYS)
                .and_then(|v| v.as_str())?
                .trim();
            let dir = object_arg_value(map, &["dir", "directory", "root", "path"])
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .trim();
            let is_regex = object_arg_value(map, &["is_regex", "isRegex", "regex"])
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(format!(
                "grep_search|query={query}|dir={dir}|regex={is_regex}"
            ))
        }
        "read_file" | "view_file" => {
            let path = object_arg_value(
                map,
                PATH_KEYS,
            )
            .and_then(|v| v.as_str())?
            .trim();
            let start_line =
                object_arg_value(map, &["start_line", "startLine"]).and_then(|v| v.as_u64());
            let end_line = object_arg_value(map, &["end_line", "endLine"]).and_then(|v| v.as_u64());
            Some(format!(
                "{tool_name}|path={path}|start={start_line:?}|end={end_line:?}"
            ))
        }
        _ => None,
    }
}

fn normalized_json_arg(args: &serde_json::Value) -> Option<serde_json::Value> {
    if let Some(raw) = args.as_str() {
        serde_json::from_str::<serde_json::Value>(raw).ok()
    } else {
        Some(args.clone())
    }
}

fn tool_args_match(
    tool_name: &str,
    current: &serde_json::Value,
    previous: &serde_json::Value,
) -> bool {
    if let (Some(current_key), Some(previous_key)) = (
        canonical_tool_args_key(tool_name, current),
        canonical_tool_args_key(tool_name, previous),
    ) {
        return current_key == previous_key;
    }

    match (normalized_json_arg(current), normalized_json_arg(previous)) {
        (Some(current), Some(previous)) => current == previous,
        _ => previous == current,
    }
}

fn is_state_changing_tool(tool_name: &str) -> bool {
    crate::tools::metadata::is_state_changing_tool(tool_name)
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
    let exact_arg_repeat = exact_arg_repeat_counts_without_result_signature(tool_name);
    let mut state_epoch = 0usize;
    let mut call_epochs = Vec::new();

    for msg in turn_messages {
        if msg.role != "assistant" {
            continue;
        }
        if let Some(tool_calls) = msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
            for tc in tool_calls {
                let name = tc.get("name").and_then(|v| v.as_str()).or_else(|| {
                    tc.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|v| v.as_str())
                });
                call_epochs.push(state_epoch);
                if let Some(name_str) = name {
                    if is_state_changing_tool(name_str) {
                        state_epoch += 1;
                    }
                }
            }
        }
    }
    let current_state_epoch = state_epoch;

    let mut seen_signatures = std::collections::HashSet::new();
    let mut count = 0;
    let mut call_index = 0usize;
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
                    let call_epoch = call_epochs.get(call_index).copied().unwrap_or(0);
                    call_index += 1;

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
                            } else {
                                tool_args_match(tool_name, tool_args, args_val)
                            };
                            if match_args {
                                if exact_arg_repeat && call_epoch == current_state_epoch {
                                    count += 1;
                                    continue;
                                }
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



fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn is_research_or_browser_tool(tool_lower: &str) -> bool {
    contains_any(
        tool_lower,
        &[
            "search", "research", "fetch", "crawl", "browser", "obscura", "firefox", "gsd",
        ],
    )
}

fn tool_not_found_hint(err_lower: &str) -> Option<&'static str> {
    (err_lower.starts_with("tool '") && err_lower.ends_with("' not found")).then_some(
        "Requested tool is not currently loaded or does not exist. Call 'tool_catalog' (alias 'tool_search') with `query: \"<keyword>\"` to discover matching tools and automatically promote them into the active sticky working set, or use `batch_execute` on 'tool_catalog' to execute deferred tools directly.",
    )
}

fn filesystem_self_healing_hint(tool_lower: &str, err_lower: &str) -> Option<&'static str> {
    contains_any(tool_lower, &["read", "write", "patch", "replace", "file"])
        .then(|| {
            contains_any(
                err_lower,
                &[
                    "hashline mismatch",
                    "expected_hash",
                    "exceeds total lines",
                    "cannot be greater than end_line",
                ],
            )
            .then_some(
                "Line numbers or content hash drifted since the file was last read. Call 'read_file' with `include_hashes: true` around the target lines to obtain fresh `LINE:HASH|` anchors, or pass `anchor_text` to 'replace_lines' for automatic drift relocation.",
            )
            .or_else(|| {
                contains_any(
                    err_lower,
                    &[
                        "failed to match hunk",
                        "target substring not found",
                        "no valid unified diff hunks",
                    ],
                )
                .then_some(
                    "Patch context or target block did not match current file contents. Read the surrounding lines with 'read_file' (`include_hashes: true`), then either pass a `<<<<<<< SEARCH ... ======= ... >>>>>>> REPLACE` block or `edits: [{\"target\": \"...\", \"replacement\": \"...\"}]` to 'patch_file', or use 'replace_lines' with `expected_hash`.",
                )
            })
            .or_else(|| {
                contains_any(err_lower, &["notfound", "no such file"]).then_some(
                    "The target path does not exist. Ensure the file path is correct and absolute. You can use 'list_dir' or 'find_files' to check the folder contents.",
                )
            })
            .or_else(|| {
                contains_any(err_lower, &["permission", "denied"]).then_some(
                    "Permission denied. The agent process does not have access to read/write this path. Ensure you are targeting files within the permitted workspace folder.",
                )
            })
        })
        .flatten()
}

fn shell_self_healing_hint(tool_lower: &str, err_lower: &str) -> Option<&'static str> {
    contains_any(tool_lower, &["exec", "shell", "command"])
        .then(|| {
            contains_any(err_lower, &["permission", "denied"])
                .then_some(
                    "Execution permission denied. You may need to make the file executable via 'chmod +x <path>' or run the script using an explicit interpreter (e.g. 'bash <script_path>').",
                )
                .or_else(|| {
                    contains_any(err_lower, &["not found", "127", "no such file"]).then_some(
                        "Command or script executable not found. Verify the path or binary name is correct and check if the required tool is installed on the system.",
                    )
                })
                .or_else(|| {
                    contains_any(err_lower, &["seccomp", "sandbox", "operation not permitted"]).then_some(
                        "Operation blocked by the seccomp BPF sandbox. Note that networking syscalls (e.g. curl, wget, git push), mount/umount, and other privileged actions are forbidden in the sandboxed environment. Please perform the action without network or sandbox-restricted system calls, or run locally via a different approved script if possible.",
                    )
                })
        })
        .flatten()
}

fn openmedia_self_healing_hint(tool_lower: &str) -> Option<&'static str> {
    match tool_lower {
        "openmedia_video_create" | "openmedia_video_preview" => Some(
            "OpenMedia VideoScene schema: pass `scene` as a structured object or `scene_path` as a .json file. Top level requires width, height, fps, duration, background, scenes. Each scene requires id, start, end, elements. Text elements use type=text with content, position {x,y}, anchor, and style.font_family, style.font_size, style.font_weight (number), style.color, style.text_align. Valid transition types include crossfade, dissolve, blur, glitch, radial_wipe; do not use fade_in/fade_out as transitions or rect/circle as element types.",
        ),
        "openmedia_create_svg" => Some(
            "OpenMedia SVG schema: pass width, height, and elements (or alias shapes). Valid element type values are rect, circle, line, and text. Text uses content (alias text is accepted by OpenZ), x, y, fill, font_size, font_family, font_weight, and text_anchor=middle for centered logos. Use line with stroke, stroke_width, and stroke_linecap for diagonals instead of unsupported path-like guesses. Optional output_path copies the generated SVG to your requested location.",
        ),
        _ => None,
    }
}

fn research_self_healing_hint(tool_lower: &str, error_str: &str) -> Option<&'static str> {
    is_research_or_browser_tool(tool_lower)
        .then(|| {
            let failure = super::research_policy::classify_research_failure(error_str);
            match failure {
                super::research_policy::ResearchFailureKind::Captcha => Some(
                    "Research browser fallback hit an anti-bot or CAPTCHA page. Do not keep retrying the same browser path; switch to official URLs, cached sources, or return a partial answer with an unverified-source caveat.",
                ),
                super::research_policy::ResearchFailureKind::BrowserDependencyMissing => Some(
                    "Research browser fallback cannot run because a browser dependency is missing. Use inspect_browsers to check Chrome CDP, gsd-browser, and geckodriver status before trying another browser tool.",
                ),
                super::research_policy::ResearchFailureKind::BrowserSessionLost => Some(
                    "Research browser fallback lost its active browser session. Use inspect_browsers, restart the browser backend once if healthy, then stop retrying that backend if the same failure repeats.",
                ),
                super::research_policy::ResearchFailureKind::SearchExhausted => Some(
                    "Search backends returned no usable results. Run searchxyz_doctor for backend health, then use configured fallback policy or answer from cached/official sources with a clear source caveat.",
                ),
                other => other.is_retryable().then_some(
                    "Research lookup failed with a transient backend/network condition. Retry within the configured research budget; after budget exhaustion, return partial findings and cite which sources could not be verified.",
                ),
            }
        })
        .flatten()
}

pub(crate) fn generate_self_healing_hint(tool_name: &str, error_str: &str) -> String {
    let err_lower = error_str.to_lowercase();
    let tool_lower = tool_name.to_lowercase();

    tool_not_found_hint(&err_lower)
        .or_else(|| filesystem_self_healing_hint(&tool_lower, &err_lower))
        .or_else(|| shell_self_healing_hint(&tool_lower, &err_lower))
        .or_else(|| openmedia_self_healing_hint(&tool_lower))
        .or_else(|| research_self_healing_hint(&tool_lower, error_str))
        .or_else(|| {
            contains_any(&err_lower, &["mcp", "connection", "broken pipe", "bridge"]).then_some(
                "MCP server connection error. The MCP server process might be offline or failed to initialize. Try using the 'manage_mcp' tool to list, configure, or restart the active MCP servers.",
            )
        })
        .or_else(|| {
            contains_any(&tool_lower, &["delegate", "research", "optimizer", "loop"]).then_some(
                "Subagent execution encountered an error. You can use the 'optimize_subagent' tool to refine the subagent system prompt/instructions to handle the issue better, or try breaking the goal down into smaller, simpler tasks for subagent delegation.",
            )
        })
        .unwrap_or(
            "Please double-check the arguments format, verify the target file paths or command options exist, and try a different tool or approach if this error persists.",
        )
        .to_string()
}

#[cfg(test)]
#[path = "loop_control_tests.rs"]
mod tests;
