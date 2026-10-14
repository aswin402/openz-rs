use crate::providers::circuit_breaker::CircuitBreaker;
use crate::providers::{
    ChatStreamChunk, GenerationSettings, LLMProvider, LLMResponse, ToolCallRequest,
};
use crate::providers::transport::{
    build_provider_client, openai_chat_endpoint, post_json_with_retry, ProviderAuth,
};
use crate::session::Message;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct OpenAIProvider {
    pub client: Client,
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub breaker: CircuitBreaker,
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Serialize)]
struct OpenAIMessage {
    role: String,
    content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    /// DeepSeek thinking mode: must be echoed back as a top-level field.
    /// Other providers (OpenRouter, etc.) receive it embedded in <think> tags instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    _call_type: String,
    function: OpenAIFunction,
}

#[derive(Deserialize)]
struct OpenAIFunction {
    name: String,
    arguments: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, api_base: String, model: String) -> Self {
        OpenAIProvider {
            client: build_provider_client(),
            api_key,
            api_base,
            model,
            breaker: CircuitBreaker::new(5, Duration::from_secs(30)),
        }
    }

    async fn serialize_messages(
        model: &str,
        api_base: &str,
        system_prompt: &str,
        messages: &[Message],
    ) -> Vec<OpenAIMessage> {
        let mut api_messages = Vec::new();

        // DeepSeek's thinking API requires reasoning_content as a separate top-level
        // field in assistant messages. Other OpenAI-compatible providers (OpenRouter,
        // Groq, etc.) don't accept that field, so we fall back to <think> tags for them.
        let is_deepseek = api_base.contains("deepseek.com") || model.starts_with("deepseek");

        let sanitized_messages = crate::providers::sanitize_tool_messages(messages);

        match system_prompt.is_empty() {
            true => {}
            false => api_messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: serde_json::Value::String(system_prompt.to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }),
        }

        let supports_vision = crate::providers::model_supports_vision(model);

        for msg in sanitized_messages {
            let role = msg.role.clone();
            let tool_call_id = (role == "tool")
                .then(|| {
                    msg.extra
                        .get("tool_call_id")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .flatten();

            let name = msg
                .extra
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string);

            // Handle reasoning_content for assistant messages.
            // DeepSeek requires it as a dedicated top-level field; other providers
            // (OpenRouter, Groq, etc.) get it embedded as <think> tags in content.
            let non_empty_reasoning = (role == "assistant")
                .then(|| msg.extra.get("reasoning_content").and_then(|v| v.as_str()))
                .flatten()
                .filter(|r| !r.is_empty());

            let (reasoning_field, text_content) = match (non_empty_reasoning, is_deepseek) {
                (Some(reasoning), true) => (Some(reasoning.to_string()), msg.content.clone()),
                (Some(reasoning), false) => (
                    None,
                    format!("<think>\n{}\n</think>\n\n{}", reasoning, msg.content),
                ),
                (None, _) => (None, msg.content.clone()),
            };

            let parts = match (role.as_str(), supports_vision) {
                ("user", true) => crate::providers::parse_multimodal_content(&text_content).await,
                _ => Vec::new(),
            };
            let has_images = parts
                .iter()
                .any(|p| matches!(p, crate::providers::ContentPart::Image { .. }));

            let content_value = match (supports_vision && has_images, parts.as_slice()) {
                (false, _) => serde_json::Value::String(text_content.clone()),
                (true, [crate::providers::ContentPart::Text(t)]) => {
                    serde_json::Value::String(t.clone())
                }
                (
                    true,
                    [crate::providers::ContentPart::Image {
                        mime_type,
                        base64_data,
                    }],
                ) => serde_json::json!([
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:{};base64,{}", mime_type, base64_data)
                        }
                    }
                ]),
                (true, many) => {
                    let arr: Vec<serde_json::Value> = many
                        .iter()
                        .map(|part| match part {
                            crate::providers::ContentPart::Text(t) => serde_json::json!({
                                "type": "text",
                                "text": t
                            }),
                            crate::providers::ContentPart::Image {
                                mime_type,
                                base64_data,
                            } => serde_json::json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{};base64,{}", mime_type, base64_data)
                                }
                            }),
                        })
                        .collect();
                    serde_json::Value::Array(arr)
                }
            };

            api_messages.push(OpenAIMessage {
                role,
                content: content_value,
                name,
                tool_calls: msg
                    .extra
                    .get("tool_calls")
                    .cloned()
                    .and_then(|v| v.as_array().cloned()),
                tool_call_id,
                reasoning_content: reasoning_field,
            });
        }
        api_messages
    }
}

