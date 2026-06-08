use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;

pub struct GsdBrowserTool;

#[async_trait::async_trait]
impl Tool for GsdBrowserTool {
    fn name(&self) -> &str {
        "gsd_browser"
    }

    fn description(&self) -> &str {
        "Control a real Chrome browser instance to navigate pages, interact with elements using reference IDs, and take page structure snapshots."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["navigate", "snapshot", "click", "fill"],
                    "description": "The browser action: 'navigate' to URL, 'snapshot' for structures, 'click' element, 'fill' text input."
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (required for 'navigate')."
                },
                "ref_id": {
                    "type": "string",
                    "description": "Element reference ID from snapshot, e.g. '@v1:e5' (required for 'click' and 'fill')."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type into input element (required for 'fill')."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let mut cmd = Command::new("gsd-browser");

        match action {
            "navigate" => {
                let url = arguments.get("url").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'url' parameter for navigate action"))?;
                cmd.arg("navigate").arg(url);
            }
            "snapshot" => {
                cmd.arg("snapshot");
            }
            "click" => {
                let ref_id = arguments.get("ref_id").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for click action"))?;
                cmd.arg("click-ref").arg(ref_id);
            }
            "fill" => {
                let ref_id = arguments.get("ref_id").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for fill action"))?;
                let text = arguments.get("text").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'text' parameter for fill action"))?;
                cmd.arg("fill-ref").arg(ref_id).arg(text);
            }
            _ => return Err(anyhow!("Unsupported browser action: {}", action)),
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(anyhow!("gsd-browser error: {}", if stderr.trim().is_empty() { stdout } else { stderr }));
        }

        Ok(json!({
            "status": "success",
            "output": stdout.trim()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gsd_browser_struct() -> Result<()> {
        let tool = GsdBrowserTool;
        assert_eq!(tool.name(), "gsd_browser");
        Ok(())
    }
}
