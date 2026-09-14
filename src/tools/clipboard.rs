use crate::tools::Tool;
use anyhow::{anyhow, Result};
use arboard::Clipboard;
use serde_json::{json, Value};

pub struct ClipboardTool;

#[async_trait::async_trait]
impl Tool for ClipboardTool {
    fn name(&self) -> &str {
        "clipboard"
    }

    fn description(&self) -> &str {
        "Get or set text content in the system clipboard."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get", "set"],
                    "description": "The action to perform: 'get' to read text, 'set' to write text."
                },
                "text": {
                    "type": "string",
                    "description": "The text to set (required for 'set' action)."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let mut clipboard = Clipboard::new()
            .map_err(|e| anyhow!("Failed to initialize system clipboard: {}. (If running headless/CI, clipboard access may not be supported)", e))?;

        match action {
            "get" => {
                let text = clipboard
                    .get_text()
                    .map_err(|e| anyhow!("Failed to read text from system clipboard: {}", e))?;
                Ok(json!({
                    "status": "success",
                    "text": text
                }))
            }
            "set" => {
                let text = arguments
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'text' parameter for 'set' action"))?;
                clipboard
                    .set_text(text.to_string())
                    .map_err(|e| anyhow!("Failed to write text to system clipboard: {}", e))?;
                Ok(json!({
                    "status": "success",
                    "message": "Text successfully copied to clipboard"
                }))
            }
            _ => Err(anyhow!("Unsupported action: {}", action)),
        }
    }
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