#[async_trait::async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
    ) -> Result<LLMResponse> {
        let api_messages =
            Self::serialize_messages(&self.model, &self.api_base, system_prompt, messages).await;

        let is_reasoning = is_reasoning_model(&self.model);
        let body = OpenAIRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: if is_reasoning {
                None
            } else {
                Some(settings.temperature)
            },
            max_tokens: if is_reasoning {
                None
            } else {
                Some(settings.max_tokens)
            },
            max_completion_tokens: if is_reasoning {
                Some(settings.max_tokens)
            } else {
                None
            },
            tools: tools.to_vec(),
            stream: None,
        };

        let url = openai_chat_endpoint(&self.api_base);
        let body_for_retry =
            serde_json::to_value(&body).map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;
        let auth = if self.api_base.contains("/openai/deployments")
            || self.api_base.contains("azure")
        {
            ProviderAuth::Azure
        } else {
            ProviderAuth::Bearer
        };
        let response = post_json_with_retry(
            &self.client,
            &self.breaker,
            "openai",
            &url,
            &self.api_key,
            body_for_retry,
            auth,
        )
        .await?;

        let response: OpenAIResponse = response.json().await?;
        let choice = response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("openai provider error: No choices returned"))?;

        let mut tool_calls: Vec<ToolCallRequest> = choice.message.tool_calls.as_ref().map(|calls| {
            calls.iter().map(|call| {
                let args_str = &call.function.arguments;
                let args_parsed = match serde_json::from_str(args_str) {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        tracing::warn!("Failed to parse native tool call arguments JSON: {}", e);
                        let repaired = args_str.replace("\n", "\\n").replace("\r", "\\r");
                        serde_json::from_str(&repaired).unwrap_or_else(|_| {
                            let mut map = serde_json::Map::new();
                            map.insert(
                                "parse_error".to_string(),
                                serde_json::Value::String(format!(
                                    "Tool arguments JSON was truncated or malformed: {}. This typically occurs when the response exceeds the maximum output token limit. Try writing files in smaller chunks or using command-line tools.",
                                    e
                                ))
                            );
                            serde_json::Value::Object(map)
                        })
                    }
                };
                ToolCallRequest {
                    id: call.id.clone(),
                    name: call.function.name.clone(),
                    arguments: args_parsed,
                }
            }).collect()
        }).unwrap_or_default();

        let mut content = choice.message.content.clone();
        let mut reasoning_content = choice.message.reasoning_content.clone();
        if let Some(text) = content.take() {
            let (clean_content, extracted_reasoning) = split_think_blocks(&text);
            content = clean_content;
            reasoning_content = merge_reasoning(reasoning_content, extracted_reasoning);
        }

        if tool_calls.is_empty() {
            if let Some(ref text) = content {
                let parsed = parse_fallback_tool_calls(text);
                if !parsed.is_empty() {
                    tool_calls = parsed;
                    content = None;
                }
            }
        }

        Ok(LLMResponse {
            content,
            tool_calls,
            finish_reason: choice
                .finish_reason
                .clone()
                .unwrap_or_else(|| "stop".to_string()),
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
        std::pin::Pin<
            Box<dyn futures_util::Stream<Item = Result<crate::providers::ChatStreamChunk>> + Send>,
        >,
    > {
        let api_messages =
            Self::serialize_messages(&self.model, &self.api_base, system_prompt, messages).await;

        let is_reasoning = is_reasoning_model(&self.model);
        let body = OpenAIRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: if is_reasoning {
                None
            } else {
                Some(settings.temperature)
            },
            max_tokens: if is_reasoning {
                None
            } else {
                Some(settings.max_tokens)
            },
            max_completion_tokens: if is_reasoning {
                Some(settings.max_tokens)
            } else {
                None
            },
            tools: tools.to_vec(),
            stream: Some(true),
        };

        let url = openai_chat_endpoint(&self.api_base);
        let body_for_retry =
            serde_json::to_value(&body).map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;
        let auth = if self.api_base.contains("/openai/deployments")
            || self.api_base.contains("azure")
        {
            ProviderAuth::Azure
        } else {
            ProviderAuth::Bearer
        };
        let res = post_json_with_retry(
            &self.client,
            &self.breaker,
            "openai",
            &url,
            &self.api_key,
            body_for_retry,
            auth,
        )
        .await?;

        let byte_stream = res.bytes_stream();
        use futures_util::StreamExt;

        let stream = futures_util::stream::unfold(
            (
                byte_stream,
                String::new(),
                std::collections::VecDeque::<crate::providers::ChatStreamChunk>::new(),
                false,
            ),
            |(mut stream, mut buffer, mut pending, mut done)| async move {
                loop {
                    if let Some(chunk) = pending.pop_front() {
                        return Some((Ok(chunk), (stream, buffer, pending, done)));
                    }
                    if done {
                        return None;
                    }

                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].trim().to_string();
                        buffer = buffer[pos + 1..].to_string();
                        if let Some(stripped) = line.strip_prefix("data:") {
                            let (chunks, is_done) = parse_sse_data_chunks(stripped);
                            for c in chunks {
                                pending.push_back(c);
                            }
                            if is_done {
                                done = true;
                                break;
                            }
                            if !pending.is_empty() {
                                break;
                            }
                        }
                    }

                    if let Some(chunk) = pending.pop_front() {
                        return Some((Ok(chunk), (stream, buffer, pending, done)));
                    }
                    if done {
                        return None;
                    }

                    match stream.next().await {
                        Some(Ok(bytes)) => {
                            buffer.push_str(&String::from_utf8_lossy(&bytes));
                        }
                        Some(Err(e)) => {
                            done = true;
                            return Some((
                                Err(anyhow::anyhow!("Stream error: {e}")),
                                (stream, buffer, pending, done),
                            ));
                        }
                        None => {
                            if !buffer.trim().is_empty() {
                                let remaining = std::mem::take(&mut buffer);
                                for raw_line in remaining.lines() {
                                    let line = raw_line.trim();
                                    if let Some(stripped) = line.strip_prefix("data:") {
                                        let (chunks, _) = parse_sse_data_chunks(stripped);
                                        for c in chunks {
                                            pending.push_back(c);
                                        }
                                    }
                                }
                            }
                            let has_done = pending.iter().any(|c| {
                                matches!(c, crate::providers::ChatStreamChunk::Done { .. })
                            });
                            if !has_done {
                                pending.push_back(crate::providers::ChatStreamChunk::Done {
                                    finish_reason: None,
                                });
                            }
                            done = true;
                            if let Some(chunk) = pending.pop_front() {
                                return Some((Ok(chunk), (stream, buffer, pending, done)));
                            }
                            return None;
                        }
                    }
                }
            },
        );

        let filtered = filter_think_stream(Box::pin(stream));
        Ok(Box::pin(filtered))
    }
}

