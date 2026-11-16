use crate::session::Message;
use crate::tools::arguments::{PATH_KEYS, QUERY_KEYS};
use sha2::{Digest, Sha256};

fn read_scene_file_fingerprint(path: &str) -> Option<String> {
    let path = std::path::PathBuf::from(path.strip_prefix("file://").unwrap_or(path));
    let metadata = std::fs::metadata(&path).ok()?;
    metadata.is_file().then_some(())?;
    let bytes = std::fs::read(&path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(metadata.len().to_le_bytes());
    let _ = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| hasher.update(elapsed.as_nanos().to_le_bytes()));
    hasher.update(&bytes);
    Some(format!("sha256:{:x}", hasher.finalize()))
}

fn scene_file_arg_path(args: &serde_json::Value) -> Option<&str> {
    let obj = args.as_object()?;
    obj.get("scene_path")
        .or_else(|| obj.get("scenePath"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            obj.get("scene")
                .and_then(|v| v.as_str())
                .filter(|s| s.ends_with(".json") || s.starts_with("file://"))
        })
}

pub(crate) fn tool_arg_fingerprint(args: &serde_json::Value) -> Option<String> {
    read_scene_file_fingerprint(scene_file_arg_path(args)?)
}

pub(crate) use crate::tools::metadata::tool_repetition_block_threshold;
use crate::tools::metadata::{
    exact_arg_repeat_counts_without_result_signature, is_progress_sensitive_repeat,
    is_state_changing_tool_call,
};
use std::collections::BTreeMap;

/// Active sliding window of recent tool invocations inspected for loop/cycle detection.
/// Prevents legitimate file re-reads 30+ steps later in a 200-step long-horizon turn
/// from being falsely flagged as infinite loops.
pub(crate) const TRAJECTORY_WINDOW_CALLS: usize = 24;
const MAX_PERIODIC_CYCLE_LEN: usize = 6;
const MIN_PERIODIC_REPETITIONS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrajectoryCycle {
    pub period: usize,
    pub repetitions: usize,
    pub cycle_tools: Vec<String>,
}

impl TrajectoryCycle {
    pub(crate) fn format_steering_error(&self, candidate_tool: &str) -> String {
        let trace = self.cycle_tools.join(" -> ");
        format!(
            "Loop detected: Periodic {}-step cycle [{}] repeated {} times with identical outcomes when calling '{}'. Execution was skipped so you can pivot: change your strategy, vary parameters, inspect new context, or synthesize findings.",
            self.period, trace, self.repetitions, candidate_tool
        )
    }
}

fn fnv1a_hash64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |acc, &b| {
        (acc ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}

fn tool_call_id(call: &serde_json::Value) -> Option<&str> {
    call.get("id")
        .and_then(|v| v.as_str())
        .or_else(|| call.get("tool_call_id").and_then(|v| v.as_str()))
}

fn object_arg_value<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<&'a serde_json::Value> {
    keys.iter().find_map(|key| map.get(*key))
}

fn canonicalize_json_structure(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(s.trim().to_string()),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonicalize_json_structure).collect())
        }
        serde_json::Value::Object(map) => {
            let sorted: BTreeMap<String, serde_json::Value> = map
                .iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (crate::tools::to_snake_case(k), canonicalize_json_structure(v)))
                .collect();
            let mut out = serde_json::Map::new();
            for (k, v) in sorted {
                out.insert(k, v);
            }
            serde_json::Value::Object(out)
        }
        other => other.clone(),
    }
}

fn parse_and_canonicalize_args(args: &serde_json::Value) -> serde_json::Value {
    let parsed = match args.as_str() {
        Some(raw) => serde_json::from_str::<serde_json::Value>(raw)
            .unwrap_or_else(|_| serde_json::Value::String(raw.trim().to_string())),
        None => args.clone(),
    };
    let normalized = crate::tools::normalize_tool_args(&parsed);
    canonicalize_json_structure(&normalized)
}

