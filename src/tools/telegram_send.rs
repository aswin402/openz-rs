use crate::tools::{Tool, ToolMetadata, ToolRisk};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::Path;

const MAX_TELEGRAM_DOCUMENT_BYTES: u64 = 50 * 1024 * 1024;

pub struct TelegramSendDocumentTool;

fn valid_telegram_target(target: &str) -> bool {
    if target.is_empty() || target.chars().any(char::is_whitespace) {
        return false;
    }

    if target.starts_with('@') {
        let username = &target[1..];
        return (5..=32).contains(&username.len())
            && username
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    }

    let is_negative = target.starts_with('-');
    let numeric = target.strip_prefix('-').unwrap_or(target);
    if numeric.is_empty() || !numeric.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }

    // Positive Telegram user IDs currently fit within ten decimal digits.
    // Longer positive numbers are commonly phone numbers and are rejected.
    is_negative || numeric.len() <= 10
}

#[async_trait::async_trait]
impl Tool for TelegramSendDocumentTool {
    fn name(&self) -> &str {
        "telegram_send_document"
    }

    fn description(&self) -> &str {
        "Send a local file through the configured OpenZ Telegram bot to an explicit Telegram chat ID or @username. Never accepts phone numbers and never guesses a recipient. Requires user approval."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Local file path to send."
                },
                "chat_id": {
                    "type": "string",
                    "description": "Explicit Telegram numeric chat ID or @username. Phone numbers are rejected."
                },
                "caption": {
                    "type": "string",
                    "description": "Optional caption, up to 1024 characters."
                }
            },
            "required": ["path", "chat_id"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            domain: "communication",
            risk: ToolRisk::High,
            uses_network: true,
            writes_disk: false,
            spawns_process: false,
            requires_approval: true,
            priority: 45,
            aliases: &["send telegram file", "telegram document", "telegram upload"],
            examples: &["Send /tmp/report.pdf to Telegram chat 123456789"],
            when_to_use: "Use only after the user explicitly confirms the exact Telegram chat ID or username and the file path.",
            when_not_to_use: "Do not use phone numbers, guessed chat IDs, old session targets, or raw shell/API commands.",
            recommended_timeout_secs: Some(120),
        }
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        let chat_id = arguments
            .get("chat_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Missing 'chat_id' parameter"))?;
        let caption = arguments
            .get("caption")
            .and_then(Value::as_str)
            .unwrap_or("");

        if !valid_telegram_target(chat_id) {
            return Err(anyhow!(
                "Invalid Telegram target. Use an explicit numeric chat ID or @username; phone numbers are not accepted."
            ));
        }
        if caption.chars().count() > 1024 {
            return Err(anyhow!("Telegram caption exceeds the 1024-character limit"));
        }

        let resolved = crate::config::resolve_path(path);
        let metadata = tokio::fs::metadata(&resolved)
            .await
            .map_err(|err| anyhow!("Cannot read file '{}': {}", resolved.display(), err))?;
        if !metadata.is_file() {
            return Err(anyhow!("Telegram document path is not a regular file"));
        }
        if metadata.len() > MAX_TELEGRAM_DOCUMENT_BYTES {
            return Err(anyhow!(
                "Telegram document is too large ({} bytes; maximum is {} bytes)",
                metadata.len(),
                MAX_TELEGRAM_DOCUMENT_BYTES
            ));
        }

        let (token, client) =
            crate::channels::telegram::get_telegram_bot_info().ok_or_else(|| {
                anyhow!("Telegram bot channel is not active; start OpenZ's Telegram channel first")
            })?;
        let file = reqwest::multipart::Part::file(Path::new(&resolved))
            .await
            .map_err(|err| anyhow!("Cannot prepare Telegram document: {}", err))?
            .file_name(
                resolved
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("document")
                    .to_string(),
            );
        let mut form = reqwest::multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .part("document", file);
        if !caption.is_empty() {
            form = form.text("caption", caption.to_string());
        }

        let url = format!("https://api.telegram.org/bot{token}/sendDocument");
        let response = client
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(|err| anyhow!("Telegram send failed: {}", err))?;
        let status = response.status();
        let body: Value = response.json().await.map_err(|err| {
            anyhow!(
                "Telegram returned invalid response (HTTP {}): {}",
                status,
                err
            )
        })?;
        if !status.is_success() || body.get("ok").and_then(Value::as_bool) != Some(true) {
            let detail = body
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("Telegram rejected the document");
            return Err(anyhow!(
                "Telegram send failed (HTTP {}): {}",
                status,
                detail
            ));
        }

        Ok(json!({
            "status": "sent",
            "channel": "telegram",
            "target": chat_id,
            "file": resolved,
            "bytes": metadata.len(),
            "message_id": body.get("result").and_then(|result| result.get("message_id")),
            "do_not_retry": true
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::valid_telegram_target;

    #[test]
    fn accepts_explicit_chat_ids_and_usernames() {
        assert!(valid_telegram_target("1404322011"));
        assert!(valid_telegram_target("-1001234567890"));
        assert!(valid_telegram_target("@example_user"));
    }

    #[test]
    fn rejects_phone_numbers_and_ambiguous_targets() {
        assert!(!valid_telegram_target("+918870020639"));
        assert!(!valid_telegram_target("918870020639"));
        assert!(!valid_telegram_target("1404322011 extra"));
        assert!(!valid_telegram_target("@a"));
    }
}
