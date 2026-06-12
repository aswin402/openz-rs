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
        "Control a real Chrome browser instance to navigate pages, interact with elements using reference IDs, evaluate JS, take screenshots, or save PDFs."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "navigate",
                        "snapshot",
                        "click",
                        "hover",
                        "fill",
                        "screenshot",
                        "eval",
                        "accessibility_tree",
                        "page_source",
                        "save_pdf"
                    ],
                    "description": "The browser action: 'navigate' to a URL, 'snapshot' to get interactive elements, 'click' or 'hover' on an element ref, 'fill' text input ref, 'screenshot' to capture image, 'eval' to run custom JavaScript, 'accessibility_tree' for roles/a11y tree, 'page_source' for HTML, 'save_pdf' to save as PDF."
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (required for 'navigate')."
                },
                "ref_id": {
                    "type": "string",
                    "description": "Element reference ID from snapshot, e.g. '@v1:e5' (required for 'click', 'hover', and 'fill')."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type into input element (required for 'fill')."
                },
                "path": {
                    "type": "string",
                    "description": "Output file path (required for 'screenshot' and 'save_pdf')."
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript expression to evaluate (required for 'eval')."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let bin_path = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("gsd-browser");
            if p.exists() { p } else { std::path::PathBuf::from("gsd-browser") }
        } else {
            std::path::PathBuf::from("gsd-browser")
        };
        let mut cmd = Command::new(bin_path);

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
            "hover" => {
                let ref_id = arguments.get("ref_id").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for hover action"))?;
                cmd.arg("hover-ref").arg(ref_id);
            }
            "fill" => {
                let ref_id = arguments.get("ref_id").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for fill action"))?;
                let text = arguments.get("text").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'text' parameter for fill action"))?;
                cmd.arg("fill-ref").arg(ref_id).arg(text);
            }
            "screenshot" => {
                let path = arguments.get("path").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'path' parameter for screenshot action"))?;
                let resolved = crate::config::resolve_path(path);
                cmd.arg("screenshot").arg("--output").arg(resolved);
            }
            "eval" => {
                let script = arguments.get("script").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'script' parameter for eval action"))?;
                cmd.arg("eval").arg(script);
            }
            "accessibility_tree" => {
                cmd.arg("accessibility-tree");
            }
            "page_source" => {
                cmd.arg("page-source");
            }
            "save_pdf" => {
                let path = arguments.get("path").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'path' parameter for save_pdf action"))?;
                let resolved = crate::config::resolve_path(path);
                cmd.arg("save-pdf").arg("--output").arg(resolved);
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
