use super::{AgentLoop, TurnContext, TurnState};
use crate::agent::style::*;
use crate::providers::GenerationSettings;
use crate::session::Message;
use anyhow::Result;
use fs2::FileExt;
use futures_util::StreamExt;
use std::io::Write;

fn should_cancel_turn_after_tool_error(error_str: &str) -> bool {
    let lower = error_str.to_lowercase();
    lower.contains("cancelled by user")
        || lower.contains("canceled by user")
        || lower.contains("subagent task cancelled")
}

fn timeout_arg(arguments: &serde_json::Value, key: &str) -> Option<u64> {
    arguments.get(key).and_then(|v| v.as_u64())
}

fn max_parallel_task_timeout(arguments: &serde_json::Value) -> Option<u64> {
    arguments
        .get("tasks")
        .and_then(|v| v.as_array())
        .and_then(|tasks| {
            tasks
                .iter()
                .filter_map(|task| timeout_arg(task, "timeout_secs"))
                .max()
        })
}

fn summarize_auto_capture_topics(
    capture_summaries: &[crate::tools::shared_memory::AutoCaptureSummary],
) -> String {
    let mut seen = std::collections::HashSet::new();
    capture_summaries
        .iter()
        .filter_map(|c| {
            let topic = c.topic.trim();
            if topic.is_empty() || !seen.insert(topic.to_string()) {
                None
            } else {
                Some(topic.to_string())
            }
        })
        .take(3)
        .collect::<Vec<_>>()
        .join(", ")
}

fn count_unique_auto_capture_brief_topics(
    capture_summaries: &[crate::tools::shared_memory::AutoCaptureSummary],
) -> usize {
    capture_summaries
        .iter()
        .filter(|c| c.brief_saved)
        .map(|c| c.topic.trim())
        .filter(|topic| !topic.is_empty())
        .collect::<std::collections::HashSet<_>>()
        .len()
}

fn is_research_lookup_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "web_search"
            | "web_fetch"
            | "crawl"
            | "crawl_site"
            | "parallel_research"
            | "searchxyz_search_web"
            | "searchxyz_read_url"
            | "searchxyz_search_and_read"
            | "searchxyz_browser_search"
            | "searchxyz_deep_research"
            | "searchxyz_site_map"
            | "searchxyz_read_github_repo"
            | "social_search"
            | "obscura_browser"
            | "firefox_browser"
            | "gsd_browser"
    )
}

fn direct_research_url(tool_name: &str, arguments: &serde_json::Value) -> Option<String> {
    if !matches!(tool_name, "web_fetch" | "searchxyz_read_url") {
        return None;
    }

    let raw_url = arguments.get("url").and_then(|value| value.as_str())?;
    let mut parsed = reqwest::Url::parse(raw_url).ok()?;
    // URL fragments select a page section but do not change the fetched document.
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

fn direct_page_research_only(user_content: &str) -> bool {
    if !super::research_policy::text_has_http_url(user_content) {
        return false;
    }

    let lower = user_content.to_lowercase();
    // Extraction and download tasks legitimately follow embeds and asset URLs.
    // Ordinary "research this URL" requests remain page-local by default.
    let allows_related_sources = [
        "deep",
        "broader",
        "more detail",
        "related",
        "compare",
        "comparison",
        "multiple sources",
        "full research",
        "scrape",
        "scrap",
        "source code",
        "download",
        "assets",
        "asset",
        "run locally",
        "local copy",
    ]
    .iter()
    .any(|term| lower.contains(term));
    !allows_related_sources
}

fn user_content_requests_fresh_fetch(user_content: &str) -> bool {
    let lower = user_content.to_lowercase();
    [
        "latest",
        "current",
        "today",
        "now",
        "check again",
        "verify",
        "refresh",
        "recheck",
        "up to date",
        "what's new",
        "whats new",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn saved_tool_output_ref_from_read_file(
    call: &crate::providers::ToolCallRequest,
) -> Option<String> {
    if call.name != "read_file" {
        return None;
    }

    let raw_path = call.arguments.get("path")?.as_str()?.trim();
    if raw_path.is_empty() {
        return None;
    }

    let path_without_scheme = raw_path.strip_prefix("file://").unwrap_or(raw_path);
    let path = std::path::Path::new(path_without_scheme);
    let outputs_dir = crate::config::loader::runtime_data_dir().join("tool_outputs");

    if !path.is_absolute() || !path.starts_with(&outputs_dir) {
        return None;
    }

    if raw_path.starts_with("file://") {
        Some(raw_path.to_string())
    } else {
        Some(format!("file://{}", path.to_string_lossy()))
    }
}

fn auto_adjust_tool_call_for_user_intent(
    mut call: crate::providers::ToolCallRequest,
    user_content: &str,
) -> crate::providers::ToolCallRequest {
    if let Some(original_ref) = saved_tool_output_ref_from_read_file(&call) {
        return crate::providers::ToolCallRequest {
            id: call.id,
            name: "retrieve_original".to_string(),
            arguments: serde_json::json!({ "ccr_id": original_ref }),
        };
    }

    if call.name == "web_fetch"
        && user_content_requests_fresh_fetch(user_content)
        && call.arguments.get("cache_mode").is_none()
        && call.arguments.get("cacheMode").is_none()
    {
        if let serde_json::Value::Object(map) = &mut call.arguments {
            map.insert(
                "cache_mode".to_string(),
                serde_json::Value::String("revalidate".to_string()),
            );
        }
    }
    call
}

fn edit_tool_target_path(tool_name: &str, arguments: &serde_json::Value) -> Option<String> {
    if !matches!(
        tool_name,
        "write_file" | "patch_file" | "replace_lines" | "zenflow_edit"
    ) {
        return None;
    }

    arguments
        .get("path")
        .or_else(|| arguments.get("file_path"))
        .or_else(|| arguments.get("target_path"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToString::to_string)
}

fn scope_context_target_for_edit_path(target_path: &str) -> String {
    let trimmed = target_path.trim();
    let path = std::path::Path::new(trimmed);
    if path.exists() {
        return trimmed.to_string();
    }

    let mut parent = path.parent();
    while let Some(candidate) = parent {
        if candidate.as_os_str().is_empty() {
            return ".".to_string();
        }
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
        parent = candidate.parent();
    }

    trimmed.to_string()
}

fn auto_scope_context_before_edit(
    call: crate::providers::ToolCallRequest,
    scoped_edit_paths: &mut std::collections::HashSet<String>,
) -> crate::providers::ToolCallRequest {
    let Some(edit_path) = edit_tool_target_path(&call.name, &call.arguments) else {
        return call;
    };
    let target_path = scope_context_target_for_edit_path(&edit_path);

    if !scoped_edit_paths.insert(target_path.clone()) {
        return call;
    }

    crate::providers::ToolCallRequest {
        id: call.id,
        name: "scope_context".to_string(),
        arguments: serde_json::json!({
            "target_path": target_path,
            "auto_reason": "before_edit"
        }),
    }
}

fn user_content_requests_artifact_open(user_content: &str) -> bool {
    let lower = user_content.to_lowercase();
    [
        "show", "open", "display", "view", "preview", "play", "launch", "see it",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_local_artifact_path(path: &str) -> bool {
    let clean = path.trim().trim_start_matches("file://");
    if clean.starts_with("http://") || clean.starts_with("https://") {
        return false;
    }
    let Some(ext) = std::path::Path::new(clean)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    else {
        return false;
    };
    matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "webp"
            | "gif"
            | "bmp"
            | "svg"
            | "mp4"
            | "mov"
            | "webm"
            | "pdf"
            | "docx"
            | "pptx"
            | "xlsx"
            | "html"
            | "htm"
    )
}

fn artifact_path_from_value(value: &serde_json::Value) -> Option<String> {
    let obj = value.as_object()?;
    for key in [
        "output_path",
        "outputPath",
        "path",
        "file_path",
        "filePath",
        "target",
        "url",
    ] {
        if let Some(path) = obj.get(key).and_then(|value| value.as_str()) {
            if is_local_artifact_path(path) {
                return Some(path.to_string());
            }
        }
    }
    None
}

fn artifact_path_from_tool_result(
    call: &crate::providers::ToolCallRequest,
    result: &serde_json::Value,
) -> Option<String> {
    if result.get("error").is_some() {
        return None;
    }
    artifact_path_from_value(result).or_else(|| artifact_path_from_value(&call.arguments))
}

fn auto_open_artifact_call_after_tool(
    user_content: &str,
    call: &crate::providers::ToolCallRequest,
    result: &serde_json::Value,
    opened_artifact_paths: &mut std::collections::HashSet<String>,
) -> Option<crate::providers::ToolCallRequest> {
    if call.name == "open_path" || !user_content_requests_artifact_open(user_content) {
        return None;
    }

    let target = artifact_path_from_tool_result(call, result)?;
    if !opened_artifact_paths.insert(target.clone()) {
        return None;
    }

    Some(crate::providers::ToolCallRequest {
        id: format!("auto_open_{}", call.id),
        name: "open_path".to_string(),
        arguments: serde_json::json!({
            "target": target,
            "auto_reason": "user_requested_artifact_display"
        }),
    })
}

fn artifact_open_category(target: &str) -> Option<&'static str> {
    let clean = target.trim().trim_start_matches("file://");
    if clean.starts_with("http://") || clean.starts_with("https://") {
        return Some("browser");
    }
    let ext = std::path::Path::new(clean)
        .extension()
        .and_then(|ext| ext.to_str())?
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "svg" => Some("image_viewer"),
        "mp4" | "mkv" | "webm" | "mov" | "avi" => Some("video_player"),
        "mp3" | "wav" | "ogg" | "flac" | "m4a" => Some("audio_player"),
        "pdf" => Some("pdf_viewer"),
        "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp" => {
            Some("office_docs")
        }
        "txt" | "md" | "rs" | "js" | "ts" | "html" | "htm" | "css" | "json" | "toml" | "yaml"
        | "yml" => Some("editor"),
        _ => Some("file_manager"),
    }
}

fn auto_device_inventory_suggest_call_after_open_failure(
    call: &crate::providers::ToolCallRequest,
    result: &serde_json::Value,
    suggested_open_targets: &mut std::collections::HashSet<String>,
) -> Option<crate::providers::ToolCallRequest> {
    if call.name != "open_path" || result.get("error").is_none() {
        return None;
    }
    let target = call
        .arguments
        .get("target")
        .and_then(|value| value.as_str())?;
    if !suggested_open_targets.insert(target.to_string()) {
        return None;
    }
    let category = artifact_open_category(target)?;

    Some(crate::providers::ToolCallRequest {
        id: format!("auto_device_inventory_{}", call.id),
        name: "device_inventory".to_string(),
        arguments: serde_json::json!({
            "action": "suggest",
            "category": category,
            "target": target,
            "limit": 5,
            "auto_reason": "open_path_failed"
        }),
    })
}

#[cfg(test)]
mod auto_tool_arg_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn web_fetch_latest_intent_injects_revalidate_cache_mode() {
        let call = crate::providers::ToolCallRequest {
            id: "call_1".to_string(),
            name: "web_fetch".to_string(),
            arguments: json!({ "url": "https://example.com" }),
        };
        let normalized = auto_adjust_tool_call_for_user_intent(
            call,
            "check the latest version from https://example.com",
        );
        assert_eq!(normalized.arguments["cache_mode"], "revalidate");
    }

    #[test]
    fn web_fetch_explicit_cache_mode_is_preserved() {
        let call = crate::providers::ToolCallRequest {
            id: "call_1".to_string(),
            name: "web_fetch".to_string(),
            arguments: json!({ "url": "https://example.com", "cache_mode": "prefer_cache" }),
        };
        let normalized = auto_adjust_tool_call_for_user_intent(call, "check latest");
        assert_eq!(normalized.arguments["cache_mode"], "prefer_cache");
    }

    #[test]
    fn read_file_for_saved_tool_output_is_rewritten_to_retrieve_original() {
        let call = crate::providers::ToolCallRequest {
            id: "call_read_output".to_string(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "file:///home/aswin/.openz/tool_outputs/output_big_tool.json" }),
        };
        let normalized = auto_adjust_tool_call_for_user_intent(call, "show the full output");
        assert_eq!(normalized.name, "retrieve_original");
        assert_eq!(normalized.id, "call_read_output");
        assert_eq!(
            normalized.arguments["ccr_id"],
            "file:///home/aswin/.openz/tool_outputs/output_big_tool.json"
        );
    }

    #[test]
    fn read_file_for_normal_project_file_is_preserved() {
        let call = crate::providers::ToolCallRequest {
            id: "call_read_file".to_string(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "src/main.rs" }),
        };
        let normalized = auto_adjust_tool_call_for_user_intent(call, "show the file");
        assert_eq!(normalized.name, "read_file");
        assert_eq!(normalized.arguments["path"], "src/main.rs");
    }

    #[test]
    fn first_edit_call_is_replaced_with_scope_context() {
        let call = crate::providers::ToolCallRequest {
            id: "call_edit".to_string(),
            name: "patch_file".to_string(),
            arguments: json!({ "path": "src/main.rs", "patch": "---" }),
        };
        let mut scoped = std::collections::HashSet::new();
        let normalized = auto_scope_context_before_edit(call, &mut scoped);
        assert_eq!(normalized.name, "scope_context");
        assert_eq!(normalized.id, "call_edit");
        assert_eq!(normalized.arguments["target_path"], "src/main.rs");
        assert!(scoped.contains("src/main.rs"));
    }

    #[test]
    fn first_write_to_new_file_scopes_existing_parent_directory() {
        let call = crate::providers::ToolCallRequest {
            id: "call_new_file".to_string(),
            name: "write_file".to_string(),
            arguments: json!({ "path": "src/__openz_auto_scope_new_file.rs", "content": "" }),
        };
        let mut scoped = std::collections::HashSet::new();
        let normalized = auto_scope_context_before_edit(call, &mut scoped);
        assert_eq!(normalized.name, "scope_context");
        assert_eq!(normalized.arguments["target_path"], "src");
        assert!(scoped.contains("src"));
    }

    #[test]
    fn second_edit_call_for_same_path_is_preserved() {
        let call = crate::providers::ToolCallRequest {
            id: "call_edit".to_string(),
            name: "patch_file".to_string(),
            arguments: json!({ "path": "src/main.rs", "patch": "---" }),
        };
        let mut scoped = std::collections::HashSet::from(["src/main.rs".to_string()]);
        let normalized = auto_scope_context_before_edit(call, &mut scoped);
        assert_eq!(normalized.name, "patch_file");
    }
    #[test]
    fn show_intent_and_generated_output_path_builds_auto_open_call() {
        let call = crate::providers::ToolCallRequest {
            id: "call_image".to_string(),
            name: "generate_image".to_string(),
            arguments: json!({ "output_path": "/tmp/demo.png" }),
        };
        let result = json!({ "status": "success", "output_path": "/tmp/demo.png" });
        let mut opened = std::collections::HashSet::new();
        let open_call = auto_open_artifact_call_after_tool(
            "make an image and show it",
            &call,
            &result,
            &mut opened,
        )
        .expect("auto open call");
        assert_eq!(open_call.name, "open_path");
        assert_eq!(open_call.id, "auto_open_call_image");
        assert_eq!(open_call.arguments["target"], "/tmp/demo.png");
    }

    #[test]
    fn generated_artifact_without_show_intent_does_not_auto_open() {
        let call = crate::providers::ToolCallRequest {
            id: "call_image".to_string(),
            name: "generate_image".to_string(),
            arguments: json!({ "output_path": "/tmp/demo.png" }),
        };
        let result = json!({ "status": "success", "output_path": "/tmp/demo.png" });
        let mut opened = std::collections::HashSet::new();
        assert!(
            auto_open_artifact_call_after_tool("make an image", &call, &result, &mut opened,)
                .is_none()
        );
    }

    #[test]
    fn auto_open_dedupes_same_artifact_path() {
        let call = crate::providers::ToolCallRequest {
            id: "call_video".to_string(),
            name: "html_to_video".to_string(),
            arguments: json!({ "output_path": "/tmp/demo.mp4" }),
        };
        let result = json!({ "status": "success", "output_path": "/tmp/demo.mp4" });
        let mut opened = std::collections::HashSet::new();
        assert!(
            auto_open_artifact_call_after_tool("show the video", &call, &result, &mut opened)
                .is_some()
        );
        assert!(
            auto_open_artifact_call_after_tool("show the video", &call, &result, &mut opened)
                .is_none()
        );
    }

    #[test]
    fn failed_open_path_builds_device_inventory_suggestion_call() {
        let call = crate::providers::ToolCallRequest {
            id: "call_open".to_string(),
            name: "open_path".to_string(),
            arguments: json!({ "target": "/tmp/demo.pdf" }),
        };
        let result = json!({ "error": "Failed to open '/tmp/demo.pdf': no application found" });
        let mut suggested = std::collections::HashSet::new();

        let suggest_call =
            auto_device_inventory_suggest_call_after_open_failure(&call, &result, &mut suggested)
                .expect("device inventory suggestion call");

        assert_eq!(suggest_call.name, "device_inventory");
        assert_eq!(suggest_call.id, "auto_device_inventory_call_open");
        assert_eq!(suggest_call.arguments["action"], "suggest");
        assert_eq!(suggest_call.arguments["category"], "pdf_viewer");
        assert_eq!(suggest_call.arguments["target"], "/tmp/demo.pdf");
    }

    #[test]
    fn failed_open_path_suggestion_dedupes_same_target() {
        let call = crate::providers::ToolCallRequest {
            id: "call_open".to_string(),
            name: "open_path".to_string(),
            arguments: json!({ "target": "/tmp/demo.png" }),
        };
        let result = json!({ "error": "Failed to open '/tmp/demo.png': no application found" });
        let mut suggested = std::collections::HashSet::new();

        assert!(auto_device_inventory_suggest_call_after_open_failure(
            &call,
            &result,
            &mut suggested
        )
        .is_some());
        assert!(auto_device_inventory_suggest_call_after_open_failure(
            &call,
            &result,
            &mut suggested
        )
        .is_none());
    }

    #[test]
    fn browser_backed_readers_count_as_research_lookup_tools() {
        assert!(is_research_lookup_tool("obscura_browser"));
        assert!(is_research_lookup_tool("firefox_browser"));
        assert!(is_research_lookup_tool("gsd_browser"));
        assert!(is_research_lookup_tool("searchxyz_browser_search"));
    }
}

