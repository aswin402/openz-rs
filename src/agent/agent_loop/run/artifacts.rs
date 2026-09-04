//! Tool-call normalization and local artifact follow-up helpers.

use super::user_content_requests_fresh_fetch;

pub(super) fn saved_tool_output_ref_from_read_file(
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

pub(super) fn auto_adjust_tool_call_for_user_intent(
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

pub(super) fn edit_tool_target_path(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Option<String> {
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

pub(super) fn scope_context_target_for_edit_path(target_path: &str) -> String {
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

pub(super) fn auto_scope_context_before_edit(
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

pub(super) fn user_content_requests_artifact_open(user_content: &str) -> bool {
    let lower = user_content.to_lowercase();
    [
        "show", "open", "display", "view", "preview", "play", "launch", "see it",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(super) fn is_local_artifact_path(path: &str) -> bool {
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

pub(super) fn artifact_path_from_value(value: &serde_json::Value) -> Option<String> {
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

pub(super) fn artifact_path_from_tool_result(
    call: &crate::providers::ToolCallRequest,
    result: &serde_json::Value,
) -> Option<String> {
    if result.get("error").is_some() {
        return None;
    }
    artifact_path_from_value(result).or_else(|| artifact_path_from_value(&call.arguments))
}

pub(super) fn auto_open_artifact_call_after_tool(
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

pub(super) fn artifact_open_category(target: &str) -> Option<&'static str> {
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
        "txt" | "md" | "rs" | "js" | "ts" | "html" | "htm" | "css" | "json" | "toml"
        | "yaml" | "yml" => Some("editor"),
        _ => Some("file_manager"),
    }
}

pub(super) fn auto_device_inventory_suggest_call_after_open_failure(
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