pub(crate) fn parse_sse_data_chunks(
    data: &str,
) -> (Vec<crate::providers::ChatStreamChunk>, bool) {
    let data = data.trim();
    if data == "[DONE]" {
        return (
            vec![crate::providers::ChatStreamChunk::Done {
                finish_reason: None,
            }],
            true,
        );
    }

    let mut chunks = Vec::new();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
        if let Some(choices) = val.get("choices").and_then(|v| v.as_array()) {
            if let Some(choice) = choices.first() {
                if let Some(reasoning) = choice
                    .pointer("/delta/reasoning_content")
                    .or_else(|| choice.pointer("/delta/reasoning"))
                    .and_then(|v| v.as_str())
                {
                    if !reasoning.is_empty() {
                        chunks.push(crate::providers::ChatStreamChunk::Reasoning(
                            reasoning.to_string(),
                        ));
                    }
                }
                if let Some(content) = choice.pointer("/delta/content").and_then(|v| v.as_str()) {
                    if !content.is_empty() {
                        chunks.push(crate::providers::ChatStreamChunk::Content(
                            content.to_string(),
                        ));
                    }
                }
                if let Some(tool_calls) = choice
                    .pointer("/delta/tool_calls")
                    .and_then(|v| v.as_array())
                {
                    for (pos, tc) in tool_calls.iter().enumerate() {
                        let index = tc
                            .get("index")
                            .and_then(|v| v.as_u64())
                            .map(|v| v as usize)
                            .unwrap_or(pos);
                        let id = tc
                            .get("id")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string());
                        let name = tc
                            .pointer("/function/name")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string());
                        let arguments = tc
                            .pointer("/function/arguments")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string());
                        if id.is_some() || name.is_some() || arguments.is_some() {
                            chunks.push(crate::providers::ChatStreamChunk::ToolCall {
                                index,
                                id,
                                name,
                                arguments,
                            });
                        }
                    }
                }
                if let Some(finish_reason) = choice
                    .get("finish_reason")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    chunks.push(crate::providers::ChatStreamChunk::Done {
                        finish_reason: Some(finish_reason.to_string()),
                    });
                }
            }
        }
    }

    (chunks, false)
}