async fn fresh_research_brief_blocks_lookup(
    user_content: &str,
    tool_name: &str,
    arguments: &serde_json::Value,
) -> bool {
    if !is_research_lookup_tool(tool_name)
        || super::research_policy::should_force_live_research_lookup(user_content, arguments)
    {
        return false;
    }
    match crate::tools::shared_memory::search_research_briefs(user_content, 1).await {
        Ok(items) => items.first().is_some_and(|item| {
            item.score >= super::build::MIN_MATCH_SCORE && item.freshness == "fresh"
        }),
        Err(err) => {
            tracing::debug!(error = ?err, "fresh research brief lookup gate skipped");
            false
        }
    }
}

fn resolve_tool_timeout_secs(
    tool_name: &str,
    arguments: &serde_json::Value,
    recommended_timeout_secs: Option<u64>,
    default_timeout_secs: u64,
) -> u64 {
    if let Some(explicit) = timeout_arg(arguments, "_timeout_secs") {
        return crate::tools::clamp_tool_timeout_secs(explicit);
    }

    let mut timeout_secs = recommended_timeout_secs.unwrap_or(default_timeout_secs);
    if let Some(argument_timeout) = timeout_arg(arguments, "timeout_secs") {
        timeout_secs = timeout_secs.max(argument_timeout);
    }
    if tool_name == "parallel_research" {
        if let Some(task_timeout) = max_parallel_task_timeout(arguments) {
            timeout_secs = timeout_secs.max(task_timeout);
        }
    }

    crate::tools::clamp_tool_timeout_secs(timeout_secs)
}

struct ApprovedToolExec<'a> {
    tool: std::sync::Arc<dyn crate::tools::Tool>,
    call: &'a crate::providers::ToolCallRequest,
    metadata: &'a crate::tools::ToolMetadata,
    config: &'a crate::config::schema::Config,
    formatted_args: &'a str,
    session_key: &'a str,
    silent: bool,
    tool_spinner_msg: &'a str,
    turn_cancel: &'a crate::tools::subagent::CancellationToken,
    turn_errors: &'a mut Vec<String>,
}

struct ToolExecutionPipeline<'a> {
    params: ApprovedToolExec<'a>,
}

impl<'a> ToolExecutionPipeline<'a> {
    fn new(params: ApprovedToolExec<'a>) -> Self {
        Self { params }
    }

    async fn execute(&mut self) -> serde_json::Value {
        let _process_guard = match self.acquire_process_guard() {
            Ok(guard) => guard,
            Err(reason) => return self.render_process_policy_block(&reason).await,
        };

        let tool_timeout_secs = self.resolve_timeout_secs();
        let mut attempts = 0;
        let max_attempts = 3;
        let mut delay = std::time::Duration::from_secs(1);

        loop {
            attempts += 1;
            match self.run_with_spinner(tool_timeout_secs).await {
                Ok(res) => return self.render_success(res).await,
                Err(err) => {
                    if attempts < max_attempts && is_transient_error(&err) {
                        tracing::warn!(
                            tool = %self.params.call.name,
                            attempt = attempts,
                            error = %err,
                            "Transient tool error encountered. Retrying in {:?}",
                            delay
                        );
                        tokio::time::sleep(delay).await;
                        delay *= 2;
                    } else {
                        return self.render_error(err).await;
                    }
                }
            }
        }
    }

    fn acquire_process_guard(
        &self,
    ) -> Result<Option<crate::tools::resource_policy::ProcessToolGuard>, String> {
        if !self.params.metadata.spawns_process {
            return Ok(None);
        }

        crate::tools::resource_policy::try_acquire_process_tool(
            self.params
                .config
                .agents
                .defaults
                .max_concurrent_process_tools,
        )
        .map(Some)
    }

