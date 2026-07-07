use super::delegate_profile::DelegateProfileTool;
use super::CancellationToken;
use crate::agent::style::*;
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::Arc;

pub struct EvaluatorOptimizerLoopTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
    pub cancellation_token: CancellationToken,
}

#[async_trait::async_trait]
impl Tool for EvaluatorOptimizerLoopTool {
    fn name(&self) -> &str {
        "evaluator_optimizer_loop"
    }

    fn description(&self) -> &str {
        "Run a stateful draft-and-review cycle (reflection loop) between an optimizer subagent (e.g. coding_agent) and an evaluator subagent (e.g. reviewer) to generate high-quality outputs."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "optimizer": {
                    "type": "string",
                    "description": "Name of the subagent to generate and refine the draft (e.g. 'coding_agent')."
                },
                "evaluator": {
                    "type": "string",
                    "description": "Name of the subagent to evaluate and review the draft (e.g. 'reviewer')."
                },
                "goal": {
                    "type": "string",
                    "description": "The specific goal or task to accomplish."
                },
                "context": {
                    "type": "string",
                    "description": "Additional context or background details required for the task."
                },
                "checklist": {
                    "type": "string",
                    "description": "Grading checklist or quality criteria for the evaluator to check against (e.g. 'Must include unit tests', 'No compilation warnings')."
                },
                "max_iterations": {
                    "type": "integer",
                    "description": "Maximum number of optimization iterations (default: 3)."
                }
            },
            "required": ["optimizer", "evaluator", "goal", "checklist"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        crate::agent::style::spinner::IS_SILENT.scope(crate::agent::style::is_silent(), async {
            let optimizer_name = arguments.get("optimizer").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'optimizer' argument"))?;
        let evaluator_name = arguments.get("evaluator").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'evaluator' argument"))?;
        let goal = arguments.get("goal").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'goal' argument"))?;
        let context = arguments.get("context").and_then(|v| v.as_str()).unwrap_or("");
        let checklist = arguments.get("checklist").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'checklist' argument"))?;
        let max_iterations = arguments.get("max_iterations").and_then(|v| v.as_i64()).unwrap_or(3) as usize;

        let profiles = crate::subagents::load_profiles()?;
        let optimizer_profile = profiles.iter().find(|p| p.name == optimizer_name)
            .ok_or_else(|| anyhow!("Optimizer subagent profile '{}' not found", optimizer_name))?;
        let evaluator_profile = profiles.iter().find(|p| p.name == evaluator_name)
            .ok_or_else(|| anyhow!("Evaluator subagent profile '{}' not found", evaluator_name))?;

        let optimizer_tool = DelegateProfileTool {
            config: self.config.clone(),
            parent_provider: self.parent_provider.clone(),
            session_manager: self.session_manager.clone(),
            profile: optimizer_profile.clone(),
            parent_tools: self.parent_tools.clone(),
            cancellation_token: self.cancellation_token.clone(),
        };

        let evaluator_tool = DelegateProfileTool {
            config: self.config.clone(),
            parent_provider: self.parent_provider.clone(),
            session_manager: self.session_manager.clone(),
            profile: evaluator_profile.clone(),
            parent_tools: self.parent_tools.clone(),
            cancellation_token: self.cancellation_token.clone(),
        };

        let mut optimizer_output = String::new();
        let mut feedback = String::new();
        let mut passed = false;
        let mut iterations_run = 0;

        let eval_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "passed": { "type": "boolean" },
                "feedback": {
                    "type": "string",
                    "description": "Detailed feedback describing precisely what checklist items failed, or empty if all passed."
                }
            },
            "required": ["passed", "feedback"]
        });

