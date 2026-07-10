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
                format!(
                    "{}\n\n... [TRUNCATED - Full output saved for reference at file://{}] ...",
                    compressed,
                    file_path.to_string_lossy()
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn large_namespaced_tool_output_uses_runtime_dir_and_safe_filename() {
        let _lock = crate::tools::graph_memory::test_lock().lock().await;
        let temp_dir = std::env::temp_dir().join(format!(
            "openz_transcript_tool_outputs_{}",
            uuid::Uuid::new_v4()
        ));
        let config_dir = temp_dir.join("config");
        let home_dir = temp_dir.join("home");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&home_dir).unwrap();

        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &home_dir);

        let mut config = Config::default();
        config.agents.defaults.tool_output_limit = Some(10);
        let mut messages = Vec::new();

        crate::config::loader::CONFIG_DIR_OVERRIDE
            .scope(config_dir.clone(), async {
                append_tool_results(
                    &mut messages,
                    &config,
                    vec![ToolTranscriptResult {
                        id: "call_1".to_string(),
                        name: "mcp/server.tool".to_string(),
                        result: serde_json::json!({
                            "payload": "this output is intentionally long enough to be stored"
                        }),
                    }],
                )
                .await;
            })
            .await;

        if let Some(old_home) = old_home {
            std::env::set_var("HOME", old_home);
        } else {
            std::env::remove_var("HOME");
        }

        let output_dir = config_dir.join("tool_outputs");
        let entries = std::fs::read_dir(&output_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1, "expected one stored tool output file");
        assert!(
            entries[0].is_file(),
            "tool output path must be a file, not a nested path"
        );
        assert!(!entries[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains('/'));
        assert!(messages[0]
            .content
            .contains(&format!("file://{}", entries[0].to_string_lossy())));
        assert!(std::fs::read_dir(home_dir.join(".openz")).is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

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