    fn resolve_timeout_secs(&self) -> u64 {
        let normalized = crate::tools::normalize_tool_args(&self.params.call.arguments);
        resolve_tool_timeout_secs(
            &self.params.call.name,
            &normalized,
            self.params.metadata.recommended_timeout_secs,
            self.params.config.agents.defaults.tool_timeout_secs,
        )
    }

    async fn run_with_spinner(&self, timeout_secs: u64) -> anyhow::Result<serde_json::Value> {
        let tool_timeout = std::time::Duration::from_secs(timeout_secs);
        let normalized = crate::tools::normalize_tool_args(&self.params.call.arguments);
        let fut = self.params.tool.call(&normalized);
        let timed_fut = tokio::time::timeout(tool_timeout, fut);
        let tool_cancel_tx = crate::shutdown::cli_cancel_tx();
        let mut tool_cancel_rx = tool_cancel_tx.subscribe();
        let tool_cancel_initial = *tool_cancel_rx.borrow();

        let cancel_aware_fut = async {
            tokio::select! {
                biased;
                _ = async {
                    while *tool_cancel_rx.borrow() == tool_cancel_initial {
                        if tool_cancel_rx.changed().await.is_err() { break; }
                    }
                } => {
                    Err(anyhow::anyhow!("Cancelled by user"))
                }
                res = timed_fut => {
                    match res {
                        Ok(r) => r,
                        Err(_) => Err(anyhow::anyhow!(
                            "Tool execution timed out after {}s",
                            timeout_secs
                        )),
                    }
                }
            }
        };

        with_spinner(self.params.tool_spinner_msg, cancel_aware_fut).await
    }

    async fn render_process_policy_block(&mut self, reason: &str) -> serde_json::Value {
        let error_str = format!(
            "Tool blocked by resource policy: {}. {}",
            self.params.call.name, reason
        );
        self.params.turn_errors.push(format!(
            "Tool {} blocked by process resource policy: {}",
            self.params.call.name, reason
        ));
        self.render_failure(&error_str).await
    }

    async fn render_success(&self, res: serde_json::Value) -> serde_json::Value {
        super::tool_execution::render_tool_success(
            self.params.call,
            self.params.formatted_args,
            self.params.session_key,
            self.params.silent,
            res,
        )
        .await
    }

    async fn render_error(&mut self, err: anyhow::Error) -> serde_json::Value {
        let error_str = err.to_string();
        if should_cancel_turn_after_tool_error(&error_str) {
            self.params.turn_cancel.cancel();
        }
        self.params.turn_errors.push(format!(
            "Tool {} failed: {}",
            self.params.call.name, error_str
        ));
        self.render_failure(&error_str).await
    }

    async fn render_failure(&self, error_str: &str) -> serde_json::Value {
        super::tool_execution::render_tool_failure(
            self.params.call,
            self.params.formatted_args,
            self.params.session_key,
            self.params.silent,
            error_str,
        )
        .await
    }
}

fn is_transient_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    if msg.contains("cancelled by user") || msg.contains("tool execution timed out") {
        return false;
    }
    msg.contains("rate limit")
        || msg.contains("429")
        || msg.contains("too many requests")
        || msg.contains("timeout")
        || msg.contains("timed out")
        || msg.contains("connection")
        || msg.contains("connect")
        || msg.contains("network")
        || msg.contains("dns")
        || msg.contains("host unreachable")
        || msg.contains("temporary failure")
        || msg.contains("502")
        || msg.contains("503")
        || msg.contains("504")
        || msg.contains("bad gateway")
        || msg.contains("service unavailable")
}

async fn execute_approved_tool(params: ApprovedToolExec<'_>) -> serde_json::Value {
    ToolExecutionPipeline::new(params).execute().await
}

fn provider_turn_lock_key(provider: &str, model: &str) -> Option<String> {
    let mode = std::env::var("OPENZ_PROVIDER_TURN_LOCK").ok();
    provider_turn_lock_key_for_mode(provider, model, mode.as_deref())
}

fn provider_turn_lock_key_for_mode(
    provider: &str,
    model: &str,
    mode: Option<&str>,
) -> Option<String> {
    let Some(mode) = mode.map(str::trim).filter(|value| !value.is_empty()) else {
        return None;
    };
    let mode = mode.to_lowercase();
    if mode == "off" || mode == "false" || mode == "0" {
        return None;
    }

    let provider_lower = provider.to_lowercase();
    let model_lower = model.to_lowercase();
    let fragile = provider_lower == "opencode_zen"
        || provider_lower == "opencode-zen"
        || model_lower.contains(":free")
        || model_lower.ends_with("-free")
        || model_lower.contains("flash-free");
    let should_lock = mode == "all"
        || ((mode == "fragile" || mode == "free" || mode == "on" || mode == "true") && fragile);
    if !should_lock {
        return None;
    }

    let raw = format!("{}__{}", provider_lower, model_lower);
    let slug = raw
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    Some(if slug.is_empty() {
        "provider".to_string()
    } else {
        slug
    })
}