        for i in 1..=max_iterations {
            if self.cancellation_token.is_cancelled() {
                return Err(anyhow!("Evaluator-Optimizer loop cancelled"));
            }
            iterations_run = i;
            crate::tui_println!(
                "{}🔄 [Evaluator-Optimizer] Starting iteration {}/{} (Optimizer: '{}', Evaluator: '{}'){}",
                AURA_PURPLE, i, max_iterations, optimizer_name, evaluator_name, COLOR_RESET
            );

            // Invoke Optimizer
            let opt_goal = if i == 1 {
                goal.to_string()
            } else {
                format!(
                    "Your previous draft failed evaluation. Please refine it based on this feedback:\n\n\
                    FEEDBACK:\n{}\n\n\
                    CRITERIA CHECKLIST:\n{}\n\n\
                    ORIGINAL GOAL:\n{}",
                    feedback, checklist, goal
                )
            };

            let opt_context = if i == 1 {
                context.to_string()
            } else {
                format!(
                    "PREVIOUS DRAFT:\n{}\n\n\
                    {}",
                    optimizer_output, context
                )
            };

            let opt_res = optimizer_tool.call(&serde_json::json!({
                "goal": opt_goal,
                "context": opt_context
            })).await?;

            if opt_res.get("status").and_then(|v| v.as_str()) != Some("success") {
                let err = opt_res.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown optimizer error");
                return Err(anyhow!("Optimizer subagent '{}' failed: {}", optimizer_name, err));
            }

            optimizer_output = opt_res.get("summary").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Optimizer response missing summary"))?.to_string();

            // Invoke Evaluator
            let eval_goal = format!(
                "Review the draft produced by the optimizer against the following checklist criteria. Assess whether the draft passes all checklist items. If it fails any item, specify detailed feedback on how to fix it.\n\n\
                CHECKLIST CRITERIA:\n{}\n\n\
                OPTIMIZER DRAFT:\n{}",
                checklist, optimizer_output
            );

            let eval_context = format!(
                "Original task goal: {}\nOriginal context: {}",
                goal, context
            );

            let eval_res = evaluator_tool.call(&serde_json::json!({
                "goal": eval_goal,
                "context": eval_context,
                "json_schema": eval_schema
            })).await?;

            if eval_res.get("status").and_then(|v| v.as_str()) != Some("success") {
                let err = eval_res.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown evaluator error");
                return Err(anyhow!("Evaluator subagent '{}' failed: {}", evaluator_name, err));
            }

            let eval_summary = eval_res.get("summary").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Evaluator response missing summary"))?;

            let eval_json: Value = serde_json::from_str(eval_summary)
                .map_err(|e| anyhow!("Failed to parse evaluator JSON response ({}): {}", e, eval_summary))?;

            passed = eval_json.get("passed").and_then(|v| v.as_bool()).unwrap_or(false);
            feedback = eval_json.get("feedback").and_then(|v| v.as_str()).unwrap_or("").to_string();

            if passed {
                crate::tui_println!(
                    "{}✓ [Evaluator-Optimizer] Evaluation PASSED on iteration {}/{}!{}",
                    EMERALD_GREEN, i, max_iterations, COLOR_RESET
                );
                break;
            } else {
                crate::tui_println!(
                    "{}✕ [Evaluator-Optimizer] Evaluation FAILED on iteration {}/{}. Feedback: {}{}",
                    AURA_GOLD, i, max_iterations, feedback, COLOR_RESET
                );
            }
        }

        Ok(serde_json::json!({
            "status": if passed { "success" } else { "partial_success" },
            "iterations_run": iterations_run,
            "passed": passed,
            "final_output": optimizer_output,
            "final_feedback": feedback
        }))
        }).await
    }
}

pub fn validate_schema(value: &Value, schema: &Value) -> Result<(), String> {
    // 1. Get type field
    let type_str = schema
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Schema missing 'type' field".to_string())?;

    match type_str {
        "object" => {
            if !value.is_object() {
                return Err("Value is not an object".to_string());
            }
            let obj = value.as_object().unwrap();

            // Check required fields
            if let Some(req_val) = schema.get("required") {
                if let Some(req_arr) = req_val.as_array() {
                    for req_field in req_arr {
                        if let Some(field_name) = req_field.as_str() {
                            if !obj.contains_key(field_name) {
                                return Err(format!("Missing required field: '{}'", field_name));
                            }
                        }
                    }
                }
            }

            // Recursively validate properties
            if let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) {
                for (prop_name, prop_schema) in properties {
                    if let Some(prop_value) = obj.get(prop_name) {
                        validate_schema(prop_value, prop_schema)
                            .map_err(|e| format!("Property '{}' invalid: {}", prop_name, e))?;
                    }
                }
            }
        }
        "array" => {
            if !value.is_array() {
                return Err("Value is not an array".to_string());
            }
            let arr = value.as_array().unwrap();

            if let Some(items_schema) = schema.get("items") {
                for (idx, item) in arr.iter().enumerate() {
                    validate_schema(item, items_schema)
                        .map_err(|e| format!("Array item at index {} invalid: {}", idx, e))?;
                }
            }
        }
        "string" => {
            if !value.is_string() {
                return Err("Value is not a string".to_string());
            }
            // Check enum constraint
            if let Some(enum_val) = schema.get("enum").and_then(|v| v.as_array()) {
                let val_str = value.as_str().unwrap();
                let matches = enum_val
                    .iter()
                    .any(|allowed| allowed.as_str() == Some(val_str));
                if !matches {
                    return Err(format!(
                        "Value '{}' not in allowed enum: {:?}",
                        val_str,
                        enum_val
                            .iter()
                            .map(|v| v.as_str().unwrap_or(""))
                            .collect::<Vec<_>>()
                    ));
                }
            }
        }
        "integer" => {
            if !value.is_i64() && !value.is_u64() {
                return Err("Value is not an integer".to_string());
            }
        }
        "number" => {
            if !value.is_number() {
                return Err("Value is not a number".to_string());
            }
        }
        "boolean" => {
            if !value.is_boolean() {
                return Err("Value is not a boolean".to_string());
            }
        }
        _ => return Err(format!("Unsupported schema type: '{}'", type_str)),
    }

    Ok(())
}
