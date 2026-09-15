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
#[path = "streaming_tests.rs"]
mod tests;
