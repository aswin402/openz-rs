use crate::providers::circuit_breaker::CircuitBreaker;
use crate::providers::transport::{
    anthropic_messages_endpoint, build_provider_client, post_json_with_retry, ProviderAuth,
};
use crate::providers::{
    ChatStreamChunk, ContentPart, GenerationSettings, LLMProvider, LLMResponse, ToolCallRequest,
};
use crate::session::Message;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct AnthropicProvider {
    pub client: Client,
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub breaker: CircuitBreaker,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    system: serde_json::Value,
    messages: Vec<AnthropicMessage>,
    max_tokens: usize,
    temperature: f32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(other)]
    Other,
}

fn content_part_to_anthropic_block(
    part: ContentPart,
    supports_vision: bool,
) -> Option<serde_json::Value> {
    match (part, supports_vision) {
        (ContentPart::Text(t), _) => Some(serde_json::json!({
            "type": "text",
            "text": t
        })),
        (
            ContentPart::Image {
                mime_type,
                base64_data,
            },
            true,
        ) => Some(serde_json::json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": mime_type,
                "data": base64_data
            }
        })),
        (ContentPart::Image { .. }, false) => None,
    }
}

fn tool_call_value_to_anthropic_block(call: &serde_json::Value) -> Option<serde_json::Value> {
    let call_obj = call.as_object()?;
    let id = call_obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let func = call_obj.get("function").and_then(|v| v.as_object());
    let name = func
        .and_then(|f| f.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let args_str = func
        .and_then(|f| f.get("arguments"))
        .and_then(|v| v.as_str())
        .unwrap_or("{}");
    let args: serde_json::Value = serde_json::from_str(args_str).unwrap_or(serde_json::Value::Null);
    Some(serde_json::json!({
        "type": "tool_use",
        "id": id,
        "name": name,
        "input": args
    }))
}

async fn build_anthropic_message_content(msg: &Message, model: &str) -> serde_json::Value {
    match msg.role.as_str() {
        "tool" => {
            let tool_call_id = msg
                .extra
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!([
                {
                    "type": "tool_result",
                    "tool_use_id": tool_call_id,
                    "content": msg.content
                }
            ])
        }
        _ => match msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
            Some(tool_calls) => {
                let mut blocks = Vec::new();
                match msg.content.is_empty() {
                    true => {}
                    false => {
                        let parts = crate::providers::parse_multimodal_content(&msg.content).await;
                        let supports_vision = crate::providers::model_supports_vision(model);
                        for part in parts {
                            match content_part_to_anthropic_block(part, supports_vision) {
                                Some(block) => blocks.push(block),
                                None => {}
                            }
                        }
                    }
                }
                for call in tool_calls {
                    match tool_call_value_to_anthropic_block(call) {
                        Some(block) => blocks.push(block),
                        None => {}
                    }
                }
                serde_json::Value::Array(blocks)
            }
            None => {
                let parts = crate::providers::parse_multimodal_content(&msg.content).await;
                let has_images = parts
                    .iter()
                    .any(|p| matches!(p, ContentPart::Image { .. }));
                let supports_vision = crate::providers::model_supports_vision(model);

                match (!supports_vision || !has_images, parts.as_slice()) {
                    (true, _) => serde_json::Value::String(msg.content.clone()),
                    (false, [ContentPart::Text(t)]) => serde_json::Value::String(t.clone()),
                    (
                        false,
                        [ContentPart::Image {
                            mime_type,
                            base64_data,
                        }],
                    ) => serde_json::json!([
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": mime_type,
                                "data": base64_data
                            }
                        }
                    ]),
                    (false, _) => {
                        let arr: Vec<serde_json::Value> = parts
                            .into_iter()
                            .filter_map(|part| content_part_to_anthropic_block(part, true))
                            .collect();
                        serde_json::Value::Array(arr)
                    }
                }
            }
        },
    }
}

fn convert_anthropic_tools(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .filter_map(|tool| {
            let f = tool.as_object()?.get("function")?.as_object()?;
            let name = f.get("name").cloned().unwrap_or(serde_json::Value::Null);
            let desc = f
                .get("description")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let schema = f
                .get("parameters")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Some(serde_json::json!({
                "name": name,
                "description": desc,
                "input_schema": schema
            }))
        })
        .collect()
}

impl AnthropicProvider {
    pub fn new(api_key: String, api_base: String, model: String) -> Self {
        AnthropicProvider {
            client: build_provider_client(),
            api_key,
            api_base,
            model,
            breaker: CircuitBreaker::new(5, Duration::from_secs(30)),
        }
    }

