use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct DesktopNotifyTool;

#[async_trait::async_trait]
impl Tool for DesktopNotifyTool {
    fn name(&self) -> &str {
        "desktop_notify"
    }

    fn description(&self) -> &str {
        "Show a desktop notification on the user's current graphical session. Use this instead of shell notify-send or editing system crontab."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Short notification title." },
                "message": { "type": "string", "description": "Notification body." },
                "urgency": { "type": "string", "enum": ["low", "normal", "critical"] }
            },
            "required": ["message"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let title = arguments
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("OpenZ")
            .trim();
        let message = arguments
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|message| !message.is_empty())
            .ok_or_else(|| anyhow!("Missing non-empty 'message' parameter"))?;
        let urgency = arguments
            .get("urgency")
            .and_then(Value::as_str)
            .unwrap_or("normal");

        let status = tokio::process::Command::new("notify-send")
            .arg("--app-name")
            .arg("OpenZ")
            .arg("--urgency")
            .arg(urgency)
            .arg(title)
            .arg(message)
            .output()
            .await
            .map_err(|error| anyhow!("notify-send is unavailable: {error}"))?;
        if !status.status.success() {
            return Err(anyhow!(
                "Desktop notification failed: {}",
                String::from_utf8_lossy(&status.stderr).trim()
            ));
        }

        let inventory_id = crate::tools::device_inventory::record_successful_desktop_notification()
            .ok()
            .flatten();
        Ok(json!({
            "status": "success",
            "delivered": true,
            "title": title,
            "message": message,
            "inventory_id": inventory_id,
            "instruction": "The notification was handed to the current desktop session. Do not retry unless the user reports that it was not visible."
        }))
    }
}
