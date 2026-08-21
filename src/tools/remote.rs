use crate::tools::Tool;
use anyhow::{Result, anyhow};
use serde_json::json;

pub struct SendRemoteInputTool;

fn is_self_target(current_session: Option<&str>, target_session: &str) -> bool {
    let Some(current) = current_session else {
        return false;
    };
    current == target_session || (target_session == "cli:direct" && current.starts_with("cli:"))
}

#[async_trait::async_trait]
impl Tool for SendRemoteInputTool {
    fn name(&self) -> &str {
        "send_remote_input"
    }

    fn description(&self) -> &str {
        "Queues a prompt, command, or query for another active agent session on the computer (e.g. 'cli:direct'). Self-targeting the current session is rejected to prevent TUI input loops."
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

        if is_self_target(
            crate::agent::style::spinner::get_current_session_key().as_deref(),
            session_id,
        ) {
            return Err(anyhow!(
                "Cannot send remote input to the current session; target another channel or session"
            ));
        }

        let target_session = if session_id == "cli:direct" {
            crate::agent::activity::resolve_cli_direct_target()?
        } else {
            session_id.to_string()
        };

        if target_session.starts_with("cli:")
            && !crate::agent::activity::active_tui_session_exists(&target_session)
        {
            return Err(anyhow!(
                "No active TUI session matches '{}'; start openz agent first",
                target_session
            ));
        }

        crate::agent::activity::send_inbox_message(&target_session, message, "remote")?;

        Ok(json!({
            "status": "success",
            "detail": format!("Successfully forwarded remote prompt to session '{}'", session_id)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::is_self_target;

    #[test]
    fn rejects_current_cli_and_direct_alias_targets() {
        assert!(is_self_target(Some("cli:abc"), "cli:abc"));
        assert!(is_self_target(Some("cli:abc"), "cli:direct"));
        assert!(!is_self_target(Some("telegram:123"), "cli:direct"));
    }
}
