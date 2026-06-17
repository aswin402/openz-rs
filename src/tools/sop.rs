use crate::tools::Tool;
use anyhow::{Result, anyhow};
use serde_json::{json, Value};
use crate::config::schema::Config;

pub struct TriggerSopTool {
    pub config: Config,
}

#[async_trait::async_trait]
impl Tool for TriggerSopTool {
    fn name(&self) -> &str {
        "trigger_sop"
    }

    fn description(&self) -> &str {
        "Trigger a stateful Standard Operating Procedure (SOP) closed-loop workflow by ID. Available SOPs: 'ship-pr-until-green' (loops implementation, branch creation, remote PR creation, and remote CI checks verification & self-healing), 'pre-commit-guard' (automatic pre-commit test hook config and verification), 'pr-review' (automated PR analyzer), 'incident-response', 'feature-release'."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "sop_id": {
                    "type": "string",
                    "description": "The ID of the SOP loop definition to execute (e.g. 'ship-pr-until-green' or 'pre-commit-guard')."
                },
                "payload": {
                    "type": "object",
                    "description": "Optional key-value parameters/inputs required by the SOP steps (e.g. {'feature_request': 'implement a new database method'})."
                }
            },
            "required": ["sop_id"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let sop_id = arguments.get("sop_id").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'sop_id' parameter"))?;
        
        let payload = arguments.get("payload").cloned().unwrap_or(json!({}));

        let instance_id = crate::sop::engine::trigger_sop(self.config.clone(), sop_id.to_string(), payload).await?;

        Ok(json!({
            "status": "success",
            "sop_id": sop_id,
            "instance_id": instance_id,
            "message": format!("SOP loop '{}' successfully triggered. Instance ID: {}", sop_id, instance_id)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_trigger_sop_tool_metadata() {
        let config = Config::default();
        let tool = TriggerSopTool { config };
        assert_eq!(tool.name(), "trigger_sop");
        assert!(tool.description().contains("Trigger a stateful"));
        
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert!(params["properties"]["sop_id"].is_object());
    }
}
