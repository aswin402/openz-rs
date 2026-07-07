use crate::tools::Tool;
use anyhow::Result;
use serde_json::json;

pub struct SendRemoteInputTool;

#[async_trait::async_trait]
impl Tool for SendRemoteInputTool {
    fn name(&self) -> &str {
        "send_remote_input"
    }

    fn description(&self) -> &str {
        "Sends a prompt, command, or query to another active agent session on the computer (e.g. 'cli:direct') so the session executes it immediately."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The target session ID to send the input to (typically 'cli:direct')."
                },
                "message": {
                    "type": "string",
                    "description": "The prompt or instruction to feed into the target session."
                }
            },
            "required": ["session_id", "message"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let session_id = arguments
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'session_id'"))?;
        let message = arguments
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'message'"))?;

        crate::agent::activity::send_inbox_message(session_id, message, "remote")?;

        Ok(json!({
            "status": "success",
            "detail": format!("Successfully forwarded remote prompt to session '{}'", session_id)
        }))
    }
}
