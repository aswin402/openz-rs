use crate::config::schema::Config;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct TriggerSopTool {
    pub config: Config,
}

#[async_trait::async_trait]
impl Tool for TriggerSopTool {
    fn name(&self) -> &str {
        "trigger_sop"
    }

    fn description(&self) -> &str {
        "Trigger or inspect a stateful Standard Operating Procedure (SOP) closed-loop workflow. Actions: 'trigger' (start new run), 'status' (get execution progress), 'output' (get step results and outputs), 'list' (list available SOP definitions and past runs), 'resume' (resume a paused, pending, or failed SOP run)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["trigger", "status", "output", "list", "resume"],
                    "description": "The action to perform (default: 'trigger')."
                },
                "sop_id": {
                    "type": "string",
                    "description": "The ID of the SOP loop definition to execute (e.g. 'ship-pr-until-green', 'pre-commit-guard', 'pr-review', 'incident-response', 'feature-release'). Required for 'trigger'."
                },
                "instance_id": {
                    "type": "string",
                    "description": "The instance ID of an existing SOP execution. Required for 'status', 'output', and 'resume'."
                },
                "payload": {
                    "type": "object",
                    "description": "Optional key-value parameters/inputs required by the SOP steps (e.g. {'feature_request': 'implement a new database method'}).",
                    "additionalProperties": true
                },
                "wait": {
                    "type": "boolean",
                    "description": "Whether to wait synchronously for SOP execution to complete before returning (default: true). Set to false to trigger in background."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("trigger");

        match action {
            "trigger" => {
                let sop_id = arguments
                    .get("sop_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'sop_id' parameter for action 'trigger'"))?;

                let payload = arguments.get("payload").cloned().unwrap_or(json!({}));
                let wait = arguments.get("wait").and_then(|v| v.as_bool()).unwrap_or(true);

                let instance_id = crate::sop::engine::trigger_sop_with_options(
                    self.config.clone(),
                    sop_id.to_string(),
                    payload,
                    wait,
                )
                .await?;

                if wait {
                    if let Ok(inst) = crate::sop::load_instance(&instance_id) {
                        return Ok(json!({
                            "status": "success",
                            "sop_id": sop_id,
                            "instance_id": instance_id,
                            "sop_status": format!("{:?}", inst.status),
                            "current_step_index": inst.current_step_index,
                            "steps_total": inst.steps.len(),
                            "completed_at": inst.completed_at,
                            "message": format!("SOP loop '{}' finished execution with status {:?}", sop_id, inst.status),
                            "steps": inst.steps
                        }));
                    }
                }

                Ok(json!({
                    "status": "success",
                    "sop_id": sop_id,
                    "instance_id": instance_id,
                    "message": format!("SOP loop '{}' triggered. Instance ID: {}", sop_id, instance_id)
                }))
            }
            "status" => {
                let instance_id = arguments
                    .get("instance_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'instance_id' parameter for action 'status'"))?;

                let inst = crate::sop::load_instance(instance_id)?;
                Ok(json!({
                    "status": "success",
                    "instance": inst
                }))
            }
            "output" => {
                let instance_id = arguments
                    .get("instance_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'instance_id' parameter for action 'output'"))?;

                let inst = crate::sop::load_instance(instance_id)?;
                let step_outputs: Vec<Value> = inst
                    .steps
                    .iter()
                    .map(|s| {
                        json!({
                            "name": s.name,
                            "status": s.status,
                            "output": s.output,
                            "error": s.error,
                            "started_at": s.started_at,
                            "completed_at": s.completed_at
                        })
                    })
                    .collect();

                Ok(json!({
                    "status": "success",
                    "instance_id": instance_id,
                    "sop_id": inst.sop_id,
                    "sop_status": format!("{:?}", inst.status),
                    "context": inst.context,
                    "step_outputs": step_outputs
                }))
            }
            "list" => {
                let defs = crate::sop::load_definitions().unwrap_or_default();
                let instances = crate::sop::list_instances().unwrap_or_default();
                let def_summaries: Vec<Value> = defs
                    .into_iter()
                    .map(|d| {
                        json!({
                            "id": d.id,
                            "name": d.name,
                            "description": d.description,
                            "step_count": d.steps.len()
                        })
                    })
                    .collect();
                let recent_instances: Vec<Value> = instances
                    .into_iter()
                    .take(10)
                    .map(|inst| {
                        json!({
                            "id": inst.id,
                            "sop_id": inst.sop_id,
                            "status": format!("{:?}", inst.status),
                            "started_at": inst.started_at,
                            "completed_at": inst.completed_at
                        })
                    })
                    .collect();

                Ok(json!({
                    "status": "success",
                    "available_definitions": def_summaries,
                    "recent_instances": recent_instances
                }))
            }
            "resume" => {
                let instance_id = arguments
                    .get("instance_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'instance_id' parameter for action 'resume'"))?;

                let wait = arguments.get("wait").and_then(|v| v.as_bool()).unwrap_or(true);

                crate::sop::engine::resume_sop_with_options(
                    self.config.clone(),
                    instance_id.to_string(),
                    wait,
                )
                .await?;

                if wait {
                    if let Ok(inst) = crate::sop::load_instance(instance_id) {
                        return Ok(json!({
                            "status": "success",
                            "instance_id": instance_id,
                            "sop_status": format!("{:?}", inst.status),
                            "message": format!("SOP instance '{}' resumed and completed with status {:?}", instance_id, inst.status),
                            "instance": inst
                        }));
                    }
                }

                Ok(json!({
                    "status": "success",
                    "instance_id": instance_id,
                    "message": format!("SOP instance '{}' resume initiated.", instance_id)
                }))
            }
            other => Err(anyhow!("Unknown action '{}'. Valid actions: 'trigger', 'status', 'output', 'list', 'resume'", other)),
        }
    }
}

#[cfg(test)]
#[path = "sop_tests.rs"]
mod tests;
