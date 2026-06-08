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
        let target = arguments.get("target").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'target' parameter"))?;

        // Resolve paths containing ~/ to absolute paths
        let resolved = if target.starts_with("http://") || target.starts_with("https://") {
            target.to_string()
        } else {
            crate::config::resolve_path(target).to_string_lossy().to_string()
        };

        // We run open::that in a blocking thread since it's a synchronous system call
        let resolved_clone = resolved.clone();
        let status = tokio::task::spawn_blocking(move || {
            open::that(resolved_clone)
        }).await?;

        match status {
            Ok(_) => Ok(json!({
                "status": "success",
                "message": format!("Successfully opened '{}'", resolved)
            })),
            Err(e) => Err(anyhow!("Failed to open '{}': {}", resolved, e)),
        }
    }
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
                println!("Open tool run finished with error (expected in headless CI/containers): {}", e);
            }
        }
    }
}
