//! Tool-call normalization and local artifact follow-up helpers.

use super::user_content_requests_fresh_fetch;

pub(super) fn saved_tool_output_ref_from_read_file(
    call: &crate::providers::ToolCallRequest,
) -> Option<String> {
    (crate::tools::canonical_tool_name(&call.name) == "read_file").then_some(())?;

    let raw_paths = crate::tools::arguments::argument_values(
        &call.arguments,
        crate::tools::arguments::ToolArgumentKind::Path,
    );
    let raw_path = raw_paths
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())?;

    let path_without_scheme = raw_path.strip_prefix("file://").unwrap_or(raw_path);
    let path = std::path::Path::new(path_without_scheme);
    let outputs_dir = crate::config::loader::runtime_data_dir().join("tool_outputs");

    (path.is_absolute() && path.starts_with(&outputs_dir)).then(|| {
        raw_path
            .strip_prefix("file://")
            .map(|_| raw_path.to_string())
            .unwrap_or_else(|| format!("file://{}", path.to_string_lossy()))
    })
}

pub(super) fn auto_adjust_tool_call_for_user_intent(
    mut call: crate::providers::ToolCallRequest,
    user_content: &str,
) -> crate::providers::ToolCallRequest {
    match saved_tool_output_ref_from_read_file(&call) {
        Some(original_ref) => {
            return crate::providers::ToolCallRequest {
                id: call.id,
                name: "retrieve_original".to_string(),
                arguments: serde_json::json!({ "ccr_id": original_ref }),
            };
        }
        None => {}
    }

    let should_revalidate = crate::tools::canonical_tool_name(&call.name) == "web_fetch"
        && user_content_requests_fresh_fetch(user_content)
        && call.arguments.get("cache_mode").is_none()
        && call.arguments.get("cacheMode").is_none();
    let _ = should_revalidate
        .then(|| call.arguments.as_object_mut())
        .flatten()
        .map(|map| {
            map.insert(
                "cache_mode".to_string(),
                serde_json::Value::String("revalidate".to_string()),
            )
        });
    call
}

pub(super) fn edit_tool_target_path(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Option<String> {
    let canonical = crate::tools::canonical_tool_name(tool_name);
    matches!(
        canonical.as_str(),
        "write_file" | "patch_file" | "replace_lines" | "zenflow_edit"
    )
    .then(|| {
        crate::tools::arguments::argument_values(
            arguments,
            crate::tools::arguments::ToolArgumentKind::Path,
        )
        .into_iter()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
    })
    .flatten()
}

pub(super) fn scope_context_target_for_edit_path(target_path: &str) -> String {
    let trimmed = target_path.trim();
    let path = std::path::Path::new(trimmed);
    match path.exists() {
        true => trimmed.to_string(),
        false => {
            let mut parent = path.parent();
            while let Some(candidate) = parent {
                match (candidate.as_os_str().is_empty(), candidate.exists()) {
                    (true, _) => return ".".to_string(),
                    (false, true) => return candidate.to_string_lossy().to_string(),
                    (false, false) => parent = candidate.parent(),
                }
            }
            trimmed.to_string()
        }
    }
}

pub(super) fn should_record_auto_scope_result(scope_result: &serde_json::Value) -> bool {
    scope_result.get("status").and_then(|v| v.as_str()) != Some("empty")
}

pub(super) fn auto_scope_context_before_edit(
    call: crate::providers::ToolCallRequest,
    scoped_edit_paths: &mut std::collections::HashSet<String>,
) -> crate::providers::ToolCallRequest {
    match edit_tool_target_path(&call.name, &call.arguments) {
        None => call,
        Some(edit_path) => {
            let target_path = scope_context_target_for_edit_path(&edit_path);
            let canonical_target = std::fs::canonicalize(&target_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| {
                    crate::config::path_policy::canonicalize_with_missing_leaf(
                        &crate::config::loader::resolve_path(&target_path),
                    )
                    .to_string_lossy()
                    .to_string()
                });
            let canonical_dir = (!std::path::Path::new(&canonical_target).is_dir())
                .then(|| {
                    std::path::Path::new(&canonical_target)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                })
                .flatten()
                .filter(|s| !s.is_empty());

            let already_scoped = scoped_edit_paths.contains(&target_path)
                || scoped_edit_paths.contains(&canonical_target)
                || canonical_dir
                    .as_ref()
                    .is_some_and(|dir| scoped_edit_paths.contains(dir));
            scoped_edit_paths.insert(target_path.clone());
            scoped_edit_paths.insert(canonical_target);
            canonical_dir.map(|dir| scoped_edit_paths.insert(dir));

            match already_scoped {
                true => call,
                false => crate::providers::ToolCallRequest {
                    id: call.id,
                    name: "scope_context".to_string(),
                    arguments: serde_json::json!({
                        "target_path": target_path,
                        "auto_reason": "before_edit"
                    }),
                },
            }
        }
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
    match clean.starts_with("http://") || clean.starts_with("https://") {
        true => false,
        false => std::path::Path::new(clean)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .is_some_and(|ext| {
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
            }),
    }
}

pub(super) fn artifact_path_from_value(value: &serde_json::Value) -> Option<String> {
    let obj = value.as_object()?;
    [
        "output_path",
        "outputPath",
        "path",
        "file_path",
        "filePath",
        "target",
        "url",
    ]
    .iter()
    .find_map(|key| {
        obj.get(*key)
            .and_then(|value| value.as_str())
            .filter(|path| is_local_artifact_path(path))
            .map(|path| path.to_string())
    })
}

pub(super) fn artifact_path_from_tool_result(
    call: &crate::providers::ToolCallRequest,
    result: &serde_json::Value,
) -> Option<String> {
    result.get("error").is_none().then_some(())?;
    artifact_path_from_value(result).or_else(|| artifact_path_from_value(&call.arguments))
}

pub(super) fn auto_open_artifact_call_after_tool(
    user_content: &str,
    call: &crate::providers::ToolCallRequest,
    result: &serde_json::Value,
    opened_artifact_paths: &mut std::collections::HashSet<String>,
) -> Option<crate::providers::ToolCallRequest> {
    (call.name != "open_path" && user_content_requests_artifact_open(user_content))
        .then_some(())?;

    let target = artifact_path_from_tool_result(call, result)?;
    opened_artifact_paths.insert(target.clone()).then_some(())?;

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
    match clean.starts_with("http://") || clean.starts_with("https://") {
        true => Some("browser"),
        false => {
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
    }
}

pub(super) fn auto_device_inventory_suggest_call_after_open_failure(
    call: &crate::providers::ToolCallRequest,
    result: &serde_json::Value,
    suggested_open_targets: &mut std::collections::HashSet<String>,
) -> Option<crate::providers::ToolCallRequest> {
    (call.name == "open_path" && result.get("error").is_some()).then_some(())?;
    let target = call
        .arguments
        .get("target")
        .and_then(|value| value.as_str())?;
    suggested_open_targets
        .insert(target.to_string())
        .then_some(())?;
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
