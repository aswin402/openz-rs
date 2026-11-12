use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::json;

pub struct SendRemoteInputTool;

fn is_self_target(current_session: Option<&str>, target_session: &str) -> bool {
    match current_session {
        Some(current) => {
            current == target_session
                || (target_session == "cli:direct" && current.starts_with("cli:"))
                || (target_session == "ws:direct" && current.starts_with("ws:"))
        }
        None => false,
    }
}

#[async_trait::async_trait]
impl Tool for SendRemoteInputTool {
    fn name(&self) -> &str {
        "send_remote_input"
    }

    fn description(&self) -> &str {
        "Queues a prompt, command, or query for another active OpenZ session on this machine — either a TUI session ('cli:direct' or 'cli:<id>') or a WebUI/Gateway session ('ws:direct', 'ws:new', or 'ws:<id>'). Self-targeting the current session is rejected."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "Target session ID: 'cli:direct' (active TUI), 'ws:direct' (live open WebUI session), 'ws:new' (fresh WebUI session), 'list' (list available sessions), or an explicit 'cli:<id>' / 'ws:<id>'."
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
            .unwrap_or("");

        match session_id.trim() == "list" {
            true => {
                let sessions = crate::agent::activity::list_remote_target_sessions();
                return Ok(json!({
                    "status": "success",
                    "sessions": sessions
                }));
            }
            false => {}
        }

        match message.trim().is_empty() {
            true => return Err(anyhow!("Missing non-empty 'message'")),
            false => {}
        }

        match is_self_target(
            crate::agent::style::spinner::get_current_session_key().as_deref(),
            session_id,
        ) {
            true => {
                return Err(anyhow!(
                    "Cannot send remote input to the current session; target another channel or session"
                ));
            }
            false => {}
        }

        let target_session = match session_id {
            "cli:direct" => crate::agent::activity::resolve_cli_direct_target()?,
            "ws:direct" => crate::agent::activity::resolve_ws_direct_target()?,
            "ws:new" => {
                match crate::agent::activity::get_active_gateway_status().is_some() {
                    true => format!("ws:{}", uuid::Uuid::new_v4()),
                    false => {
                        return Err(anyhow!(
                            "Gateway / WebUI server is not active; launch `openz webui` first"
                        ));
                    }
                }
            }
            _ => session_id.to_string(),
        };

        let missing_active_tui = target_session.starts_with("cli:")
            && !crate::agent::activity::active_tui_session_exists(&target_session);
        match missing_active_tui {
            true => {
                return Err(anyhow!(
                    "No active TUI session matches '{}'; start openz first",
                    target_session
                ));
            }
            false => {}
        }

        let missing_active_gateway = target_session.starts_with("ws:")
            && crate::agent::activity::get_active_gateway_status().is_none();
        match missing_active_gateway {
            true => {
                return Err(anyhow!(
                    "No active WebUI / Gateway server is running for '{}'; start `openz webui` first",
                    target_session
                ));
            }
            false => {}
        }

        let sender = crate::agent::style::spinner::get_current_session_key()
            .filter(|key| key.starts_with("telegram:"))
            .unwrap_or_else(|| "remote".to_string());
        crate::agent::activity::send_inbox_message(&target_session, message, &sender)?;

        Ok(json!({
            "status": "success",
            "target_session": target_session,
            "detail": format!("Successfully forwarded remote prompt to session '{}'", target_session)
        }))
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
