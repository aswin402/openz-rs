use crate::config::schema::Config;
use crate::session::Message;

#[derive(Debug, Clone)]
pub struct ToolTranscriptResult {
    pub id: String,
    pub name: String,
    pub result: serde_json::Value,
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
        let content = if content_str.len() > limit && !is_retrieve {
            let outputs_dir = crate::config::resolve_path("~/.openz/tool_outputs");
            if let Err(e) = tokio::fs::create_dir_all(&outputs_dir).await {
                tracing::warn!(
                    "Failed to create tool outputs directory '{}': {}",
                    outputs_dir.display(),
                    e
                );
            }
            let file_name = format!("output_{}_{}.json", tool_result.name, uuid::Uuid::new_v4());
            let file_path = outputs_dir.join(file_name);
            if let Err(e) = tokio::fs::write(&file_path, &content_str).await {
                tracing::warn!(
                    "Failed to write tool output file '{}': {}",
                    file_path.display(),
                    e
                );
            }

            let compressed = crate::agent::context_compactor::compress_tool_output(
                &tool_result.name,
                &content_str,
            );
            format!(
                "{}\n\n... [TRUNCATED - Full output saved for reference at file://{}] ...",
                compressed,
                file_path.to_string_lossy()
            )
        } else {
            content_str
        };

        messages.push(Message {
            role: "tool".to_string(),
            content,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            extra,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_tool_calls_to_existing_assistant_message() {
        let mut messages = vec![Message {
            role: "assistant".to_string(),
            content: "thinking".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        }];

        append_assistant_tool_calls(
            &mut messages,
            vec![serde_json::json!({
                "id": "call_1",
                "type": "function",
                "function": { "name": "read_file", "arguments": "{\"path\":\"Cargo.toml\"}" }
            })],
            Some("reasoning"),
        );

        assert_eq!(messages.len(), 1);
        assert!(messages[0].extra.get("tool_calls").is_some());
        assert_eq!(messages[0].extra["reasoning_content"], "reasoning");
    }
}
