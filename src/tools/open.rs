use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct OpenTool;

#[async_trait::async_trait]
impl Tool for OpenTool {
    fn name(&self) -> &str {
        "open_path"
    }

    fn description(&self) -> &str {
        "Open a file, folder, or URL using the user's default system application (e.g. default web browser, text editor, or file manager)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "The file path, directory path, or URL to open."
                }
            },
            "required": ["target"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let raw_target = match arguments {
            Value::String(s) => s.trim().to_string(),
            Value::Object(map) => map
                .get("target")
                .or_else(|| map.get("path"))
                .or_else(|| map.get("file"))
                .or_else(|| map.get("url"))
                .or_else(|| map.get("uri"))
                .or_else(|| map.get("destination"))
                .or_else(|| map.get("location"))
                .or_else(|| map.get("link"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .ok_or_else(|| anyhow!("Missing 'target' parameter"))?,
            _ => return Err(anyhow!("Invalid arguments: expected object or target string")),
        };

        if raw_target.is_empty() {
            return Err(anyhow!("Missing or empty 'target' parameter"));
        }

        let mut target = raw_target.trim_matches(|c| c == '\'' || c == '"').trim();
        if let Some(stripped) = target.strip_prefix("file://") {
            target = stripped;
        }

        if !is_safe_target(target) {
            return Err(anyhow!(
                "Security: target contains unsafe characters or shell metacharacters"
            ));
        }

        // Validate URLs against SSRF
        if target.starts_with("http://") || target.starts_with("https://") {
            // Block shell metacharacters in URLs
            if target.contains(';')
                || target.contains('|')
                || target.contains('&')
                || target.contains('$')
                || target.contains('`')
                || target.contains('\n')
            {
                return Err(anyhow!("Security: URL contains shell metacharacters"));
            }
            let resolved = target.to_string();
            let resolved_clone = resolved.clone();
            let status = tokio::task::spawn_blocking(move || open::that(resolved_clone)).await?;
            match status {
                Ok(_) => {
                    let device_inventory_recorded = record_device_open_success(&resolved);
                    Ok(json!({
                        "status": "success",
                        "message": format!("Successfully opened '{}'", resolved),
                        "user_visible": true,
                        "do_not_retry": true,
                        "device_inventory_recorded": device_inventory_recorded,
                        "instruction": "The target was handed to the user's default application and the successful open was recorded for future local-device suggestions. Treat this as complete and do not try another viewer unless the user says it failed."
                    }))
                }
                Err(e) => Err(anyhow!("Failed to open '{}': {}", resolved, e)),
            }
        } else {
            // For file paths, resolve and validate
            let resolved = crate::config::resolve_path(target)
                .to_string_lossy()
                .to_string();
            let resolved_clone = resolved.clone();
            let status = tokio::task::spawn_blocking(move || open::that(resolved_clone)).await?;
            match status {
                Ok(_) => {
                    let device_inventory_recorded = record_device_open_success(&resolved);
                    Ok(json!({
                        "status": "success",
                        "message": format!("Successfully opened '{}'", resolved),
                        "user_visible": true,
                        "do_not_retry": true,
                        "device_inventory_recorded": device_inventory_recorded,
                        "instruction": "The target was handed to the user's default application and the successful open was recorded for future local-device suggestions. Treat this as complete and do not try another viewer unless the user says it failed."
                    }))
                }
                Err(e) => Err(anyhow!("Failed to open '{}': {}", resolved, e)),
            }
        }
    }
}

fn record_device_open_success(target: &str) -> Option<String> {
    match crate::tools::device_inventory::record_successful_default_open(target) {
        Ok(id) => id,
        Err(error) => {
            tracing::warn!(%error, target, "failed to record open_path success in device inventory");
            None
        }
    }
}

fn is_safe_target(target: &str) -> bool {
    target.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || c == '/'
            || c == '.'
            || c == '_'
            || c == '-'
            || c == ':'
            || c == '?'
            || c == '='
            || c == '%'
            || c == '+'
            || c == '#'
            || c == '@'
            || c == '~'
            || c == ' '
    })
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