pub fn split_think_blocks(content: &str) -> (Option<String>, Option<String>) {
    if !content.contains("<think>") {
        return (
            if content.trim().is_empty() {
                None
            } else {
                Some(content.to_string())
            },
            None,
        );
    }

    let mut visible = String::new();
    let mut reasoning_parts: Vec<String> = Vec::new();
    let mut rest = content;

    loop {
        let Some(start) = rest.find("<think>") else {
            visible.push_str(rest);
            break;
        };
        visible.push_str(&rest[..start]);
        let after_start = &rest[start + "<think>".len()..];
        let Some(end) = after_start.find("</think>") else {
            visible.push_str(rest);
            break;
        };
        let reasoning = after_start[..end].trim();
        if !reasoning.is_empty() {
            reasoning_parts.push(reasoning.to_string());
        }
        rest = &after_start[end + "</think>".len()..];
    }

    let mut normalized_visible = String::new();
    let mut last_was_horizontal_space = false;
    for ch in visible.trim().chars() {
        if ch == ' ' || ch == '\t' {
            if !last_was_horizontal_space {
                normalized_visible.push(' ');
            }
            last_was_horizontal_space = true;
        } else {
            normalized_visible.push(ch);
            last_was_horizontal_space = false;
        }
    }
    let visible = normalized_visible;
    let reasoning = reasoning_parts.join("\n\n---\n\n");

    let visible = if visible.is_empty() {
        None
    } else {
        Some(visible)
    };
    let reasoning = if reasoning.trim().is_empty() {
        None
    } else {
        Some(reasoning)
    };
    (visible, reasoning)
}

#[derive(Debug, Default, Clone)]
pub struct ThinkStreamFilter {
    pub in_think: bool,
    pub buffer: String,
    pub skip_leading_newlines: bool,
}

