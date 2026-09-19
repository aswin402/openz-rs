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
        let (action_str, text_opt) = match arguments {
            Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.eq_ignore_ascii_case("get")
                    || trimmed.eq_ignore_ascii_case("read")
                    || trimmed.eq_ignore_ascii_case("paste")
                {
                    ("get".to_string(), None)
                } else {
                    ("set".to_string(), Some(s.to_string()))
                }
            }
            Value::Object(map) => {
                let text = map
                    .get("text")
                    .or_else(|| map.get("content"))
                    .or_else(|| map.get("value"))
                    .or_else(|| map.get("data"))
                    .or_else(|| map.get("message"))
                    .or_else(|| map.get("input"))
                    .and_then(|v| {
                        if let Some(s) = v.as_str() {
                            Some(s.to_string())
                        } else if v.is_number() || v.is_boolean() {
                            Some(v.to_string())
                        } else {
                            None
                        }
                    });

                let action_raw = map
                    .get("action")
                    .or_else(|| map.get("mode"))
                    .or_else(|| map.get("operation"))
                    .or_else(|| map.get("type"))
                    .or_else(|| map.get("command"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| {
                        if text.is_some() {
                            "set".to_string()
                        } else {
                            "get".to_string()
                        }
                    });

                (action_raw, text)
            }
            _ => return Err(anyhow!("Invalid arguments: expected object or text string")),
        };

        let action_norm = action_str.trim().to_lowercase().replace('-', "_");
        let action = match action_norm.as_str() {
            "get" | "read" | "copy_from" | "paste" | "fetch" => "get",
            "set" | "write" | "copy" | "copy_to" | "put" => "set",
            _ => action_norm.as_str(),
        };

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
                let text = text_opt
                    .ok_or_else(|| anyhow!("Missing 'text' parameter for 'set' action"))?;
                clipboard
                    .set_text(text)
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
