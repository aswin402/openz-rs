use anyhow::{anyhow, Result};
use serde_json::Value;

use super::evaluator_optimizer::validate_schema;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaRetryDecision {
    Accepted(String),
    Retry { prompt: String, reason: String },
}

pub fn strip_json_fences(text_output: &str) -> &str {
    let trimmed = text_output.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        stripped.strip_suffix("```").unwrap_or(stripped).trim()
    } else if let Some(stripped) = trimmed.strip_prefix("```") {
        stripped.strip_suffix("```").unwrap_or(stripped).trim()
    } else {
        trimmed
    }
}

pub fn evaluate_schema_retry(
    text_output: &str,
    schema: &Value,
    attempts: usize,
    max_attempts: usize,
) -> Result<SchemaRetryDecision> {
    let clean_json_str = strip_json_fences(text_output);
    let parsed_val: Value = match serde_json::from_str(clean_json_str) {
        Ok(value) => value,
        Err(e) => {
            if attempts >= max_attempts {
                return Err(anyhow!(
                    "Subagent output failed to parse as JSON: {}. Parse Error: {}",
                    e,
                    text_output.trim()
                ));
            }
            return Ok(SchemaRetryDecision::Retry {
                reason: format!("Parse Error: {e}"),
                prompt: format!(
                    "Your previous response was not valid JSON. Parse Error: {e}\n\n\
                     Please correct your response. Return ONLY the raw valid JSON matching the schema."
                ),
            });
        }
    };

    if let Err(e) = validate_schema(&parsed_val, schema) {
        if attempts >= max_attempts {
            return Err(anyhow!("Subagent output failed schema validation: {}", e));
        }
        return Ok(SchemaRetryDecision::Retry {
            reason: e.clone(),
            prompt: format!(
                "Your previous response did not conform to the JSON Schema. Validation Error: {e}\n\n\
                 Please correct your response. Return ONLY the raw valid JSON matching the schema."
            ),
        });
    }

    Ok(SchemaRetryDecision::Accepted(clean_json_str.to_string()))
}

/// Drive the schema-validation retry loop around a completed subagent run.
/// On `Accepted` the run's content is replaced in place with the cleaned
/// JSON; on `Retry` the corrected prompt is handed to `rerun` (which
/// re-executes the child agent); at the attempt limit the error propagates.
/// Max 2 correction attempts, matching the previous inline loops.
pub(crate) async fn execute_with_schema_retries<F, Fut>(
    mut run_res: Result<crate::agent::agent_loop::RunResult>,
    json_schema: &Value,
    mut rerun: F,
) -> Result<crate::agent::agent_loop::RunResult>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<crate::agent::agent_loop::RunResult>>,
{
    let mut attempts = 0;
    while run_res.is_ok() {
        match evaluate_schema_retry(
            run_res
                .as_ref()
                .map(|res| res.content.as_str())
                .unwrap_or_default(),
            json_schema,
            attempts,
            2,
        ) {
            Ok(SchemaRetryDecision::Accepted(clean_json)) => {
                if let Ok(ref mut res) = run_res {
                    res.content = clean_json;
                }
                break;
            }
            Ok(SchemaRetryDecision::Retry { prompt, reason }) => {
                attempts += 1;
                crate::tui_println!(
                    "{}▲ [Reflection] Subagent output needs correction: {}. Retrying attempt {} of 2...{}",
                    crate::agent::style::AURA_GOLD, reason, attempts, crate::agent::style::COLOR_RESET
                );
                run_res = rerun(prompt).await;
            }
            Err(e) => {
                run_res = Err(e);
                break;
            }
        }
    }
    run_res
}