impl ThinkStreamFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process_chunk(&mut self, chunk: ChatStreamChunk) -> Vec<ChatStreamChunk> {
        match chunk {
            ChatStreamChunk::Reasoning(r) => {
                let mut out = self.flush_buffer();
                out.push(ChatStreamChunk::Reasoning(r));
                out
            }
            ChatStreamChunk::ToolCall { .. } | ChatStreamChunk::Done { .. } => {
                let mut out = self.flush_buffer();
                out.push(chunk);
                out
            }
            ChatStreamChunk::Content(text) => {
                self.buffer.push_str(&text);
                let mut out = Vec::new();
                loop {
                    if self.buffer.is_empty() {
                        break;
                    }
                    if !self.in_think {
                        // Looking for <think>
                        if let Some(start_idx) = self.buffer.find("<think>") {
                            let mut before = self.buffer[..start_idx].to_string();
                            if self.skip_leading_newlines {
                                while before.starts_with('\n') || before.starts_with('\r') {
                                    before.remove(0);
                                }
                                self.skip_leading_newlines = false;
                            }
                            if !before.is_empty() {
                                out.push(ChatStreamChunk::Content(before));
                            }
                            self.buffer = self.buffer[start_idx + "<think>".len()..].to_string();
                            self.in_think = true;
                            continue;
                        }

                        // Check if buffer ends with a prefix of "<think>"
                        let tail_len = longest_suffix_matching_prefix(&self.buffer, "<think>");
                        if tail_len > 0 {
                            if self.buffer.len() > tail_len {
                                let mut safe =
                                    self.buffer[..self.buffer.len() - tail_len].to_string();
                                self.buffer =
                                    self.buffer[self.buffer.len() - tail_len..].to_string();
                                if self.skip_leading_newlines {
                                    while safe.starts_with('\n') || safe.starts_with('\r') {
                                        safe.remove(0);
                                    }
                                    if !safe.is_empty() {
                                        self.skip_leading_newlines = false;
                                    }
                                }
                                if !safe.is_empty() {
                                    out.push(ChatStreamChunk::Content(safe));
                                }
                            }
                            break;
                        } else {
                            let mut content = std::mem::take(&mut self.buffer);
                            if self.skip_leading_newlines {
                                while content.starts_with('\n') || content.starts_with('\r') {
                                    content.remove(0);
                                }
                                if !content.is_empty() {
                                    self.skip_leading_newlines = false;
                                }
                            }
                            if !content.is_empty() {
                                out.push(ChatStreamChunk::Content(content));
                            }
                            break;
                        }
                    } else {
                        // Inside think block, looking for </think>
                        if let Some(end_idx) = self.buffer.find("</think>") {
                            let reasoning = self.buffer[..end_idx].to_string();
                            if !reasoning.is_empty() {
                                out.push(ChatStreamChunk::Reasoning(reasoning));
                            }
                            self.buffer = self.buffer[end_idx + "</think>".len()..].to_string();
                            self.in_think = false;
                            self.skip_leading_newlines = true;
                            continue;
                        }

                        // Check if buffer ends with a prefix of "</think>"
                        let tail_len = longest_suffix_matching_prefix(&self.buffer, "</think>");
                        if tail_len > 0 {
                            if self.buffer.len() > tail_len {
                                let safe = self.buffer[..self.buffer.len() - tail_len].to_string();
                                self.buffer =
                                    self.buffer[self.buffer.len() - tail_len..].to_string();
                                if !safe.is_empty() {
                                    out.push(ChatStreamChunk::Reasoning(safe));
                                }
                            }
                            break;
                        } else {
                            let reasoning = std::mem::take(&mut self.buffer);
                            if !reasoning.is_empty() {
                                out.push(ChatStreamChunk::Reasoning(reasoning));
                            }
                            break;
                        }
                    }
                }
                out
            }
        }
    }

    pub fn flush_buffer(&mut self) -> Vec<ChatStreamChunk> {
        let mut out = Vec::new();
        if !self.buffer.is_empty() {
            let mut text = std::mem::take(&mut self.buffer);
            if self.in_think {
                if !text.is_empty() {
                    out.push(ChatStreamChunk::Reasoning(text));
                }
            } else {
                if self.skip_leading_newlines {
                    while text.starts_with('\n') || text.starts_with('\r') {
                        text.remove(0);
                    }
                    self.skip_leading_newlines = false;
                }
                if !text.is_empty() {
                    out.push(ChatStreamChunk::Content(text));
                }
            }
        }
        out
    }
}