async fn acquire_provider_turn_lock(
    session_key: &str,
    provider: &str,
    model: &str,
) -> Result<Option<std::fs::File>> {
    let Some(key) = provider_turn_lock_key(provider, model) else {
        return Ok(None);
    };
    let dir = crate::config::loader::runtime_data_dir().join("provider_turn_locks");
    let path = dir.join(format!("{key}.lock"));
    let session_key = session_key.to_string();
    let provider = provider.to_string();
    let model = model.to_string();
    tokio::task::spawn_blocking(move || -> Result<Option<std::fs::File>> {
        std::fs::create_dir_all(&dir)?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;
        let mut delay = std::time::Duration::from_millis(100);
        let started = std::time::Instant::now();
        let mut announced_wait = false;
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Some(file)),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    if !announced_wait {
                        crate::agent::activity::update_activity(
                            &session_key,
                            "Waiting for provider slot",
                            Some(&format!("{} / {}", provider, model)),
                        );
                        announced_wait = true;
                    }
                    if started.elapsed() > std::time::Duration::from_secs(120) {
                        anyhow::bail!(
                            "Provider '{}' model '{}' is still busy in another OpenZ session after 120s",
                            provider,
                            model
                        );
                    }
                    std::thread::sleep(delay);
                    delay = std::cmp::min(
                        delay.saturating_mul(2),
                        std::time::Duration::from_secs(1),
                    );
                }
                Err(err) => return Err(err.into()),
            }
        }
    })
    .await?
}

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    if let Some(question) =
        crate::agent::marketplace_intent::clarification_question_for_marketplace_intent(
            ctx.user_content,
        )
    {
        ctx.final_content = question.to_string();
        ctx.messages.push(Message {
            role: "assistant".to_string(),
            content: question.to_string(),
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            extra: serde_json::Map::new(),
        });
        return Ok(TurnState::Save);
    }

    let mut iterations = 0;
    let mut loop_blocked_count = 0;
    let max_iterations = ctx.config.agents.defaults.max_tool_iterations;
    let mut turn_capture_summaries: Vec<crate::tools::shared_memory::AutoCaptureSummary> =
        Vec::new();
    let mut turn_source_ledger = crate::agent::source_ledger::SourceLedger::default();
    let mut completed_direct_research_urls = std::collections::HashSet::new();
    let mut auto_scoped_edit_paths = std::collections::HashSet::new();
    let mut auto_opened_artifact_paths = std::collections::HashSet::new();
    let mut auto_suggested_open_targets = std::collections::HashSet::new();
    let direct_page_only = direct_page_research_only(ctx.user_content);
    let mut direct_page_fetched = false;
    let _provider_turn_lock = acquire_provider_turn_lock(
        ctx.session_key,
        &ctx.config.agents.defaults.provider,
        &ctx.config.agents.defaults.model,
    )
    .await?;

    // Build a turn-level cancellation token from the current CLI context.
    // This provides early cancellation detection even before the CLI select! drops run_fut.
    let turn_cancel = crate::tools::subagent::CancellationToken::new();
    let turn_cancel_clone = turn_cancel.clone();

    loop {
        // Check for turn-level cancellation at the start of each iteration.
        // Without this, a subagent cancellation error is fed back to the LLM
        // which may continue iterating instead of stopping.
        if turn_cancel.is_cancelled() {
            let msg = "Turn cancelled by user.".to_string();
            ctx.final_content = msg.clone();
            super::tool_execution::send_progress_update(ctx.session_key, &msg).await;
            if !crate::agent::style::spinner::is_silent() {
                crate::tui_println!("{}▲ {}{}", AURA_GOLD, msg, COLOR_RESET);
            }
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: msg,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
            break;
        }
        let config = ctx.config.clone();
        let settings = GenerationSettings {
            temperature: config.agents.defaults.temperature,
            max_tokens: config.agents.defaults.max_tokens,
            reasoning_effort: None,
        };

        tracing::info!(
            session = %ctx.session_key,
            iteration = iterations,
            "Sending completion request to LLM (model: {})",
            config.agents.defaults.model
        );
        if iterations >= max_iterations {
            let msg = format!(
                "⚠️ Reached tool iteration limit ({}). Summarizing work so far.",
                max_iterations
            );
            ctx.final_content = msg.clone();
            super::tool_execution::send_progress_update(ctx.session_key, &msg).await;
            if !crate::agent::style::spinner::is_silent() {
                crate::tui_println!("{}⚠️ {}{}", AURA_GOLD, msg, COLOR_RESET);
            }
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: msg,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
            break;
        }

        let tools_openai = loop_ref.tools.to_openai_format_for_prompt(ctx.user_content);
        if config.agents.defaults.show_tool_router_status
            && !crate::agent::style::spinner::is_silent()
        {
            let summary = loop_ref.tools.tool_router_status_line(ctx.user_content);
            crate::tui_println!("{}◎ {}{}", AURA_PURPLE, summary, COLOR_RESET);
        }

        let activity_msg = format!("{}▶ Thinking{}", RED_ORANGE, COLOR_RESET);
        let start_time = std::time::Instant::now();
        // Track if content was already streamed to terminal (to avoid duplicate display)
        let mut content_streaming_started = false;
        let mut reasoning_printed = false;
        let mut tool_call_stream_started = false;
        let mut current_line_buffer = String::new();
        let mut resp = if config.agents.defaults.streaming {
            let mut stream = loop_ref
                .chat_stream_with_fallback(
                    &mut ctx.active_provider,
                    &ctx.system_prompt,
                    &ctx.messages,
                    &tools_openai,
                    &settings,
                    &activity_msg,
                )
                .await?;
            let silent = crate::agent::style::spinner::is_silent();
            let tui_thought_display =
                normalize_tui_thought_display(&config.agents.defaults.tui_thought_display);

            let mut full_content = String::new();
            let mut full_reasoning = String::new();
            // Track whether we're currently in reasoning phase (for live spinner)
            let mut in_reasoning_phase = false;

            let print_reasoning = |full_reasoning: &str,
                                   in_reasoning_phase: &mut bool,
                                   reasoning_printed: &mut bool,
                                   start_time: std::time::Instant,
                                   display_mode: &str| {
                if !*reasoning_printed
                    && !full_reasoning.is_empty()
                    && should_show_tui_thoughts(display_mode)
                {
                    let depth = crate::tools::subagent::DELEGATION_DEPTH
                        .try_with(|d| *d)
                        .unwrap_or(0);
                    if !silent {
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let prefix = if depth > 0 {
                            crate::agent::style::get_tree_prefix(false)
                        } else {
                            "".to_string()
                        };
                        crate::tui_println!(
                            "{}{}● {}{}{}Thought for {:.1}s{}",
                            prefix,
                            RED_ORANGE,
                            COLOR_RESET,
                            COLOR_BOLD,
                            RED_ORANGE,
                            elapsed,
                            COLOR_RESET
                        );
                        let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                        let visible_reasoning = if display_mode == "compact" {
                            compact_reasoning_summary(full_reasoning)
                        } else {
                            full_reasoning.to_string()
                        };
                        crate::agent::style::print_tree_monologue(&leaf_prefix, &visible_reasoning);
                        crate::tui_println!("");
                    }
                    *reasoning_printed = true;
                    *in_reasoning_phase = false;
                }
            };

            let mut streaming_assembly = super::streaming::StreamingAssembly::new();

            // Create a cancel receiver for the streaming loop.
            // watch::Receiver::changed() is cancel-safe, unlike Notify.
            let stream_cancel_tx = crate::shutdown::cli_cancel_tx();
            let mut stream_cancel_rx = stream_cancel_tx.subscribe();
            let stream_cancel_initial = *stream_cancel_rx.borrow();
            let stream_idle_timeout = loop_ref.provider_attempt_timeout_duration();

            loop {
                // Race: next stream chunk vs cancellation signal vs an idle provider stream.
                let chunk = tokio::select! {
                    biased;
                    _ = async {
                        while *stream_cancel_rx.borrow() == stream_cancel_initial {
                            if stream_cancel_rx.changed().await.is_err() { break; }
                        }
                    } => {
                        return Err(anyhow::anyhow!("Cancelled by user"));
                    }
                    next = tokio::time::timeout(stream_idle_timeout, stream.next()) => {
                        match next {
                            Ok(Some(c)) => c,
                            Ok(None) => break,
                            Err(_) => {
                                return Err(anyhow::anyhow!(
                                    "Provider stream timed out after {}s without output",
                                    stream_idle_timeout.as_secs()
                                ));
                            }
                        }
                    }
                };
                match chunk? {
                    crate::providers::ChatStreamChunk::Content(text) => {
                        // Preserve old Thought-with-seconds output when enabled, before
                        // streaming the final answer.
                        print_reasoning(
                            &full_reasoning,
                            &mut in_reasoning_phase,
                            &mut reasoning_printed,
                            start_time,
                            tui_thought_display,
                        );

                        if in_reasoning_phase && !silent {
                            print!("\r\x1b[2K");
                            let _ = std::io::stdout().flush();
                            in_reasoning_phase = false;
                        }
                        full_content.push_str(&text);
                        for c in text.chars() {
                            if c == '\r' {
                                continue;
                            }
                            if c == '\n' {
                                if !silent {
                                    content_streaming_started = true;
                                    print!("\r\x1b[2K");
                                    print!("{}", format_markdown_line(&current_line_buffer));
                                    print!("\r\n");
                                    let _ = std::io::stdout().flush();
                                }
                                current_line_buffer.clear();
                            } else {
                                current_line_buffer.push(c);
                                if !silent {
                                    content_streaming_started = true;
                                    print!("{}", c);
                                    let _ = std::io::stdout().flush();
                                }
                            }
                        }
                        super::tool_execution::send_progress_update(ctx.session_key, &text).await;
                        if let Some(chat_id) =
                            crate::channels::websocket::ws_chat_id(ctx.session_key)
                        {
                            crate::channels::websocket::publish_ws_event(serde_json::json!({
                                "event": "delta",
                                "chat_id": chat_id,
                                "content": text,
                            }));
                        }
                        streaming_assembly
                            .push_chunk(crate::providers::ChatStreamChunk::Content(text));
                    }
                    crate::providers::ChatStreamChunk::Reasoning(text) => {
                        full_reasoning.push_str(&text);
                        if let Some(chat_id) =
                            crate::channels::websocket::ws_chat_id(ctx.session_key)
                        {
                            crate::channels::websocket::publish_ws_event(serde_json::json!({
                                "event": "reasoning_delta",
                                "chat_id": chat_id,
                                "content": text,
                            }));
                        }
                        in_reasoning_phase = true;
                        // Show old live thinking indicator only when TUI thoughts are enabled.
                        if !silent && should_show_tui_thoughts(tui_thought_display) {
                            let elapsed = start_time.elapsed().as_secs_f32();
                            print!(
                                "\r\x1b[2K{}{}▶ Thinking... {:.1}s{}",
                                COLOR_BOLD, RED_ORANGE, elapsed, COLOR_RESET
                            );
                            let _ = std::io::stdout().flush();
                        }
                        streaming_assembly
                            .push_chunk(crate::providers::ChatStreamChunk::Reasoning(text));
                    }
                    crate::providers::ChatStreamChunk::ToolCall {
                        index,
                        id,
                        name,
                        arguments,
                    } => {
                        tool_call_stream_started = true;
                        // If we have reasoning content that hasn't been printed yet, print it now
                        print_reasoning(
                            &full_reasoning,
                            &mut in_reasoning_phase,
                            &mut reasoning_printed,
                            start_time,
                            tui_thought_display,
                        );

                        // Also clear thinking spinner if active
                        if in_reasoning_phase && !silent {
                            print!("\r\x1b[2K");
                            let _ = std::io::stdout().flush();
                            in_reasoning_phase = false;
                        }

                        streaming_assembly.push_chunk(
                            crate::providers::ChatStreamChunk::ToolCall {
                                index,
                                id,
                                name,
                                arguments,
                            },
                        );
                    }
                    crate::providers::ChatStreamChunk::Done {
                        finish_reason: reason,
                    } => {
                        streaming_assembly.push_chunk(crate::providers::ChatStreamChunk::Done {
                            finish_reason: reason,
                        });
                    }
                }
            }

            if in_reasoning_phase && !silent {
                print!("\r\x1b[2K");
                let _ = std::io::stdout().flush();
                in_reasoning_phase = false;
            }

            // Print reasoning only when a visible answer or tool call followed it. If the
            // model returns reasoning-only, keep it for fallback final content instead of
            // ending the turn with a Thought block and no answer.
            if should_show_tui_thoughts(tui_thought_display)
                && (!full_content.trim().is_empty() || tool_call_stream_started)
            {
                print_reasoning(
                    &full_reasoning,
                    &mut in_reasoning_phase,
                    &mut reasoning_printed,
                    start_time,
                    tui_thought_display,
                );
            }

            // Print the final line in the buffer if any
            if !current_line_buffer.is_empty() && !silent {
                print!("\r\x1b[2K");
                print!("{}", format_markdown_line(&current_line_buffer));
                let _ = std::io::stdout().flush();
            }

            ctx.streamed = true;

            let mut assembled = streaming_assembly.into_response();
            assembled.content = if full_content.is_empty() {
                None
            } else {
                Some(full_content)
            };
            assembled.reasoning_content = if full_reasoning.is_empty() {
                None
            } else {
                Some(full_reasoning)
            };
            assembled
        } else {
            // Race non-streaming LLM call against cancel signal
            let ns_cancel_tx = crate::shutdown::cli_cancel_tx();
            let mut ns_cancel_rx = ns_cancel_tx.subscribe();
            let ns_cancel_initial = *ns_cancel_rx.borrow();
            let chat_fut = loop_ref.chat_with_fallback(
                &mut ctx.active_provider,
                &ctx.system_prompt,
                &ctx.messages,
                &tools_openai,
                &settings,
                &activity_msg,
            );
            tokio::select! {
                biased;
                _ = async {
                    while *ns_cancel_rx.borrow() == ns_cancel_initial {
                        if ns_cancel_rx.changed().await.is_err() { break; }
                    }
                } => {
                    turn_cancel_clone.cancel();
                    let msg = "LLM request cancelled by user.".to_string();
                    ctx.final_content = msg.clone();
                    return Ok(TurnState::Save);
                }
                res = chat_fut => res?,
            }
        };

        // Handle potential response truncation (finish_reason = "length") by auto-continuing
        if resp.finish_reason == "length" {
            let mut accumulated_content = resp.content.clone();
            let mut finish_reason = resp.finish_reason.clone();
            let mut continue_attempts = 0;

            while finish_reason == "length" && continue_attempts < 3 {
                // Check for cancellation before each continuation attempt
                if turn_cancel.is_cancelled() {
                    break;
                }
                continue_attempts += 1;

                let mut temp_messages = ctx.messages.clone();
                if let Some(ref current_acc) = accumulated_content {
                    temp_messages.push(Message {
                        role: "assistant".to_string(),
                        content: current_acc.clone(),
                        timestamp: Some(chrono::Utc::now().to_rfc3339()),
                        extra: serde_json::Map::new(),
                    });
                }

                temp_messages.push(Message {
                    role: "user".to_string(),
                    content: "Continue generating the rest of your previous message exactly from where you left off. Do not repeat the beginning.".to_string(),
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra: serde_json::Map::new(),
                });

                let cont_activity_msg = format!(
                    "{}▶ Continuing response... (attempt {}){}",
                    RED_ORANGE, continue_attempts, COLOR_RESET
                );
                // Pass &[] instead of tools_openai so the model does not get confused and attempt to generate tool calls during text continuation
                if let Ok(cont_resp) = loop_ref
                    .chat_with_fallback(
                        &mut ctx.active_provider,
                        &ctx.system_prompt,
                        &temp_messages,
                        &[],
                        &settings,
                        &cont_activity_msg,
                    )
                    .await
                {
                    finish_reason = cont_resp.finish_reason.clone();
                    if let Some(ref cont_content) = cont_resp.content {
                        if let Some(ref mut acc) = accumulated_content {
                            acc.push_str(cont_content);
                        } else {
                            accumulated_content = Some(cont_content.clone());
                        }
                    }
                    if !cont_resp.tool_calls.is_empty() {
                        resp.tool_calls.extend(cont_resp.tool_calls);
                    }
                } else {
                    break;
                }
            }

            resp.content = accumulated_content;
            resp.finish_reason = finish_reason;
        }

        if let Some(text) = resp.content.take() {
            let (clean_content, extracted_reasoning) =
                crate::providers::openai::split_think_blocks(&text);
            resp.content = clean_content;
            resp.reasoning_content = crate::providers::openai::merge_reasoning(
                resp.reasoning_content.take(),
                extracted_reasoning,
            );
        }

        // If tool_calls is empty, try to parse tool calls from cleaned content.
        if resp.tool_calls.is_empty() {
            if let Some(ref text) = resp.content {
                let parsed = crate::providers::openai::parse_fallback_tool_calls(text);
                if !parsed.is_empty() {
                    resp.tool_calls = parsed;
                    resp.content = None;
                }
            }
        }

        // Handle models that send everything as reasoning_content with no content.
        // Common with DeepSeek-V4-style streams. Recover a user-facing answer
        // instead of dumping reasoning as plain text or ending after Thought.
        if resp.content.is_none() && resp.reasoning_content.is_some() && resp.tool_calls.is_empty()
        {
            if !crate::agent::style::spinner::is_silent() {
                print!("\r\x1b[2K");
                let _ = std::io::stdout().flush();
            }

            if config.agents.defaults.streaming {
                let original_reasoning = resp.reasoning_content.take();
                let mut recovery_messages = ctx.messages.clone();
                let recovery_prompt = match original_reasoning.as_deref() {
                    Some(reasoning) if !reasoning.trim().is_empty() => format!(
                        "Your previous streamed response contained reasoning only and no user-facing answer. Here is that reasoning:

{}

Now provide only the final user-facing answer to my last message. Do not include analysis, reasoning labels, or tool calls.",
                        reasoning.trim()
                    ),
                    _ => "Your previous streamed response contained reasoning only and no user-facing answer. Provide only the final answer to my last message now. Do not include reasoning, analysis, or tool calls.".to_string(),
                };
                recovery_messages.push(Message {
                    role: "user".to_string(),
                    content: recovery_prompt,
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra: serde_json::Map::new(),
                });
                let recovery_activity_msg = String::new();

                match loop_ref
                    .chat_stream_with_fallback(
                        &mut ctx.active_provider,
                        &ctx.system_prompt,
                        &recovery_messages,
                        &[],
                        &settings,
                        &recovery_activity_msg,
                    )
                    .await
                {
                    Ok(mut recovery_stream) => {
                        let mode = normalize_tui_thought_display(
                            &config.agents.defaults.tui_thought_display,
                        );
                        let mut recovery_content = String::new();
                        let mut recovery_reasoning = String::new();
                        let mut recovery_finish_reason = "stop".to_string();

                        let recovery_stream_idle_timeout =
                            loop_ref.provider_attempt_timeout_duration();
                        loop {
                            let Some(chunk) = tokio::time::timeout(
                                recovery_stream_idle_timeout,
                                recovery_stream.next(),
                            )
                            .await
                            .map_err(|_| {
                                anyhow::anyhow!(
                                    "Provider recovery stream timed out after {}s without output",
                                    recovery_stream_idle_timeout.as_secs()
                                )
                            })?
                            else {
                                break;
                            };
                            match chunk? {
                                crate::providers::ChatStreamChunk::Content(text) => {
                                    if !reasoning_printed
                                        && should_show_tui_thoughts(mode)
                                        && !crate::agent::style::spinner::is_silent()
                                    {
                                        let duration_secs = start_time.elapsed().as_secs_f32();
                                        let depth = crate::tools::subagent::DELEGATION_DEPTH
                                            .try_with(|d| *d)
                                            .unwrap_or(0);
                                        let prefix = if depth > 0 {
                                            crate::agent::style::get_tree_prefix(false)
                                        } else {
                                            String::new()
                                        };
                                        crate::tui_println!(
                                            "{}{}● {}{}{}Thought for {:.1}s{}",
                                            prefix,
                                            RED_ORANGE,
                                            COLOR_RESET,
                                            COLOR_BOLD,
                                            RED_ORANGE,
                                            duration_secs,
                                            COLOR_RESET
                                        );
                                        if let Some(ref reasoning) = original_reasoning {
                                            let visible_reasoning = if mode == "compact" {
                                                compact_reasoning_summary(reasoning)
                                            } else {
                                                reasoning.clone()
                                            };
                                            let leaf_prefix =
                                                crate::agent::style::get_tree_prefix(true);
                                            crate::agent::style::print_tree_monologue(
                                                &leaf_prefix,
                                                &visible_reasoning,
                                            );
                                            crate::tui_println!("");
                                        }
                                        reasoning_printed = true;
                                    }

                                    recovery_content.push_str(&text);
                                    for c in text.chars() {
                                        if c == '\r' {
                                            continue;
                                        }
                                        if c == '\n' {
                                            if !crate::agent::style::spinner::is_silent() {
                                                content_streaming_started = true;
                                                print!("\r\x1b[2K");
                                                print!(
                                                    "{}",
                                                    format_markdown_line(&current_line_buffer)
                                                );
                                                print!("\r\n");
                                                let _ = std::io::stdout().flush();
                                            }
                                            current_line_buffer.clear();
                                        } else {
                                            current_line_buffer.push(c);
                                            if !crate::agent::style::spinner::is_silent() {
                                                content_streaming_started = true;
                                                print!("{}", c);
                                                let _ = std::io::stdout().flush();
                                            }
                                        }
                                    }
                                    super::tool_execution::send_progress_update(
                                        ctx.session_key,
                                        &text,
                                    )
                                    .await;
                                }
                                crate::providers::ChatStreamChunk::Reasoning(text) => {
                                    recovery_reasoning.push_str(&text);
                                }
                                crate::providers::ChatStreamChunk::ToolCall { .. } => {}
                                crate::providers::ChatStreamChunk::Done { finish_reason } => {
                                    if let Some(reason) = finish_reason {
                                        recovery_finish_reason = reason;
                                    }
                                }
                            }
                        }

                        if !current_line_buffer.is_empty()
                            && !crate::agent::style::spinner::is_silent()
                        {
                            print!("\r\x1b[2K");
                            print!("{}", format_markdown_line(&current_line_buffer));
                            let _ = std::io::stdout().flush();
                        }

                        let recovery_reasoning_visible = recovery_reasoning.trim().to_string();
                        let recovered_content = if !recovery_content.trim().is_empty() {
                            Some(recovery_content)
                        } else if !recovery_reasoning_visible.is_empty() {
                            Some(recovery_reasoning_visible)
                        } else {
                            Some(
                                "I did not receive a final answer from the model for this turn."
                                    .to_string(),
                            )
                        };
                        let recovery_reasoning_for_memory = if recovery_reasoning.trim().is_empty()
                            || recovered_content
                                .as_ref()
                                .is_some_and(|content| content == recovery_reasoning.trim())
                        {
                            None
                        } else {
                            Some(recovery_reasoning)
                        };
                        resp.content = recovered_content;
                        resp.reasoning_content = crate::providers::openai::merge_reasoning(
                            original_reasoning,
                            recovery_reasoning_for_memory,
                        );
                        resp.finish_reason = recovery_finish_reason;
                        resp.tool_calls.clear();
                        ctx.streamed = content_streaming_started;
                    }
                    Err(e) => {
                        tracing::warn!(
                            session = %ctx.session_key,
                            error = %e,
                            "Failed to recover final answer from reasoning-only stream"
                        );
                        resp.content = Some(
                            original_reasoning
                                .as_deref()
                                .filter(|reasoning| !reasoning.trim().is_empty())
                                .unwrap_or("I did not receive a final answer from the model for this turn.")
                                .to_string(),
                        );
                        resp.reasoning_content = None;
                        ctx.streamed = false;
                        content_streaming_started = false;
                    }
                }
            } else {
                resp.content = resp.reasoning_content.take();
                ctx.streamed = false;
            }
        }

        let duration = start_time.elapsed();
        tracing::info!(
            session = %ctx.session_key,
            duration_ms = duration.as_millis(),
            has_content = resp.content.is_some(),
            has_reasoning = resp.reasoning_content.is_some(),
            tool_calls = resp.tool_calls.len(),
            "Received LLM response (finish_reason: {})",
            resp.finish_reason
        );
        if let Some(ref reasoning) = resp.reasoning_content {
            tracing::debug!(session = %ctx.session_key, "LLM reasoning content: {:?}", reasoning);
        }
        if let Some(ref content) = resp.content {
            tracing::debug!(session = %ctx.session_key, "LLM text content: {:?}", content);
        }

        let duration_secs = duration.as_secs_f32();
        let has_reasoning = resp
            .reasoning_content
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let has_content = resp
            .content
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let has_tool_calls = !resp.tool_calls.is_empty();

        if has_reasoning || (has_content && has_tool_calls) {
            let output_visibility = crate::agent::events::OutputVisibility::from_tui_trace_mode(
                &config.agents.defaults.tui_thought_display,
            );
            let public_reasoning_progress =
                should_send_public_reasoning_progress(&config.agents.defaults.tui_thought_display);
            // Send reasoning/thought summary to non-CLI channels only when explicitly enabled.
            if public_reasoning_progress && has_reasoning {
                if let Some(ref reasoning) = resp.reasoning_content {
                    if let Some(reasoning_msg) =
                        crate::agent::events::AgentEvent::PrivateReasoning(reasoning.clone())
                            .public_text(&output_visibility)
                    {
                        super::tool_execution::send_progress_update(
                            ctx.session_key,
                            &reasoning_msg,
                        )
                        .await;
                    }
                }
            } else if public_reasoning_progress && has_content && has_tool_calls {
                if let Some(ref content) = resp.content {
                    if let Some(thought_msg) =
                        crate::agent::events::AgentEvent::PrivateReasoning(content.clone())
                            .public_text(&output_visibility)
                    {
                        super::tool_execution::send_progress_update(ctx.session_key, &thought_msg)
                            .await;
                    }
                }
            }

            let silent = crate::agent::style::spinner::is_silent();
            let depth = crate::tools::subagent::DELEGATION_DEPTH
                .try_with(|d| *d)
                .unwrap_or(0);
            if !silent && should_show_tui_thoughts(&config.agents.defaults.tui_thought_display) {
                let prefix = if depth > 0 {
                    crate::agent::style::get_tree_prefix(false)
                } else {
                    "".to_string()
                };
                if ctx.streamed {
                    // During streaming, the reasoning spinner was already shown and
                    // the "Thought for Xs" badge was already printed when content
                    // started arriving or when the stream finished. If no content
                    // arrived and no reasoning was printed (e.g. pure tool-call-only response),
                    // finalize the spinner and print the badge now.
                    if !content_streaming_started && !reasoning_printed {
                        crate::tui_println!(
                            "{}{}● {}{}{}Thought for {:.1}s{}",
                            prefix,
                            RED_ORANGE,
                            COLOR_RESET,
                            COLOR_BOLD,
                            RED_ORANGE,
                            duration_secs,
                            COLOR_RESET
                        );
                    }
                } else {
                    // Non-streaming path: print the badge and thinking summary
                    crate::tui_println!(
                        "{}{}● {}{}{}Thought for {:.1}s{}",
                        prefix,
                        RED_ORANGE,
                        COLOR_RESET,
                        COLOR_BOLD,
                        RED_ORANGE,
                        duration_secs,
                        COLOR_RESET
                    );
                    let full_reasoning = if has_reasoning {
                        resp.reasoning_content.clone().unwrap_or_default()
                    } else if public_reasoning_progress && has_content && has_tool_calls {
                        resp.content.clone().unwrap_or_default()
                    } else {
                        String::new()
                    };
                    if !full_reasoning.is_empty() {
                        let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                        crate::agent::style::print_tree_monologue(&leaf_prefix, &full_reasoning);
                        print!("\r\n");
                    }
                    let _ = std::io::stdout().flush();
                }
                print!("\r\n");
                let _ = std::io::stdout().flush();
            }
        }

        if let Some(content) = resp.content {
            let text_repeat =
                super::loop_control::count_previous_text_responses(&ctx.messages, &content);
            if text_repeat >= 2 {
                let loop_msg = "⚠️ Halted execution: Detected repetitive text responses.";
                ctx.final_content = loop_msg.to_string();
                super::tool_execution::send_progress_update(ctx.session_key, loop_msg).await;
                if !crate::agent::style::spinner::is_silent() {
                    crate::tui_println!("{}⚠️ {}{}", AURA_GOLD, loop_msg, COLOR_RESET);
                }
                ctx.messages.push(Message {
                    role: "assistant".to_string(),
                    content: loop_msg.to_string(),
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra: serde_json::Map::new(),
                });
                break;
            }

            let content = if config.research.require_sources_for_current_claims
                && super::research_policy::has_live_research_intent(ctx.user_content)
            {
                turn_source_ledger.append_live_research_caveat_if_needed(content, true)
            } else {
                content
            };

            ctx.final_content = content.clone();
            let mut extra = serde_json::Map::new();
            if let Some(ref reasoning) = resp.reasoning_content {
                extra.insert(
                    "reasoning_content".to_string(),
                    serde_json::Value::String(reasoning.clone()),
                );
            }
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra,
            });
        }

        if resp.tool_calls.is_empty() {
            break;
        }

        let mut should_halt = false;
        let mut tool_results = Vec::new();
        let mut assistant_tool_calls_json = Vec::new();
        let mut loop_detection_messages = ctx.messages.clone();

        for call in resp.tool_calls {
            let call = auto_adjust_tool_call_for_user_intent(call, ctx.user_content);
            let call = auto_scope_context_before_edit(call, &mut auto_scoped_edit_paths);
            // Break early if already cancelled (e.g. previous tool in batch was cancelled)
            if turn_cancel.is_cancelled() {
                break;
            }
            ctx.tools_used.push(call.name.clone());

            crate::agent::activity::update_activity(
                ctx.session_key,
                "Executing tool",
                Some(&call.name),
            );
            let silent = crate::agent::style::spinner::is_silent();
            let formatted_args =
                super::tool_execution::format_tool_args(&call.name, &call.arguments);
            let tool_spinner_msg =
                crate::agent::style::get_tree_spinner_msg(&call.name, &formatted_args);
            let direct_url = direct_research_url(&call.name, &call.arguments);
            let duplicate_direct_url = direct_url
                .as_ref()
                .is_some_and(|url| completed_direct_research_urls.contains(url));
            let out_of_scope_direct_page_lookup = direct_page_only
                && direct_page_fetched
                && is_research_lookup_tool(&call.name)
                && !direct_url
                    .as_ref()
                    .is_some_and(|url| completed_direct_research_urls.contains(url));

            let tool_msg = format!("▸ Running *{}*...", formatted_args);
            super::tool_execution::send_progress_update(ctx.session_key, &tool_msg).await;

            if let Some(chat_id) = crate::channels::websocket::ws_chat_id(ctx.session_key) {
                crate::channels::websocket::publish_ws_event(serde_json::json!({
                    "event": "tool_start",
                    "chat_id": chat_id,
                    "tool_call_id": call.id.clone(),
                    "name": call.name.clone(),
                    "args": call.arguments.clone(),
                    "status": "running",
                }));
            }

            if !silent {
                crate::agent::style::print_tree_tool_start(&call.name, &formatted_args);
            }

            tracing::info!(
                session = %ctx.session_key,
                tool = %call.name,
                arguments = %call.arguments,
                "Executing tool call"
            );
            let fresh_brief_blocks_lookup =
                fresh_research_brief_blocks_lookup(ctx.user_content, &call.name, &call.arguments)
                    .await;
            let approval = super::security_approval::evaluate_tool_approval(
                &call,
                &loop_detection_messages,
                ctx.session_key,
                &config.agents.defaults.security_mode,
                silent,
                &mut loop_blocked_count,
            )
            .await;
            if approval.should_halt {
                should_halt = true;
            }

            let result_val = if out_of_scope_direct_page_lookup {
                let skip_msg = "Skipped broader research: this request supplied one direct URL. Use the fetched page, or ask for deeper/broader research to inspect additional sources.";
                super::tool_execution::send_progress_update(ctx.session_key, skip_msg).await;
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}↳ {}{}",
                        AURA_SLATE,
                        leaf_prefix,
                        AURA_BLUE,
                        skip_msg,
                        COLOR_RESET
                    );
                }
                serde_json::json!({ "status": "skipped", "reason": skip_msg })
            } else if duplicate_direct_url {
                let skip_msg = "Skipped duplicate URL lookup: this page was already fetched successfully in this turn. Use the previous result or continue with a different source.";
                super::tool_execution::send_progress_update(ctx.session_key, skip_msg).await;
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}↳ {}{}",
                        AURA_SLATE,
                        leaf_prefix,
                        AURA_BLUE,
                        skip_msg,
                        COLOR_RESET
                    );
                }
                serde_json::json!({ "status": "skipped", "reason": skip_msg })
            } else if fresh_brief_blocks_lookup {
                let skip_msg = "Skipped web/search lookup: a fresh saved research brief already matches this non-latest query. Answer from [Relevant Research Briefs] unless the user asks for latest/current data.";
                super::tool_execution::send_progress_update(ctx.session_key, skip_msg).await;
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}↳ {}{}",
                        AURA_SLATE,
                        leaf_prefix,
                        AURA_BLUE,
                        skip_msg,
                        COLOR_RESET
                    );
                }
                serde_json::json!({ "status": "skipped", "reason": skip_msg })
            } else if let Some(err_msg) = approval.parse_error.as_deref() {
                let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, err_msg);
                super::tool_execution::send_progress_update(ctx.session_key, &fail_msg).await;
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}✕ {} - Failed: {}{}",
                        AURA_SLATE,
                        leaf_prefix,
                        AURA_ROSE,
                        formatted_args,
                        err_msg,
                        COLOR_RESET
                    );
                }
                ctx.turn_errors.push(format!(
                    "Tool {} arguments parse error: {}",
                    call.name, err_msg
                ));
                serde_json::json!({ "error": err_msg })
            } else if approval.is_loop {
                let warning_str = format!(
                    "Loop detected: You have already executed the tool '{}' with these exact arguments {} times in this turn. To prevent infinite loops, execution was blocked. Do NOT call this tool again. Analyze previous tool outputs and use a different strategy, or finish your response.",
                    call.name, approval.repeat_count
                );
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}↶ Loop detected for tool '{}'! Blocking execution. (Count: {}){}",
                        AURA_SLATE,
                        leaf_prefix,
                        AURA_GOLD,
                        call.name,
                        loop_blocked_count,
                        COLOR_RESET
                    );
                }
                tracing::warn!(
                    session = %ctx.session_key,
                    tool = %call.name,
                    "Tool execution blocked (repetition/loop detected)"
                );
                serde_json::json!({ "error": warning_str })
            } else if approval.forbidden {
                let reject_msg = format!(
                    "✕ *{}* - Rejected: Dangerous command is forbidden",
                    formatted_args
                );
                super::tool_execution::send_progress_update(ctx.session_key, &reject_msg).await;
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}✕ {} - Rejected: Dangerous command is forbidden{}",
                        AURA_SLATE,
                        leaf_prefix,
                        ERROR_RED,
                        formatted_args,
                        COLOR_RESET
                    );
                }
                tracing::warn!(
                    session = %ctx.session_key,
                    tool = %call.name,
                    "Tool execution forbidden by security guard"
                );
                serde_json::json!({ "error": "Execution denied by host: This command is forbidden by security rules." })
            } else if !approval.approved {
                let deny_msg = format!("✕ *{}* - Denied by user", formatted_args);
                super::tool_execution::send_progress_update(ctx.session_key, &deny_msg).await;
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}✕ {} - Denied by user{}",
                        AURA_SLATE,
                        leaf_prefix,
                        ERROR_RED,
                        formatted_args,
                        COLOR_RESET
                    );
                }
                tracing::warn!(
                    session = %ctx.session_key,
                    tool = %call.name,
                    "Tool execution denied by user approval request"
                );
                serde_json::json!({ "error": "Execution denied by user." })
            } else {
                match loop_ref.tools.get(&call.name) {
                    Some(t) => {
                        let metadata = t.metadata();
                        let runtime =
                            crate::tools::resource_policy::RuntimeResourceSnapshot::current();
                        let policy_decision =
                            crate::tools::resource_policy::ToolResourcePolicy::evaluate(
                                &metadata,
                                &config.agents.defaults,
                                &runtime,
                            );

                        match policy_decision {
                            crate::tools::resource_policy::ToolResourceDecision::Block { reason }
                                if crate::tools::resource_policy::is_free_disk_block(&reason)
                                    && crate::tools::resource_policy::is_low_disk_recovery_tool(
                                        &call.name,
                                        &call.arguments,
                                    ) =>
                            {
                                execute_approved_tool(ApprovedToolExec {
                                    tool: t,
                                    call: &call,
                                    metadata: &metadata,
                                    config: &config,
                                    formatted_args: &formatted_args,
                                    session_key: ctx.session_key,
                                    silent,
                                    tool_spinner_msg: &tool_spinner_msg,
                                    turn_cancel: &turn_cancel,
                                    turn_errors: &mut ctx.turn_errors,
                                })
                                .await
                            }
                            crate::tools::resource_policy::ToolResourceDecision::Block { reason } => {
                                let error_str = format!(
                                    "Tool blocked by resource policy: {}. {}",
                                    call.name, reason
                                );
                                ctx.turn_errors.push(format!(
                                    "Tool {} blocked by resource policy: {}",
                                    call.name, reason
                                ));
                                super::tool_execution::render_tool_failure(
                                    &call,
                                    &formatted_args,
                                    ctx.session_key,
                                    silent,
                                    &error_str,
                                )
                                .await
                            }
                            crate::tools::resource_policy::ToolResourceDecision::RequireApproval { reason } => {
                                let approved = if approval.approval_requested {
                                    true
                                } else {
                                    crate::agent::security::ask_approval(
                                        ctx.session_key,
                                        &call.name,
                                        &serde_json::json!({
                                            "resource_policy_reason": reason,
                                            "arguments": call.arguments,
                                        }),
                                    )
                                    .await
                                    .unwrap_or(false)
                                };
                                if !approved {
                                    let error_str = format!(
                                        "Tool denied by resource policy approval: {}. {}",
                                        call.name, reason
                                    );
                                    ctx.turn_errors.push(format!(
                                        "Tool {} denied by resource policy approval: {}",
                                        call.name, reason
                                    ));
                                    super::tool_execution::render_tool_failure(
                                        &call,
                                        &formatted_args,
                                        ctx.session_key,
                                        silent,
                                        &error_str,
                                    )
                                    .await
                                } else {
                                    execute_approved_tool(ApprovedToolExec {
                                        tool: t.clone(),
                                        call: &call,
                                        metadata: &metadata,
                                        config: &config,
                                        formatted_args: &formatted_args,
                                        session_key: ctx.session_key,
                                        silent,
                                        tool_spinner_msg: &tool_spinner_msg,
                                        turn_cancel: &turn_cancel_clone,
                                        turn_errors: &mut ctx.turn_errors,
                                    })
                                    .await
                                }
                            }
                            crate::tools::resource_policy::ToolResourceDecision::Allow => {
                                execute_approved_tool(ApprovedToolExec {
                                    tool: t.clone(),
                                    call: &call,
                                    metadata: &metadata,
                                    config: &config,
                                    formatted_args: &formatted_args,
                                    session_key: ctx.session_key,
                                    silent,
                                    tool_spinner_msg: &tool_spinner_msg,
                                    turn_cancel: &turn_cancel_clone,
                                    turn_errors: &mut ctx.turn_errors,
                                })
                                .await
                            }
                        }
                    }
                    None => {
                        ctx.turn_errors
                            .push(format!("Tool {} not found", call.name));
                        super::tool_execution::render_tool_not_found(
                            &call,
                            &formatted_args,
                            ctx.session_key,
                            silent,
                        )
                        .await
                    }
                }
            };
            if let Some(chat_id) = crate::channels::websocket::ws_chat_id(ctx.session_key) {
                let (status, output) = if result_val.get("error").is_some() {
                    ("error", result_val.to_string())
                } else {
                    ("success", result_val.to_string())
                };
                let mut output = output;
                if output.len() > 2000 {
                    output.truncate(2000);
                    output.push_str("...");
                }
                crate::channels::websocket::publish_ws_event(serde_json::json!({
                    "event": "tool_end",
                    "chat_id": chat_id,
                    "tool_call_id": call.id.clone(),
                    "name": call.name.clone(),
                    "status": status,
                    "output": output,
                }));
            }
            if is_research_lookup_tool(&call.name) {
                turn_source_ledger.record_tool_result(&call.name, &call.arguments, &result_val);
            }
            if direct_url.is_some() && result_val.get("error").is_none() {
                if let Some(url) = direct_url {
                    completed_direct_research_urls.insert(url);
                    direct_page_fetched = true;
                }
            }
            if let Some(err_val) = result_val.get("error").and_then(|v| v.as_str()) {
                ctx.turn_errors
                    .push(format!("Tool {} returned error: {}", call.name, err_val));
            } else if let Ok(Some(capture)) =
                crate::tools::shared_memory::auto_capture_research_memory(
                    &call.name,
                    &call.arguments,
                    &result_val,
                    ctx.user_content,
                )
                .await
            {
                turn_capture_summaries.push(capture);
            }
            crate::agent::activity::update_activity(
                ctx.session_key,
                "Processing user prompt",
                None,
            );

            let maybe_auto_open_call = auto_open_artifact_call_after_tool(
                ctx.user_content,
                &call,
                &result_val,
                &mut auto_opened_artifact_paths,
            );
            let maybe_device_suggest_call = auto_device_inventory_suggest_call_after_open_failure(
                &call,
                &result_val,
                &mut auto_suggested_open_targets,
            );

            let transcript_result = super::transcript::ToolTranscriptResult {
                id: call.id.clone(),
                name: call.name.clone(),
                result: result_val,
            };
            tool_results.push(transcript_result.clone());

            let mut assistant_tool_call = serde_json::json!({
                "id": call.id,
                "type": "function",
                "function": {
                    "name": call.name,
                    "arguments": call.arguments.to_string()
                }
            });
            if let Some(fingerprint) = super::loop_control::tool_arg_fingerprint(&call.arguments) {
                if let Some(obj) = assistant_tool_call.as_object_mut() {
                    obj.insert(
                        "_openz_arg_fingerprint".to_string(),
                        serde_json::Value::String(fingerprint),
                    );
                }
            }
            let loop_assistant_tool_call = assistant_tool_call.clone();
            assistant_tool_calls_json.push(assistant_tool_call);
            super::transcript::append_assistant_tool_calls(
                &mut loop_detection_messages,
                vec![loop_assistant_tool_call],
                None,
            );
            let mut loop_tool_extra = serde_json::Map::new();
            loop_tool_extra.insert(
                "tool_call_id".to_string(),
                serde_json::Value::String(transcript_result.id),
            );
            loop_tool_extra.insert(
                "name".to_string(),
                serde_json::Value::String(transcript_result.name),
            );
            loop_detection_messages.push(crate::session::Message {
                role: "tool".to_string(),
                content: transcript_result.result.to_string(),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: loop_tool_extra,
            });

            if let Some(suggest_call) = maybe_device_suggest_call {
                if let Some(suggest_tool) = loop_ref.tools.get(&suggest_call.name) {
                    let metadata = suggest_tool.metadata();
                    let formatted_suggest_args = super::tool_execution::format_tool_args(
                        &suggest_call.name,
                        &suggest_call.arguments,
                    );
                    let suggest_spinner_msg = crate::agent::style::get_tree_spinner_msg(
                        &suggest_call.name,
                        &formatted_suggest_args,
                    );
                    let suggest_result = execute_approved_tool(ApprovedToolExec {
                        tool: suggest_tool,
                        call: &suggest_call,
                        metadata: &metadata,
                        config: &config,
                        formatted_args: &formatted_suggest_args,
                        session_key: ctx.session_key,
                        silent,
                        tool_spinner_msg: &suggest_spinner_msg,
                        turn_cancel: &turn_cancel_clone,
                        turn_errors: &mut ctx.turn_errors,
                    })
                    .await;

                    tool_results.push(super::transcript::ToolTranscriptResult {
                        id: suggest_call.id.clone(),
                        name: suggest_call.name.clone(),
                        result: suggest_result,
                    });

                    assistant_tool_calls_json.push(serde_json::json!({
                        "id": suggest_call.id,
                        "type": "function",
                        "function": {
                            "name": suggest_call.name,
                            "arguments": suggest_call.arguments.to_string()
                        },
                        "_openz_auto_tool": true
                    }));
                }
            }

            if let Some(open_call) = maybe_auto_open_call {
                if let Some(open_tool) = loop_ref.tools.get(&open_call.name) {
                    let metadata = open_tool.metadata();
                    let formatted_open_args = super::tool_execution::format_tool_args(
                        &open_call.name,
                        &open_call.arguments,
                    );
                    let open_spinner_msg = crate::agent::style::get_tree_spinner_msg(
                        &open_call.name,
                        &formatted_open_args,
                    );
                    let open_result = execute_approved_tool(ApprovedToolExec {
                        tool: open_tool,
                        call: &open_call,
                        metadata: &metadata,
                        config: &config,
                        formatted_args: &formatted_open_args,
                        session_key: ctx.session_key,
                        silent,
                        tool_spinner_msg: &open_spinner_msg,
                        turn_cancel: &turn_cancel_clone,
                        turn_errors: &mut ctx.turn_errors,
                    })
                    .await;
                    let maybe_open_suggest_call =
                        auto_device_inventory_suggest_call_after_open_failure(
                            &open_call,
                            &open_result,
                            &mut auto_suggested_open_targets,
                        );

                    tool_results.push(super::transcript::ToolTranscriptResult {
                        id: open_call.id.clone(),
                        name: open_call.name.clone(),
                        result: open_result,
                    });

                    assistant_tool_calls_json.push(serde_json::json!({
                        "id": open_call.id,
                        "type": "function",
                        "function": {
                            "name": open_call.name,
                            "arguments": open_call.arguments.to_string()
                        },
                        "_openz_auto_tool": true
                    }));

                    if let Some(suggest_call) = maybe_open_suggest_call {
                        if let Some(suggest_tool) = loop_ref.tools.get(&suggest_call.name) {
                            let metadata = suggest_tool.metadata();
                            let formatted_suggest_args = super::tool_execution::format_tool_args(
                                &suggest_call.name,
                                &suggest_call.arguments,
                            );
                            let suggest_spinner_msg = crate::agent::style::get_tree_spinner_msg(
                                &suggest_call.name,
                                &formatted_suggest_args,
                            );
                            let suggest_result = execute_approved_tool(ApprovedToolExec {
                                tool: suggest_tool,
                                call: &suggest_call,
                                metadata: &metadata,
                                config: &config,
                                formatted_args: &formatted_suggest_args,
                                session_key: ctx.session_key,
                                silent,
                                tool_spinner_msg: &suggest_spinner_msg,
                                turn_cancel: &turn_cancel_clone,
                                turn_errors: &mut ctx.turn_errors,
                            })
                            .await;

                            tool_results.push(super::transcript::ToolTranscriptResult {
                                id: suggest_call.id.clone(),
                                name: suggest_call.name.clone(),
                                result: suggest_result,
                            });

                            assistant_tool_calls_json.push(serde_json::json!({
                                "id": suggest_call.id,
                                "type": "function",
                                "function": {
                                    "name": suggest_call.name,
                                    "arguments": suggest_call.arguments.to_string()
                                },
                                "_openz_auto_tool": true
                            }));
                        }
                    }
                }
            }
        }

        super::transcript::append_assistant_tool_calls(
            &mut ctx.messages,
            assistant_tool_calls_json,
            resp.reasoning_content.as_deref(),
        );

        super::transcript::append_tool_results(&mut ctx.messages, &config, tool_results).await;

        ctx.session.messages = ctx.messages.clone();
        if iterations % 5 == 0 {
            if let Err(e) = loop_ref.session_manager.save(&ctx.session).await {
                tracing::warn!("Failed to save session incrementally in Run loop: {}", e);
            }
        }

        iterations += 1;
        if should_halt {
            let halt_msg = "⚠️ Halted execution: Too many repeating tool calls blocked by loop detection. Halting to save RAM and tokens.";
            ctx.final_content = halt_msg.to_string();
            super::tool_execution::send_progress_update(ctx.session_key, halt_msg).await;
            if !crate::agent::style::spinner::is_silent() {
                crate::tui_println!("{}⚠️ {}{}", AURA_GOLD, halt_msg, COLOR_RESET);
            }
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: halt_msg.to_string(),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
            break;
        }
    }

    if !turn_capture_summaries.is_empty() {
        let sources_saved: usize = turn_capture_summaries.iter().map(|c| c.sources_saved).sum();
        let briefs_saved = count_unique_auto_capture_brief_topics(&turn_capture_summaries);
        let topics = summarize_auto_capture_topics(&turn_capture_summaries);
        let output_visibility = crate::agent::events::OutputVisibility {
            memory_notices: ctx.config.agents.defaults.show_auto_capture_notices,
            ..Default::default()
        };
        if let Some(notice) = (crate::agent::events::AgentEvent::MemoryCaptureSummary {
            sources_saved,
            briefs_saved,
            topics: topics.clone(),
        })
        .public_text(&output_visibility)
        {
            crate::channels::cli::send_notification(&notice);
            crate::channels::websocket::publish_activity_notice(
                ctx.session_key,
                "memory",
                "Memory stored",
                format!(
                    "{} source(s), {} brief(s) | {}",
                    sources_saved, briefs_saved, topics
                ),
            );
        }
    }

    ctx.session.messages = ctx.messages.clone();
    if let Err(e) = loop_ref.session_manager.save(&ctx.session).await {
        tracing::warn!(
            "Failed to save session unconditionally on final iteration in Run loop: {}",
            e
        );
    }
    if let Some(ref inter_id) = ctx.interaction_id {
        if !ctx.turn_errors.is_empty() {
            let errors_str = ctx.turn_errors.join("\n");
            let _ =
                crate::tools::shared_memory::update_interaction_errors(inter_id, &errors_str).await;
        }
    }

    Ok(TurnState::Save)
}

