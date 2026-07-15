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