fn canonical_tool_args_key(tool_name: &str, args: &serde_json::Value) -> Option<String> {
    let canonical_name = crate::tools::canonical_tool_name(tool_name);
    let parsed;
    let args = match args.as_str() {
        Some(raw) => {
            parsed = serde_json::from_str::<serde_json::Value>(raw).ok()?;
            &parsed
        }
        None => args,
    };
    let map = args.as_object()?;

    match canonical_name.as_str() {
        "grep_search" => {
            let query = object_arg_value(map, QUERY_KEYS)
                .and_then(|v| v.as_str())?
                .trim();
            let dir = object_arg_value(map, &["dir", "directory", "root", "path"])
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .trim();
            let use_pattern = object_arg_value(map, &["is_regex", "isRegex", "regex"])
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(format!(
                "grep_search|query={query}|dir={dir}|pattern={use_pattern}"
            ))
        }
        "read_file" | "view_file" => {
            let path = object_arg_value(map, PATH_KEYS)
                .and_then(|v| v.as_str())?
                .trim();
            let start_line =
                object_arg_value(map, &["start_line", "startLine"]).and_then(|v| v.as_u64());
            let end_line = object_arg_value(map, &["end_line", "endLine"]).and_then(|v| v.as_u64());
            Some(format!(
                "read_file|path={path}|start={start_line:?}|end={end_line:?}"
            ))
        }
        _ => None,
    }
}

fn canonical_call_fingerprint_hash(tool_name: &str, args: &serde_json::Value) -> u64 {
    let canonical_name = crate::tools::canonical_tool_name(tool_name);
    let key = canonical_tool_args_key(&canonical_name, args).unwrap_or_else(|| {
        let structural = parse_and_canonicalize_args(args);
        format!("{canonical_name}|{structural}")
    });
    fnv1a_hash64(key.as_bytes())
}

fn tool_args_match(
    tool_name: &str,
    current: &serde_json::Value,
    previous: &serde_json::Value,
) -> bool {
    match (
        canonical_tool_args_key(tool_name, current),
        canonical_tool_args_key(tool_name, previous),
    ) {
        (Some(current_key), Some(previous_key)) => current_key == previous_key,
        _ => parse_and_canonicalize_args(current) == parse_and_canonicalize_args(previous),
    }
}

fn find_tool_result_message<'a>(
    messages: &'a [Message],
    call_id: Option<&str>,
    tool_name: &str,
) -> Option<&'a Message> {
    let canonical_target = crate::tools::canonical_tool_name(tool_name);
    messages.iter().find(|msg| {
        msg.role == "tool" && {
            let same_call = call_id.is_some_and(|id| {
                msg.extra.get("tool_call_id").and_then(|v| v.as_str()) == Some(id)
            });
            let same_name = msg
                .extra
                .get("name")
                .and_then(|v| v.as_str())
                .is_some_and(|n| crate::tools::canonical_tool_name(n) == canonical_target);
            same_call || (call_id.is_none() && same_name)
        }
    })
}

fn tool_result_signature(
    messages: &[Message],
    call_id: Option<&str>,
    tool_name: &str,
) -> Option<String> {
    find_tool_result_message(messages, call_id, tool_name).map(|msg| {
        let mut hasher = Sha256::new();
        hasher.update(msg.content.as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    })
}

fn tool_result_is_error(
    messages: &[Message],
    call_id: Option<&str>,
    tool_name: &str,
) -> bool {
    find_tool_result_message(messages, call_id, tool_name).is_some_and(|msg| {
        let trimmed = msg.content.trim();
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(serde_json::Value::Object(map)) => {
                map.get("error").is_some_and(|v| !v.is_null())
                    || map
                        .get("status_code")
                        .and_then(|v| v.as_i64())
                        .is_some_and(|code| code != 0)
                    || map
                        .get("exit_code")
                        .and_then(|v| v.as_i64())
                        .is_some_and(|code| code != 0)
            }
            _ => trimmed.starts_with("Error:"),
        }
    })
}

fn extract_call_name(tc: &serde_json::Value) -> Option<&str> {
    tc.get("name").and_then(|v| v.as_str()).or_else(|| {
        tc.get("function")
            .and_then(|f| f.get("name"))
            .and_then(|v| v.as_str())
    })
}