fn normalize_tui_thought_display(mode: &str) -> &'static str {
    match mode.trim().to_lowercase().as_str() {
        "off" | "none" | "hide" | "hidden" => "off",
        "compact" | "summary" | "summarized" => "compact",
        _ => "full",
    }
}

fn should_show_tui_thoughts(mode: &str) -> bool {
    normalize_tui_thought_display(mode) != "off"
}

fn should_send_public_reasoning_progress(mode: &str) -> bool {
    should_show_tui_thoughts(mode)
}

fn compact_reasoning_summary(reasoning: &str) -> String {
    let mut text = reasoning.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > 360 {
        text = text.chars().take(357).collect::<String>();
        text.push_str("...");
    }
    text
}

fn format_markdown_line(line: &str) -> String {
    static RE_BOLD: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    static RE_CODE: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    static RE_ITALIC: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();

    let re_bold = RE_BOLD
        .get_or_init(|| regex::Regex::new(r"\*\*(.*?)\*\*").ok())
        .as_ref();
    let re_code = RE_CODE
        .get_or_init(|| regex::Regex::new(r"`(.*?)`").ok())
        .as_ref();
    let re_italic = RE_ITALIC
        .get_or_init(|| regex::Regex::new(r"\*(.*?)\*").ok())
        .as_ref();

    let light_blue = "\x1b[38;2;135;206;250m";

    let trimmed = line.trim();
    if trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3 && !trimmed.is_empty() {
        return format!("{}──────{}", LIGHT_WHITE, COLOR_RESET);
    }

    if line.trim_start().starts_with('#') {
        return format!("{}{}{}", HEADING_BLUE, line, COLOR_RESET);
    }

    let mut formatted = line.to_string();
    formatted = formatted
        .replace('✔', &format!("{}{}{}", EMERALD_GREEN, "✔", COLOR_RESET))
        .replace("✅", &format!("{}{}{}", EMERALD_GREEN, "✅", COLOR_RESET))
        .replace('✓', &format!("{}{}{}", EMERALD_GREEN, "✓", COLOR_RESET))
        .replace('✖', &format!("{}{}{}", ERROR_RED, "✖", COLOR_RESET))
        .replace("❌", &format!("{}{}{}", ERROR_RED, "❌", COLOR_RESET))
        .replace('✗', &format!("{}{}{}", ERROR_RED, "✗", COLOR_RESET));

    if let Some(re_bold) = re_bold {
        formatted = re_bold
            .replace_all(
                &formatted,
                &format!("{}{}$1{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET),
            )
            .to_string();
    }
    if let Some(re_code) = re_code {
        formatted = re_code
            .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
            .to_string();
    }
    if let Some(re_italic) = re_italic {
        formatted = re_italic
            .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
            .to_string();
    }

    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tui_thought_display_modes_normalize() {
        assert_eq!(normalize_tui_thought_display("full"), "full");
        assert_eq!(normalize_tui_thought_display("summary"), "compact");
        assert_eq!(normalize_tui_thought_display("off"), "off");
        assert!(should_show_tui_thoughts("full"));
        assert!(should_show_tui_thoughts("compact"));
        assert!(!should_show_tui_thoughts("off"));
    }

    #[test]
    fn direct_research_url_deduplicates_fragments_across_readers() {
        let first = direct_research_url(
            "web_fetch",
            &serde_json::json!({"url": "https://9router.com/#get-started"}),
        );
        let second = direct_research_url(
            "searchxyz_read_url",
            &serde_json::json!({"url": "https://9router.com/"}),
        );
        assert_eq!(first, second);
    }

    #[test]
    fn direct_research_url_ignores_non_url_research_tools() {
        assert!(direct_research_url(
            "web_search",
            &serde_json::json!({"query": "9router get started"}),
        )
        .is_none());
    }

    #[test]
    fn direct_page_scope_requires_explicit_broader_intent() {
        assert!(direct_page_research_only(
            "research this https://9router.com/#get-started"
        ));
        assert!(!direct_page_research_only(
            "research this https://9router.com/#get-started and compare related sources"
        ));
        assert!(!direct_page_research_only(
            "scrape the source code and assets from https://example.com/game"
        ));
    }

    #[test]
    fn public_reasoning_progress_is_hidden_by_default() {
        assert!(!should_send_public_reasoning_progress("off"));
        assert!(!should_send_public_reasoning_progress("hidden"));
        assert!(should_send_public_reasoning_progress("compact"));
        assert!(should_send_public_reasoning_progress("full"));
    }

    #[test]
    fn compact_reasoning_summary_truncates_long_text() {
        let raw = "word ".repeat(120);
        let compact = compact_reasoning_summary(&raw);
        assert!(compact.chars().count() <= 360);
        assert!(compact.ends_with("..."));
    }

    #[test]
    fn compact_reasoning_summary_never_returns_full_long_reasoning() {
        let raw = "internal step ".repeat(80);
        let compact = compact_reasoning_summary(&raw);
        assert!(compact.chars().count() <= 360);
        assert_ne!(compact, raw);
    }

    #[tokio::test]
    async fn fresh_brief_blocks_non_latest_research_tools() {
        let marker = uuid::Uuid::new_v4().to_string();
        let topic = format!("OpenHuman {marker}");
        crate::tools::shared_memory::save_research_brief(
            &topic,
            "OpenHuman is a local-first personal AI agent.",
            vec![],
            0.8,
            86400,
        )
        .await
        .unwrap();
        assert!(
            fresh_research_brief_blocks_lookup(
                &format!("what is openhuman {marker}"),
                "web_fetch",
                &serde_json::json!({}),
            )
            .await
        );
        assert!(
            !fresh_research_brief_blocks_lookup(
                &format!("latest openhuman {marker} release"),
                "web_fetch",
                &serde_json::json!({}),
            )
            .await
        );
        assert!(
            !fresh_research_brief_blocks_lookup(
                &format!("check this https://example.com/{marker}"),
                "web_fetch",
                &serde_json::json!({}),
            )
            .await
        );
        assert!(
            fresh_research_brief_blocks_lookup(
                &format!("what is openhuman {marker}"),
                "web_fetch",
                &serde_json::json!({ "url": format!("https://example.com/{marker}") }),
            )
            .await
        );
        assert!(
            !fresh_research_brief_blocks_lookup(
                &format!("go and check again openhuman {marker}"),
                "web_search",
                &serde_json::json!({}),
            )
            .await
        );
        assert!(
            !fresh_research_brief_blocks_lookup(
                &format!("what is openhuman {marker}"),
                "read_file",
                &serde_json::json!({}),
            )
            .await
        );
        assert!(
            !fresh_research_brief_blocks_lookup(
                &format!("research about openhuman {marker} and tell me about this"),
                "web_fetch",
                &serde_json::json!({}),
            )
            .await
        );
        let _ = crate::tools::shared_memory::delete_research_brief(&topic).await;
    }

    #[test]
    fn auto_capture_notice_dedupes_topics() {
        let summaries = vec![
            crate::tools::shared_memory::AutoCaptureSummary {
                sources_saved: 5,
                brief_saved: true,
                topic: "hermes".to_string(),
            },
            crate::tools::shared_memory::AutoCaptureSummary {
                sources_saved: 5,
                brief_saved: true,
                topic: "hermes".to_string(),
            },
            crate::tools::shared_memory::AutoCaptureSummary {
                sources_saved: 1,
                brief_saved: false,
                topic: "mem0".to_string(),
            },
        ];
        assert_eq!(summarize_auto_capture_topics(&summaries), "hermes, mem0");
        assert_eq!(count_unique_auto_capture_brief_topics(&summaries), 1);
    }

    #[test]
    fn resolves_tool_timeout_with_bounded_explicit_override() {
        assert_eq!(
            resolve_tool_timeout_secs(
                "web_fetch",
                &serde_json::json!({ "_timeout_secs": 1 }),
                Some(600),
                300,
            ),
            crate::tools::MIN_TOOL_TIMEOUT_SECS
        );
        assert_eq!(
            resolve_tool_timeout_secs(
                "web_fetch",
                &serde_json::json!({ "_timeout_secs": 999_999 }),
                Some(600),
                300,
            ),
            crate::tools::MAX_TOOL_TIMEOUT_SECS
        );
    }

    #[test]
    fn delegate_timeout_raises_outer_tool_timeout() {
        assert_eq!(
            resolve_tool_timeout_secs(
                "delegate_task",
                &serde_json::json!({ "timeout_secs": 900 }),
                Some(600),
                300,
            ),
            900
        );
        assert_eq!(
            resolve_tool_timeout_secs(
                "reviewer",
                &serde_json::json!({ "timeout_secs": 900 }),
                None,
                300,
            ),
            900
        );
    }

    #[test]
    fn parallel_research_uses_largest_task_timeout_for_outer_tool() {
        assert_eq!(
            resolve_tool_timeout_secs(
                "parallel_research",
                &serde_json::json!({
                    "tasks": [
                        { "goal": "quick", "timeout_secs": 120 },
                        { "goal": "deep", "timeout_secs": 900 }
                    ]
                }),
                Some(600),
                300,
            ),
            900
        );
    }

    #[test]
    fn tool_timeout_does_not_count_as_user_cancel() {
        assert!(!should_cancel_turn_after_tool_error(
            "Tool execution timed out after 300s"
        ));
        assert!(should_cancel_turn_after_tool_error("Cancelled by user"));
        assert!(should_cancel_turn_after_tool_error(
            "Subagent task cancelled"
        ));
    }

    #[test]
    fn provider_turn_lock_key_is_opt_in_for_fragile_models() {
        assert!(
            provider_turn_lock_key_for_mode("opencode_zen", "deepseek-v4-flash-free", None)
                .is_none()
        );
        assert!(provider_turn_lock_key_for_mode(
            "opencode_zen",
            "deepseek-v4-flash-free",
            Some("fragile")
        )
        .is_some());
        assert!(
            provider_turn_lock_key_for_mode("openrouter", "qwen/qwen3:free", Some("free"))
                .is_some()
        );
        assert!(provider_turn_lock_key_for_mode("openai", "gpt-4o", Some("fragile")).is_none());
        assert!(provider_turn_lock_key_for_mode("openai", "gpt-4o", Some("all")).is_some());
    }

    #[test]
    fn process_tool_guard_respects_resource_limit() {
        let first = crate::tools::resource_policy::try_acquire_process_tool(1)
            .expect("first process slot should be available");
        let err = crate::tools::resource_policy::try_acquire_process_tool(1)
            .expect_err("second process slot should be blocked");
        assert!(err.contains("process tool limit reached"));
        drop(first);
        assert!(crate::tools::resource_policy::try_acquire_process_tool(1).is_ok());
    }
}