    async fn send_request(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
        stream: Option<bool>,
    ) -> Result<reqwest::Response> {
        let mut api_messages = Vec::new();
        let sanitized_messages = crate::providers::sanitize_tool_messages(messages);

        for msg in sanitized_messages {
            let role = match msg.role.as_str() {
                "assistant" => "assistant",
                _ => "user",
            };
            let content = build_anthropic_message_content(&msg, &self.model).await;
            api_messages.push(AnthropicMessage {
                role: role.to_string(),
                content,
            });
        }

        let anthropic_tools = convert_anthropic_tools(tools);

        let system_val = serde_json::json!([
            {
                "type": "text",
                "text": system_prompt,
                "cache_control": { "type": "ephemeral" }
            }
        ]);

        let body = AnthropicRequest {
            model: self.model.clone(),
            system: system_val,
            messages: api_messages,
            max_tokens: settings.max_tokens,
            temperature: settings.temperature,
            tools: anthropic_tools,
            stream,
        };

        let url = anthropic_messages_endpoint(&self.api_base);
        let body_for_retry =
            serde_json::to_value(&body).map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;
        post_json_with_retry(
            &self.client,
            &self.breaker,
            "anthropic",
            &url,
            &self.api_key,
            body_for_retry,
            ProviderAuth::Anthropic,
        )
        .await
    }
}

fn map_anthropic_stop_reason(reason: &str) -> String {
    match reason {
        "tool_use" => "tool_calls".to_string(),
        "max_tokens" => "length".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn parse_anthropic_sse_data(data: &str) -> Result<(Vec<ChatStreamChunk>, bool)> {
    let trimmed = data.trim();
    match trimmed.is_empty() || trimmed == "[DONE]" {
        true => return Ok((Vec::new(), trimmed == "[DONE]")),
        false => {}
    }
    let val: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Ok((Vec::new(), false)),
    };
    let event_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let mut chunks = Vec::new();
    match event_type {
        "error" => {
            let msg = val
                .pointer("/error/message")
                .and_then(|v| v.as_str())
                .unwrap_or("Anthropic stream error");
            return Err(anyhow::anyhow!("{msg}"));
        }
        "content_block_start" => {
            let index = val
                .get("index")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(0);
            let block_type = val
                .pointer("/content_block/type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match block_type {
                "tool_use" => {
                    let id = val
                        .pointer("/content_block/id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    let name = val
                        .pointer("/content_block/name")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    chunks.push(ChatStreamChunk::ToolCall {
                        index,
                        id,
                        name,
                        arguments: None,
                    });
                }
                "thinking" => {
                    let _ = val
                        .pointer("/content_block/thinking")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|t| chunks.push(ChatStreamChunk::Reasoning(t.to_string())));
                }
                "text" => {
                    let _ = val
                        .pointer("/content_block/text")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|t| chunks.push(ChatStreamChunk::Content(t.to_string())));
                }
                _ => {}
            }
        }
        "content_block_delta" => {
            let index = val
                .get("index")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(0);
            let delta_type = val
                .pointer("/delta/type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match delta_type {
                "text_delta" => {
                    let _ = val
                        .pointer("/delta/text")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|t| chunks.push(ChatStreamChunk::Content(t.to_string())));
                }
                "thinking_delta" => {
                    let _ = val
                        .pointer("/delta/thinking")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|t| chunks.push(ChatStreamChunk::Reasoning(t.to_string())));
                }
                "input_json_delta" => {
                    let _ = val
                        .pointer("/delta/partial_json")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|args| {
                            chunks.push(ChatStreamChunk::ToolCall {
                                index,
                                id: None,
                                name: None,
                                arguments: Some(args.to_string()),
                            })
                        });
                }
                _ => {}
            }
        }
        "message_delta" => {
            let _ = val
                .pointer("/delta/stop_reason")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|reason| {
                    chunks.push(ChatStreamChunk::Done {
                        finish_reason: Some(map_anthropic_stop_reason(reason)),
                    });
                });
        }
        "message_stop" => return Ok((chunks, true)),
        _ => {}
    }
    Ok((chunks, false))
}

#[async_trait::async_trait]
impl LLMProvider for AnthropicProvider {
    async fn chat(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
    ) -> Result<LLMResponse> {
        let response = self
            .send_request(system_prompt, messages, tools, settings, None)
            .await?;

        let anthropic_resp: AnthropicResponse = response.json().await?;

        let mut text_content = String::new();
        let mut reasoning_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in anthropic_resp.content {
            match block {
                ContentBlock::Text { text } => {
                    text_content.push_str(&text);
                }
                ContentBlock::Thinking { thinking } => {
                    let trimmed = thinking.trim();
                    match trimmed.is_empty() {
                        true => {}
                        false => reasoning_parts.push(trimmed.to_string()),
                    }
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCallRequest {
                        id,
                        name,
                        arguments: input,
                    });
                }
                ContentBlock::Other => {}
            }
        }

        let finish_reason = match tool_calls.is_empty() {
            false => "tool_calls".to_string(),
            true => anthropic_resp
                .stop_reason
                .as_deref()
                .map(map_anthropic_stop_reason)
                .unwrap_or_else(|| "stop".to_string()),
        };

        let content = (!text_content.is_empty()).then_some(text_content);
        let reasoning_content =
            (!reasoning_parts.is_empty()).then(|| reasoning_parts.join("\n\n---\n\n"));