fn extract_call_args(tc: &serde_json::Value) -> Option<&serde_json::Value> {
    tc.get("arguments")
        .or_else(|| tc.get("function").and_then(|f| f.get("arguments")))
}

struct RecordedCallRef<'a> {
    msg_idx: usize,
    tc: &'a serde_json::Value,
    canonical_name: String,
    args_val: &'a serde_json::Value,
    call_hash: u64,
    mutated_ok: bool,
}

fn collect_turn_call_window(turn_messages: &[Message]) -> Vec<RecordedCallRef<'_>> {
    let mut all_calls = Vec::new();

    for (idx, msg) in turn_messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == "assistant")
    {
        let tool_calls = match msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
            Some(tc) => tc,
            None => continue,
        };
        for tc in tool_calls {
            let (name_str, args_val) = match (extract_call_name(tc), extract_call_args(tc)) {
                (Some(n), Some(a)) => (n, a),
                _ => continue,
            };
            let canonical_name = crate::tools::canonical_tool_name(name_str);
            let parsed_args = parse_and_canonicalize_args(args_val);
            let call_hash = canonical_call_fingerprint_hash(&canonical_name, args_val);
            let mutated_ok = is_state_changing_tool_call(&canonical_name, &parsed_args)
                && !tool_result_is_error(
                    &turn_messages[idx + 1..],
                    tool_call_id(tc),
                    &canonical_name,
                );
            all_calls.push(RecordedCallRef {
                msg_idx: idx,
                tc,
                canonical_name,
                args_val,
                call_hash,
                mutated_ok,
            });
        }
    }

    let window_start = all_calls.len().saturating_sub(TRAJECTORY_WINDOW_CALLS);
    all_calls.into_iter().skip(window_start).collect()
}

fn observation_hash_for_cycle(content: &str) -> u64 {
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(mut val) => {
            let _ = val.as_object_mut().map(|obj| obj.remove("z_guard_diagnostic"));
            let canonical = canonicalize_json_structure(&val);
            fnv1a_hash64(canonical.to_string().as_bytes())
        }
        Err(_) => fnv1a_hash64(content.as_bytes()),
    }
}

/// Generalized suffix-periodicity detector over `(call_hash, obs_hash)` pairs.
/// Detects multi-step alternating/ping-pong loops of period `p` in `2..=6` (e.g.,
/// `A -> B -> A -> B -> A` or `A -> B -> C -> A -> B -> C -> A`) when observations
/// repeat identically across cycles without forward progress.
pub(crate) fn detect_trajectory_cycle(
    messages: &[Message],
    tool_name: &str,
    tool_args: &serde_json::Value,
) -> Option<TrajectoryCycle> {
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let turn_messages = &messages[last_user_idx..];
    let window_calls = collect_turn_call_window(turn_messages);

    let atoms: Vec<(u64, u64, String)> = window_calls
        .iter()
        .map(|entry| {
            let obs_h = find_tool_result_message(
                &turn_messages[entry.msg_idx + 1..],
                tool_call_id(entry.tc),
                &entry.canonical_name,
            )
            .map(|msg| observation_hash_for_cycle(&msg.content))
            .unwrap_or(0);
            (entry.call_hash, obs_h, entry.canonical_name.clone())
        })
        .collect();

    let n = atoms.len();
    let candidate_canonical = crate::tools::canonical_tool_name(tool_name);
    let candidate_hash = canonical_call_fingerprint_hash(&candidate_canonical, tool_args);

    (2..=MAX_PERIODIC_CYCLE_LEN).find_map(|period| {
        (n >= MIN_PERIODIC_REPETITIONS * period).then_some(())?;
        let cycle_slice = &atoms[n - period..n];
        (cycle_slice[0].0 == candidate_hash).then_some(())?;
        let has_distinct_step = cycle_slice.iter().any(|(ch, _, _)| *ch != candidate_hash);
        has_distinct_step.then_some(())?;

        let mut reps = 1usize;
        while n >= (reps + 1) * period {
            let prior_slice = &atoms[n - (reps + 1) * period..n - reps * period];
            let matches_cycle = prior_slice
                .iter()
                .zip(cycle_slice.iter())
                .all(|((c1, o1, _), (c2, o2, _))| c1 == c2 && o1 == o2);
            match matches_cycle {
                true => reps += 1,
                false => break,
            }
        }

        (reps >= MIN_PERIODIC_REPETITIONS).then(|| TrajectoryCycle {
            period,
            repetitions: reps,
            cycle_tools: cycle_slice.iter().map(|(_, _, name)| name.clone()).collect(),
        })
    })
}

