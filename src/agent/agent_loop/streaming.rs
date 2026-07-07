use crate::providers::{ChatStreamChunk, LLMResponse, ToolCallRequest};

#[derive(Debug, Default)]
pub struct StreamingAssembly {
    content: String,
    reasoning: String,
    finish_reason: String,
    partial_tool_calls: std::collections::HashMap<usize, PartialToolCall>,
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl StreamingAssembly {
    pub fn new() -> Self {
        Self {
            finish_reason: "stop".to_string(),
            ..Self::default()
        }
    }

    pub fn push_chunk(&mut self, chunk: ChatStreamChunk) {
        match chunk {
            ChatStreamChunk::Content(text) => self.content.push_str(&text),
            ChatStreamChunk::Reasoning(text) => self.reasoning.push_str(&text),
            ChatStreamChunk::ToolCall {
                index,
                id,
                name,
                arguments,
            } => {
                let entry = self.partial_tool_calls.entry(index).or_default();
                if let Some(id) = id {
                    entry.id = id;
                }
                if let Some(name) = name {
                    entry.name = name;
                }
                if let Some(arguments) = arguments {
                    entry.arguments.push_str(&arguments);
                }
            }
            ChatStreamChunk::Done { finish_reason } => {
                if let Some(reason) = finish_reason {
                    self.finish_reason = reason;
                }
            }
        }
    }

    pub fn into_response(self) -> LLMResponse {
        let mut keys: Vec<_> = self.partial_tool_calls.keys().copied().collect();
        keys.sort_unstable();

        let mut tool_calls = Vec::new();
        for key in keys {
            if let Some(partial) = self.partial_tool_calls.get(&key) {
                let arguments = serde_json::from_str(&partial.arguments).unwrap_or_else(|err| {
                    let repaired = partial.arguments.replace('\n', "\\n").replace('\r', "\\r");
                    serde_json::from_str(&repaired)
                        .unwrap_or_else(|_| serde_json::json!({ "parse_error": err.to_string() }))
                });

                tool_calls.push(ToolCallRequest {
                    id: partial.id.clone(),
                    name: partial.name.clone(),
                    arguments,
                });
            }
        }

        LLMResponse {
            content: if self.content.is_empty() {
                None
            } else {
                Some(self.content)
            },
            tool_calls,
            finish_reason: self.finish_reason,
            reasoning_content: if self.reasoning.is_empty() {
                None
            } else {
                Some(self.reasoning)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_split_tool_call_arguments_in_index_order() {
        let mut assembly = StreamingAssembly::new();
        assembly.push_chunk(ChatStreamChunk::ToolCall {
            index: 1,
            id: Some("call_b".to_string()),
            name: Some("read_file".to_string()),
            arguments: Some("{\"path\":\"b".to_string()),
        });
        assembly.push_chunk(ChatStreamChunk::ToolCall {
            index: 1,
            id: None,
            name: None,
            arguments: Some(".rs\"}".to_string()),
        });
        assembly.push_chunk(ChatStreamChunk::ToolCall {
            index: 0,
            id: Some("call_a".to_string()),
            name: Some("list_dir".to_string()),
            arguments: Some("{\"path\":\".\"}".to_string()),
        });

        let response = assembly.into_response();
        assert_eq!(response.tool_calls.len(), 2);
        assert_eq!(response.tool_calls[0].id, "call_a");
        assert_eq!(response.tool_calls[1].id, "call_b");
        assert_eq!(response.tool_calls[1].arguments["path"], "b.rs");
    }
}