        Ok(LLMResponse {
            content,
            tool_calls,
            finish_reason,
            reasoning_content,
        })
    }

    async fn chat_stream(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
    ) -> Result<
        std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<ChatStreamChunk>> + Send>>,
    > {
        let res = self
            .send_request(system_prompt, messages, tools, settings, Some(true))
            .await?;

        let byte_stream = res.bytes_stream();
        use futures_util::StreamExt;

        let stream = futures_util::stream::unfold(
            (
                byte_stream,
                Vec::<u8>::new(),
                std::collections::VecDeque::<Result<ChatStreamChunk>>::new(),
                false,
            ),
            |(mut stream, mut buffer, mut pending, mut done)| async move {
                loop {
                    match pending.pop_front() {
                        Some(item) => return Some((item, (stream, buffer, pending, done))),
                        None => {}
                    }
                    match done {
                        true => return None,
                        false => {}
                    }

                    while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                        let line_bytes: Vec<u8> = buffer.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&line_bytes).trim().to_string();
                        match line.strip_prefix("data:") {
                            Some(stripped) => match parse_anthropic_sse_data(stripped) {
                                Ok((chunks, is_done)) => {
                                    for c in chunks {
                                        pending.push_back(Ok(c));
                                    }
                                    match is_done {
                                        true => {
                                            done = true;
                                            break;
                                        }
                                        false => {}
                                    }
                                    match pending.is_empty() {
                                        false => break,
                                        true => {}
                                    }
                                }
                                Err(err) => {
                                    done = true;
                                    pending.push_back(Err(err));
                                    break;
                                }
                            },
                            None => {}
                        }
                    }

                    match pending.pop_front() {
                        Some(item) => return Some((item, (stream, buffer, pending, done))),
                        None => {}
                    }
                    match done {
                        true => return None,
                        false => {}
                    }

                    match stream.next().await {
                        Some(Ok(bytes)) => {
                            buffer.extend_from_slice(&bytes);
                        }
                        Some(Err(e)) => {
                            done = true;
                            return Some((
                                Err(anyhow::anyhow!("Stream error: {e}")),
                                (stream, buffer, pending, done),
                            ));
                        }
                        None => {
                            let remaining_text = String::from_utf8_lossy(&buffer).to_string();
                            buffer.clear();
                            for raw_line in remaining_text.lines() {
                                match raw_line.trim().strip_prefix("data:") {
                                    Some(stripped) => match parse_anthropic_sse_data(stripped) {
                                        Ok((chunks, _)) => {
                                            for c in chunks {
                                                pending.push_back(Ok(c));
                                            }
                                        }
                                        Err(err) => pending.push_back(Err(err)),
                                    },
                                    None => {}
                                }
                            }
                            let has_done_or_err = pending.iter().any(|item| match item {
                                Ok(ChatStreamChunk::Done { .. }) | Err(_) => true,
                                Ok(_) => false,
                            });
                            match has_done_or_err {
                                true => {}
                                false => pending.push_back(Ok(ChatStreamChunk::Done {
                                    finish_reason: None,
                                })),
                            }
                            done = true;
                            return pending
                                .pop_front()
                                .map(|item| (item, (stream, buffer, pending, done)));
                        }
                    }
                }
            },
        );

        let filtered = crate::providers::openai::filter_think_stream(Box::pin(stream));
        Ok(Box::pin(filtered))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_sse_parser_streams_thinking_text_and_tool_deltas() {
        let (thinking_chunks, done1) = parse_anthropic_sse_data(
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Checking files..."}}"#,
        )
        .unwrap();
        assert!(!done1);
        assert_eq!(thinking_chunks.len(), 1);
        match &thinking_chunks[0] {
            ChatStreamChunk::Reasoning(r) => assert_eq!(r, "Checking files..."),
            _ => panic!("expected Reasoning chunk"),
        }

        let (text_chunks, done2) = parse_anthropic_sse_data(
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Hello world"}}"#,
        )
        .unwrap();
        assert!(!done2);
        assert_eq!(text_chunks.len(), 1);
        match &text_chunks[0] {
            ChatStreamChunk::Content(c) => assert_eq!(c, "Hello world"),
            _ => panic!("expected Content chunk"),
        }

        let (tool_start, _) = parse_anthropic_sse_data(
            r#"{"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"toolu_01","name":"read_file"}}"#,
        )
        .unwrap();
        assert_eq!(tool_start.len(), 1);
        match &tool_start[0] {
            ChatStreamChunk::ToolCall {
                index: 2,
                id: Some(id),
                name: Some(name),
                arguments: None,
            } => {
                assert_eq!(id, "toolu_01");
                assert_eq!(name, "read_file");
            }
            _ => panic!("expected ToolCall start chunk"),
        }

        let (msg_delta, done3) = parse_anthropic_sse_data(
            r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#,
        )
        .unwrap();
        assert!(!done3);
        match &msg_delta[0] {
            ChatStreamChunk::Done {
                finish_reason: Some(reason),
            } => assert_eq!(reason, "tool_calls"),
            _ => panic!("expected Done chunk with normalized tool_calls reason"),
        }
    }
}