pub(crate) fn count_previous_tool_calls(
    messages: &[Message],
    tool_name: &str,
    tool_args: &serde_json::Value,
) -> usize {
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let turn_messages = &messages[last_user_idx..];
    let target_canonical = crate::tools::canonical_tool_name(tool_name);
    let progress_sensitive_repeat = is_progress_sensitive_repeat(&target_canonical, tool_args);
    let exact_arg_repeat = exact_arg_repeat_counts_without_result_signature(&target_canonical);
    let window_calls = collect_turn_call_window(turn_messages);

    let mut seen_signatures = std::collections::HashSet::new();
    let mut count = 0;
    for (pos, entry) in window_calls.iter().enumerate() {
        let intervening_distinct_mutation = window_calls[pos + 1..]
            .iter()
            .any(|later| later.mutated_ok && later.call_hash != entry.call_hash);

        match !intervening_distinct_mutation && entry.canonical_name == target_canonical {
            false => continue,
            true => {
                let current_fingerprint = tool_arg_fingerprint(tool_args);
                let previous_fingerprint =
                    entry.tc.get("_openz_arg_fingerprint").and_then(|v| v.as_str());

                let match_args = match (
                    current_fingerprint.as_deref(),
                    previous_fingerprint.is_some() && scene_file_arg_path(tool_args).is_some(),
                ) {
                    (Some(current), _) => previous_fingerprint == Some(current),
                    (None, true) => false,
                    (None, false) => tool_args_match(&target_canonical, tool_args, entry.args_val),
                };

                match match_args {
                    false => {}
                    true => match exact_arg_repeat {
                        true => {
                            count += 1;
                        }
                        false => {
                            let is_new_progress_sig = progress_sensitive_repeat && {
                                let signature = tool_result_signature(
                                    &turn_messages[entry.msg_idx + 1..],
                                    tool_call_id(entry.tc),
                                    &entry.canonical_name,
                                );
                                signature
                                    .as_ref()
                                    .is_some_and(|sig| seen_signatures.insert(sig.clone()))
                            };
                            let _ = (!is_new_progress_sig).then(|| {
                                count += 1;
                            });
                        }
                    },
                }
            }
        }
    }
    count
}

pub(crate) fn count_previous_text_responses(messages: &[Message], next_content: &str) -> usize {
    let next_trimmed = next_content.trim();
    match next_trimmed.is_empty() {
        true => 0,
        false => {
            let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
            messages[last_user_idx..]
                .iter()
                .filter(|msg| {
                    msg.role == "assistant"
                        && !msg.content.trim().is_empty()
                        && msg.content.trim() == next_trimmed
                })
                .count()
        }
    }
}

use crate::tools::routing::contains_any;

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
                    "Research browser fallback cannot run because the native WebKit2GTK runtime library is unavailable. Call browser(action=\"status\") to inspect shared-library availability and fallback options.",
                ),
                super::research_policy::ResearchFailureKind::BrowserSessionLost => Some(
                    "Research browser fallback lost its active worker session. Call browser(action=\"status\"), relaunch the session once if the native WebKit2GTK runtime is available, and stop retrying if the failure repeats.",
                ),
                super::research_policy::ResearchFailureKind::SearchExhausted => Some(
                    "Search backends returned no usable results. Run diagnose_system for web_engine backend health, then use configured fallback policy or answer from cached/official sources with a clear source caveat.",
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
