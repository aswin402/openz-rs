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
                match id.filter(|s| !s.is_empty()) {
                    Some(id) => entry.id = id,
                    None => {}
                }
                match name.filter(|s| !s.is_empty()) {
                    Some(name) => entry.name = name,
                    None => {}
                }
                match arguments {
                    Some(arguments) => entry.arguments.push_str(&arguments),
                    None => {}
                }
            }
            ChatStreamChunk::Done { finish_reason } => match finish_reason {
                Some(reason) => self.finish_reason = reason,
                None => {}
            },
        }
    }

    pub fn into_response(self) -> LLMResponse {
        let mut keys: Vec<_> = self.partial_tool_calls.keys().copied().collect();
        keys.sort_unstable();

        let mut tool_calls = Vec::new();
        for key in keys {
            let partial = match self.partial_tool_calls.get(&key) {
                Some(partial) => partial,
                None => continue,
            };
            match partial.name.trim().is_empty() {
                true => continue,
                false => {}
            }
            let arguments = match partial.arguments.trim().is_empty() {
                true => serde_json::json!({}),
                false => serde_json::from_str(&partial.arguments).unwrap_or_else(|err| {
                    let repaired = partial.arguments.replace('\n', "\\n").replace('\r', "\\r");
                    serde_json::from_str(&repaired)
                        .unwrap_or_else(|_| serde_json::json!({ "parse_error": err.to_string() }))
                }),
            };

            let id = match partial.id.is_empty() {
                true => format!("call_{}", key),
                false => partial.id.clone(),
            };

            tool_calls.push(ToolCallRequest {
                id,
                name: partial.name.clone(),
                arguments,
            });
        }

        let mut content = (!self.content.is_empty()).then_some(self.content);

        match (tool_calls.is_empty(), content.as_deref()) {
            (true, Some(text)) => {
                let parsed = crate::providers::openai::parse_fallback_tool_calls(text);
                match parsed.is_empty() {
                    true => {}
                    false => {
                        tool_calls = parsed;
                        content = None;
                    }
                }
            }
            _ => {}
        }

        LLMResponse {
            content,
            tool_calls,
            finish_reason: self.finish_reason,
            reasoning_content: (!self.reasoning.is_empty()).then_some(self.reasoning),
        }
    }
}

#[cfg(test)]
#[path = "streaming_tests.rs"]
mod tests;
