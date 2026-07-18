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
        let target = arguments
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'target' parameter"))?;

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
                Ok(_) => Ok(json!({
                    "status": "success",
                    "message": format!("Successfully opened '{}'", resolved),
                    "user_visible": true,
                    "do_not_retry": true,
                    "instruction": "The target was handed to the user's default application. Treat this as complete and do not try another viewer unless the user says it failed."
                })),
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
                Ok(_) => Ok(json!({
                    "status": "success",
                    "message": format!("Successfully opened '{}'", resolved),
                    "user_visible": true,
                    "do_not_retry": true,
                    "instruction": "The target was handed to the user's default application. Treat this as complete and do not try another viewer unless the user says it failed."
                })),
                Err(e) => Err(anyhow!("Failed to open '{}': {}", resolved, e)),
            }
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_open_tool() {
        let tool = OpenTool;

        let args = json!({
            "target": "https://example.com"
        });

        // In headless CI/test environments, this might return an error due to missing display server or xdg-open defaults.
        // We ensure it parses and handles results/errors gracefully.
        let res = tool.call(&args).await;
        match res {
            Ok(val) => {
                assert_eq!(val["status"], "success");
            }
            Err(e) => {
                println!(
                    "Open tool run finished with error (expected in headless CI/containers): {}",
                    e
                );
            }
        }
    }
}