fn longest_suffix_matching_prefix(buffer: &str, tag: &str) -> usize {
    let max_len = (tag.len() - 1).min(buffer.len());
    for len in (1..=max_len).rev() {
        if let Some(suffix) = buffer.get(buffer.len() - len..) {
            if tag.starts_with(suffix) {
                return len;
            }
        }
    }
    0
}

pub fn filter_think_stream<S>(
    stream: S,
) -> impl futures_util::Stream<Item = Result<ChatStreamChunk>> + Send
where
    S: futures_util::Stream<Item = Result<ChatStreamChunk>> + Send + Unpin + 'static,
{
    use futures_util::StreamExt;
    let initial_state = (
        stream,
        ThinkStreamFilter::new(),
        std::collections::VecDeque::<ChatStreamChunk>::new(),
        false,
    );

    futures_util::stream::unfold(
        initial_state,
        |(mut stream, mut filter, mut pending, mut done)| async move {
            if !pending.is_empty() {
                let chunk = pending.pop_front().unwrap();
                return Some((Ok(chunk), (stream, filter, pending, done)));
            }

            if done {
                return None;
            }

            loop {
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        let processed = filter.process_chunk(chunk);
                        for c in processed {
                            pending.push_back(c);
                        }
                        if let Some(first) = pending.pop_front() {
                            return Some((Ok(first), (stream, filter, pending, done)));
                        }
                    }
                    Some(Err(e)) => {
                        done = true;
                        return Some((Err(e), (stream, filter, pending, done)));
                    }
                    None => {
                        done = true;
                        let flushed = filter.flush_buffer();
                        for c in flushed {
                            pending.push_back(c);
                        }
                        if let Some(first) = pending.pop_front() {
                            return Some((Ok(first), (stream, filter, pending, done)));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

pub fn merge_reasoning(existing: Option<String>, extracted: Option<String>) -> Option<String> {
    match (existing, extracted) {
        (Some(a), Some(b)) if !a.trim().is_empty() && !b.trim().is_empty() => {
            Some(format!("{}\n\n---\n\n{}", a.trim(), b.trim()))
        }
        (Some(a), _) if !a.trim().is_empty() => Some(a),
        (_, Some(b)) if !b.trim().is_empty() => Some(b),
        _ => None,
    }
}

pub fn parse_fallback_tool_calls(content: &str) -> Vec<ToolCallRequest> {
    let mut tool_calls = Vec::new();

    // 1. Check delimited blocks: [TOOL_CALL]...[/TOOL_CALL], <tool_call>...</tool_call>, ```json...```, ```tool_call...```
    let block_delimiters = [
        ("[TOOL_CALL]", "[/TOOL_CALL]"),
        ("<tool_call>", "</tool_call>"),
        ("```json", "```"),
        ("```tool_call", "```"),
    ];

    for (start_tag, end_tag) in block_delimiters {
        let mut search_str = content;
        while let Some(start_idx) = search_str.find(start_tag) {
            let after_start = &search_str[start_idx + start_tag.len()..];
            if let Some(end_idx) = after_start.find(end_tag) {
                let raw_inner = after_start[..end_idx].trim();
                if let Some(val) = parse_lenient_tool_json(raw_inner) {
                    if let Some(tc) = extract_tool_call(&val) {
                        tool_calls.push(tc);
                    }
                }
                search_str = &after_start[end_idx + end_tag.len()..];
            } else {
                break;
            }
        }
    }

    // 2. If no delimited blocks found, check if the trimmed content (or balanced JSON object) is a raw tool call
    if tool_calls.is_empty() {
        let trimmed = content.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            if let Some(val) = parse_lenient_tool_json(trimmed) {
                if let Some(tc) = extract_tool_call(&val) {
                    tool_calls.push(tc);
                }
            }
        }
    }

    tool_calls
}

fn parse_lenient_tool_json(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Some(val);
    }
    let normalized = normalize_arrow_map_syntax(trimmed);
    serde_json::from_str::<serde_json::Value>(&normalized).ok()
}

/// Converts Ruby/MiniMax-style `{tool => "list_jobs", args => {}}` syntax into valid JSON `{"tool": "list_jobs", "args": {}}`.
fn normalize_arrow_map_syntax(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut in_double = false;
    let mut in_single = false;

    while i < chars.len() {
        let c = chars[i];
        if in_double {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                i += 1;
                out.push(chars[i]);
            } else if c == '"' {
                in_double = false;
            }
            i += 1;
            continue;
        }
        if in_single {
            if c == '\\' && i + 1 < chars.len() {
                i += 1;
                let escaped = chars[i];
                if escaped == '"' {
                    out.push('\\');
                }
                out.push(escaped);
            } else if c == '\'' {
                out.push('"');
                in_single = false;
            } else if c == '"' {
                out.push('\\');
                out.push('"');
            } else {
                out.push(c);
            }
            i += 1;
            continue;
        }

        if c == '"' {
            in_double = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '\'' {
            in_single = true;
            out.push('"');
            i += 1;
            continue;
        }

        if c == '=' && i + 1 < chars.len() && chars[i + 1] == '>' {
            out.push(':');
            i += 2;
            continue;
        }

        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '-')
            {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            let mut lookahead = i;
            while lookahead < chars.len() && chars[lookahead].is_whitespace() {
                lookahead += 1;
            }
            if lookahead + 1 < chars.len()
                && chars[lookahead] == '='
                && chars[lookahead + 1] == '>'
            {
                out.push('"');
                out.push_str(&ident);
                out.push_str("\":");
                i = lookahead + 2;
                continue;
            } else if lookahead < chars.len() && chars[lookahead] == ':' {
                out.push('"');
                out.push_str(&ident);
                out.push_str("\":");
                i = lookahead + 1;
                continue;
            } else {
                out.push_str(&ident);
                continue;
            }
        }

        out.push(c);
        i += 1;
    }

    out
}

fn extract_tool_call(val: &serde_json::Value) -> Option<ToolCallRequest> {
    let name = val
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| val.get("tool").and_then(|v| v.as_str()))
        .or_else(|| val.get("tool_name").and_then(|v| v.as_str()))
        .or_else(|| val.get("function").and_then(|v| v.as_str()))
        .or_else(|| {
            val.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
        });

    if let Some(name_str) = name {
        let name_trimmed = name_str.trim();
        if name_trimmed.is_empty() {
            return None;
        }
        let uuid = uuid::Uuid::new_v4().to_string();
        let raw_args = val
            .get("arguments")
            .or_else(|| val.get("parameters"))
            .or_else(|| val.get("args"))
            .or_else(|| val.get("input"))
            .or_else(|| val.get("function").and_then(|f| f.get("arguments")));

        let args = if let Some(args_val) = raw_args {
            if let Some(args_str) = args_val.as_str() {
                parse_lenient_tool_json(args_str)
                    .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()))
            } else {
                args_val.clone()
            }
        } else {
            // Treat the entire object as arguments, excluding metadata keys
            let mut map = val.as_object().cloned().unwrap_or_default();
            map.remove("name");
            map.remove("tool");
            map.remove("tool_name");
            map.remove("function");
            serde_json::Value::Object(map)
        };
        Some(ToolCallRequest {
            id: format!("call_{}", uuid),
            name: name_trimmed.to_string(),
            arguments: args,
        })
    } else {
        None
    }
}

#[cfg(test)]
#[path = "openai_tests.rs"]
mod tests;


fn is_reasoning_model(model: &str) -> bool {
    let m = model.to_lowercase();
    (m.contains("o1") || m.contains("o3") || m.contains("o4-mini"))
        && !m.contains("deepseek")
        && !m.contains("groq")
        && !m.contains("openrouter")
        && !m.contains("r1")
        && !m.contains("qwq")
}
