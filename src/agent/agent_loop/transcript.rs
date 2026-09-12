use crate::config::schema::Config;
use crate::providers::ToolCallRequest;
use crate::session::Message;

#[derive(Debug, Clone)]
pub struct ToolTranscriptResult {
    pub id: String,
    pub name: String,
    pub result: serde_json::Value,
}

pub(crate) fn append_auto_tool_result(
    tool_results: &mut Vec<ToolTranscriptResult>,
    assistant_tool_calls_json: &mut Vec<serde_json::Value>,
    call: &ToolCallRequest,
    result: serde_json::Value,
) {
    tool_results.push(ToolTranscriptResult {
        id: call.id.clone(),
        name: call.name.clone(),
        result,
    });

    assistant_tool_calls_json.push(serde_json::json!({
        "id": call.id,
        "type": "function",
        "function": {
            "name": call.name,
            "arguments": call.arguments.to_string()
        },
        "_openz_auto_tool": true
    }));
}

pub(crate) fn append_assistant_tool_calls(
    messages: &mut Vec<Message>,
    tool_calls_json: Vec<serde_json::Value>,
    reasoning: Option<&str>,
) {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "tool_calls".to_string(),
        serde_json::Value::Array(tool_calls_json),
    );
    if let Some(reasoning) = reasoning {
        extra.insert(
            "reasoning_content".to_string(),
            serde_json::Value::String(reasoning.to_string()),
        );
    }

    if let Some(last_msg) = messages.last_mut() {
        if last_msg.role == "assistant" {
            for (key, value) in extra {
                last_msg.extra.insert(key, value);
            }
            return;
        }
    }

    messages.push(Message {
        role: "assistant".to_string(),
        content: String::new(),
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra,
    });
}

pub(crate) async fn append_tool_results(
    messages: &mut Vec<Message>,
    config: &Config,
    tool_results: Vec<ToolTranscriptResult>,
) {
    for tool_result in tool_results {
        let mut extra = serde_json::Map::new();
        extra.insert(
            "tool_call_id".to_string(),
            serde_json::Value::String(tool_result.id),
        );
        extra.insert(
            "name".to_string(),
            serde_json::Value::String(tool_result.name.clone()),
        );

        let content_str = tool_result.result.to_string();
        let limit = config.agents.defaults.tool_output_limit.unwrap_or(4000);
        let is_retrieve = tool_result.name == "retrieve_original"
            || tool_result.name == "headroom/retrieve_original";
        let mut tool_output_metadata = None;
        let content = if content_str.len() > limit && !is_retrieve {
            let outputs_dir = crate::config::loader::runtime_data_dir().join("tool_outputs");
            let compressed = crate::agent::context_compactor::compress_tool_output(
                &tool_result.name,
                &content_str,
            );

            let saved_path = match tokio::fs::create_dir_all(&outputs_dir).await {
                Ok(()) => {
                    let file_name = tool_output_file_name(&tool_result.name);
                    let file_path = outputs_dir.join(file_name);
                    match tokio::fs::write(&file_path, &content_str).await {
                        Ok(()) => Some(file_path),
                        Err(e) => {
                            tracing::warn!(
                                "Failed to write tool output file '{}': {}",
                                file_path.display(),
                                e
                            );
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create tool outputs directory '{}': {}",
                        outputs_dir.display(),
                        e
                    );
                    None
                }
            };

            if let Some(file_path) = saved_path {
                let original_ref = format!("file://{}", file_path.to_string_lossy());
                tool_output_metadata = Some(serde_json::json!({
                    "truncated": true,
                    "original_ref": original_ref,
                    "original_path": file_path.to_string_lossy(),
                    "original_bytes": content_str.len(),
                    "compressed_bytes": compressed.len(),
                    "inline_limit": limit,
                    "retrieve_tool": "retrieve_original",
                }));
                format!(
                    "{}\n\n... [TRUNCATED - Full output saved for reference at {}] ...",
                    compressed, original_ref
                )
            } else {
                format!(
                    "{}\n\n... [TRUNCATED - Full output could not be saved to disk] ...",
                    compressed
                )
            }
        } else {
            content_str
        };

        if let Some(metadata) = tool_output_metadata {
            extra.insert("tool_output".to_string(), metadata);
        }

        messages.push(Message {
            role: "tool".to_string(),
            content,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            extra,
        });
    }
}

fn tool_output_file_name(tool_name: &str) -> String {
    let safe_name: String = tool_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let safe_name = safe_name.trim_matches('_');
    let safe_name = if safe_name.is_empty() {
        "tool"
    } else {
        safe_name
    };
    format!("output_{}_{}.json", safe_name, uuid::Uuid::new_v4())
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;

