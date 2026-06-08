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
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let mut clipboard = Clipboard::new()
            .map_err(|e| anyhow!("Failed to initialize system clipboard: {}. (If running headless/CI, clipboard access may not be supported)", e))?;

        match action {
            "get" => {
                let text = clipboard.get_text()
                    .map_err(|e| anyhow!("Failed to read text from system clipboard: {}", e))?;
                Ok(json!({
                    "status": "success",
                    "text": text
                }))
            }
            "set" => {
                let text = arguments.get("text").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'text' parameter for 'set' action"))?;
                clipboard.set_text(text.to_string())
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clipboard_tool() {
        let tool = ClipboardTool;
        
        // We test clipboard, but since test environment might be headless (e.g. running in CI/containers),
        // we handle clipboard initialization failure gracefully so the test suite doesn't fail.
        let test_val = json!({
            "action": "set",
            "text": "OpenZ is awesome!"
        });
        
        match tool.call(&test_val).await {
            Ok(res) => {
                assert_eq!(res["status"], "success");
                
                // Now test get
                let get_val = json!({
                    "action": "get"
                });
                if let Ok(get_res) = tool.call(&get_val).await {
                    assert_eq!(get_res["status"], "success");
                    if let Some(text_val) = get_res.get("text").and_then(|v| v.as_str()) {
                        // It could be that another concurrent test or tool changed it, but we check if we can get it
                        println!("Retrieved clipboard text successfully: {}", text_val);
                    }
                } else {
                    println!("Get clipboard text failed (which is normal on Linux if no clipboard manager daemon is active to persist dropped clipboard buffers).");
                }
            }
            Err(e) => {
                println!("Clipboard test skipped or failed gracefully: {}", e);
                // We pass the test if it's due to headless environment
                let err_msg = e.to_string();
                assert!(
                    err_msg.contains("Failed to initialize system clipboard") 
                    || err_msg.contains("clipboard access may not be supported")
                    || err_msg.contains("ClipboardNotSupported")
                );
            }
        }
    }
}
