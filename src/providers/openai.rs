use crate::providers::circuit_breaker::CircuitBreaker;
use crate::providers::transport::{
    build_provider_client, openai_chat_endpoint, post_json_with_retry, ProviderAuth,
};
use crate::providers::{
    ChatStreamChunk, ContentPart, GenerationSettings, LLMProvider, LLMResponse, ToolCallRequest,
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

const FALLBACK_TOOL_BLOCK_DELIMITERS: &[(&str, &str)] = &[
    ("[TOOL_CALL]", "[/TOOL_CALL]"),
    ("<tool_call>", "</tool_call>"),
    ("```json", "```"),
    ("```tool_call", "```"),
];

const TOOL_NAME_KEYS: &[&str] = &["name", "tool", "tool_name", "function"];
const TOOL_ARG_KEYS: &[&str] = &["arguments", "parameters", "args", "input"];
const REASONING_MODEL_INCLUSIONS: &[&str] = &["o1", "o3", "o4-mini"];
const REASONING_MODEL_EXCLUSIONS: &[&str] = &["deepseek", "groq", "openrouter", "r1", "qwq"];

fn provider_auth_for_base(api_base: &str) -> ProviderAuth {
    match api_base.contains("/openai/deployments") || api_base.contains("azure") {
        true => ProviderAuth::Azure,
        false => ProviderAuth::Bearer,
    }
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

    async fn send_request(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
        stream: Option<bool>,
    ) -> Result<reqwest::Response> {
        let api_messages =
            Self::serialize_messages(&self.model, &self.api_base, system_prompt, messages).await;
        let is_reasoning = is_reasoning_model(&self.model);
        let body = OpenAIRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: (!is_reasoning).then_some(settings.temperature),
            max_tokens: (!is_reasoning).then_some(settings.max_tokens),
            max_completion_tokens: is_reasoning.then_some(settings.max_tokens),
            tools: tools.to_vec(),
            stream,
        };
        let url = openai_chat_endpoint(&self.api_base);
        let body_for_retry =
            serde_json::to_value(&body).map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;
        let auth = provider_auth_for_base(&self.api_base);
        post_json_with_retry(
            &self.client,
            &self.breaker,
            "openai",
            &url,
            &self.api_key,
            body_for_retry,
            auth,
        )
        .await
    }

    async fn serialize_messages(
        model: &str,
        api_base: &str,
        system_prompt: &str,
        messages: &[Message],
    ) -> Vec<OpenAIMessage> {
        let mut api_messages = Vec::new();
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
                .any(|p| matches!(p, ContentPart::Image { .. }));

            let content_value = match (supports_vision && has_images, parts.as_slice()) {
                (false, _) => serde_json::Value::String(text_content.clone()),
                (true, [ContentPart::Text(t)]) => serde_json::Value::String(t.clone()),
                (
                    true,
                    [ContentPart::Image {
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
                            ContentPart::Text(t) => serde_json::json!({
                                "type": "text",
                                "text": t
                            }),
                            ContentPart::Image {
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

fn parse_openai_tool_call(call: &OpenAIToolCall) -> ToolCallRequest {
    let args_str = &call.function.arguments;
    let args_parsed = match serde_json::from_str(args_str) {
        Ok(parsed) => parsed,
        Err(e) => {
            tracing::warn!("Failed to parse native tool call arguments JSON: {}", e);
            let repaired = args_str.replace('\n', "\\n").replace('\r', "\\r");
            serde_json::from_str(&repaired).unwrap_or_else(|_| {
                let mut map = serde_json::Map::new();
                map.insert(
                    "parse_error".to_string(),
                    serde_json::Value::String(format!(
                        "Tool arguments JSON was truncated or malformed: {}. This typically occurs when the response exceeds the maximum output token limit. Try writing files in smaller chunks or using command-line tools.",
                        e
                    )),
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
        let response = self
            .send_request(system_prompt, messages, tools, settings, None)
            .await?;

        let response: OpenAIResponse = response.json().await?;
        let choice = response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("openai provider error: No choices returned"))?;

        let mut tool_calls: Vec<ToolCallRequest> = choice
            .message
            .tool_calls
            .as_ref()
            .map(|calls| calls.iter().map(parse_openai_tool_call).collect())
            .unwrap_or_default();

        let mut content = choice.message.content.clone();
        let mut reasoning_content = choice.message.reasoning_content.clone();
        match content.take() {
            Some(text) => {
                let (clean_content, extracted_reasoning) = split_think_blocks(&text);
                content = clean_content;
                reasoning_content = merge_reasoning(reasoning_content, extracted_reasoning);
            }
            None => {}
        }

        match (tool_calls.is_empty(), content.as_deref()) {
            (true, Some(text)) => {
                let parsed = parse_fallback_tool_calls(text);
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
            Box<dyn futures_util::Stream<Item = Result<ChatStreamChunk>> + Send>,
        >,
    > {
        let res = self
            .send_request(system_prompt, messages, tools, settings, Some(true))
            .await?;

        let byte_stream = res.bytes_stream();
        use futures_util::StreamExt;

        let stream = futures_util::stream::unfold(
            (
                byte_stream,
                String::new(),
                std::collections::VecDeque::<ChatStreamChunk>::new(),
                false,
            ),
            |(mut stream, mut buffer, mut pending, mut done)| async move {
                loop {
                    match pending.pop_front() {
                        Some(chunk) => return Some((Ok(chunk), (stream, buffer, pending, done))),
                        None => {}
                    }
                    match done {
                        true => return None,
                        false => {}
                    }

                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].trim().to_string();
                        buffer = buffer[pos + 1..].to_string();
                        match line.strip_prefix("data:") {
                            Some(stripped) => {
                                let (chunks, is_done) = parse_sse_data_chunks(stripped);
                                for c in chunks {
                                    pending.push_back(c);
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
                            None => {}
                        }
                    }

                    match pending.pop_front() {
                        Some(chunk) => return Some((Ok(chunk), (stream, buffer, pending, done))),
                        None => {}
                    }
                    match done {
                        true => return None,
                        false => {}
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
                            match buffer.trim().is_empty() {
                                true => {}
                                false => {
                                    let remaining = std::mem::take(&mut buffer);
                                    for raw_line in remaining.lines() {
                                        match raw_line.trim().strip_prefix("data:") {
                                            Some(stripped) => {
                                                let (chunks, _) = parse_sse_data_chunks(stripped);
                                                for c in chunks {
                                                    pending.push_back(c);
                                                }
                                            }
                                            None => {}
                                        }
                                    }
                                }
                            }
                            let has_done = pending
                                .iter()
                                .any(|c| matches!(c, ChatStreamChunk::Done { .. }));
                            match has_done {
                                true => {}
                                false => pending.push_back(ChatStreamChunk::Done {
                                    finish_reason: None,
                                }),
                            }
                            done = true;
                            return pending
                                .pop_front()
                                .map(|chunk| (Ok(chunk), (stream, buffer, pending, done)));
                        }
                    }
                }
            },
        );

        let filtered = filter_think_stream(Box::pin(stream));
        Ok(Box::pin(filtered))
    }
}

pub(crate) fn parse_sse_data_chunks(data: &str) -> (Vec<ChatStreamChunk>, bool) {
    let data = data.trim();
    match data == "[DONE]" {
        true => {
            return (
                vec![ChatStreamChunk::Done {
                    finish_reason: None,
                }],
                true,
            )
        }
        false => {}
    }

    let mut chunks = Vec::new();
    let choice_opt = serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|val| {
            val.get("choices")
                .and_then(|v| v.as_array())
                .and_then(|choices| choices.first().cloned())
        });

    match choice_opt {
        None => {}
        Some(choice) => {
            match choice
                .pointer("/delta/reasoning_content")
                .or_else(|| choice.pointer("/delta/reasoning"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(reasoning) => chunks.push(ChatStreamChunk::Reasoning(reasoning.to_string())),
                None => {}
            }

            match choice
                .pointer("/delta/content")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(content) => chunks.push(ChatStreamChunk::Content(content.to_string())),
                None => {}
            }

            match choice
                .pointer("/delta/tool_calls")
                .and_then(|v| v.as_array())
            {
                None => {}
                Some(tool_calls) => {
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
                            .map(str::to_string);
                        let name = tc
                            .pointer("/function/name")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .map(str::to_string);
                        let arguments = tc
                            .pointer("/function/arguments")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .map(str::to_string);
                        match id.is_some() || name.is_some() || arguments.is_some() {
                            true => chunks.push(ChatStreamChunk::ToolCall {
                                index,
                                id,
                                name,
                                arguments,
                            }),
                            false => {}
                        }
                    }
                }
            }

            match choice
                .get("finish_reason")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(finish_reason) => chunks.push(ChatStreamChunk::Done {
                    finish_reason: Some(finish_reason.to_string()),
                }),
                None => {}
            }
        }
    }

    (chunks, false)
}

pub fn split_think_blocks(content: &str) -> (Option<String>, Option<String>) {
    match content.contains("<think>") {
        false => {
            return (
                (!content.trim().is_empty()).then(|| content.to_string()),
                None,
            );
        }
        true => {}
    }

    let mut visible = String::new();
    let mut reasoning_parts: Vec<String> = Vec::new();
    let mut rest = content;

    loop {
        match rest.find("<think>") {
            None => {
                visible.push_str(rest);
                break;
            }
            Some(start) => {
                visible.push_str(&rest[..start]);
                let after_start = &rest[start + "<think>".len()..];
                match after_start.find("</think>") {
                    None => {
                        visible.push_str(rest);
                        break;
                    }
                    Some(end) => {
                        let reasoning = after_start[..end].trim();
                        match reasoning.is_empty() {
                            true => {}
                            false => reasoning_parts.push(reasoning.to_string()),
                        }
                        rest = &after_start[end + "</think>".len()..];
                    }
                }
            }
        }
    }

    let mut normalized_visible = String::new();
    let mut last_was_horizontal_space = false;
    for ch in visible.trim().chars() {
        match ch == ' ' || ch == '\t' {
            true => {
                match last_was_horizontal_space {
                    true => {}
                    false => normalized_visible.push(' '),
                }
                last_was_horizontal_space = true;
            }
            false => {
                normalized_visible.push(ch);
                last_was_horizontal_space = false;
            }
        }
    }
    let reasoning = reasoning_parts.join("\n\n---\n\n");

    let visible = (!normalized_visible.is_empty()).then_some(normalized_visible);
    let reasoning = (!reasoning.trim().is_empty()).then_some(reasoning);
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

    fn strip_leading_newlines(&mut self, text: &mut String, clear_unconditionally: bool) {
        match self.skip_leading_newlines {
            false => {}
            true => {
                while text.starts_with('\n') || text.starts_with('\r') {
                    text.remove(0);
                }
                match clear_unconditionally || !text.is_empty() {
                    true => self.skip_leading_newlines = false,
                    false => {}
                }
            }
        }
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
                    match self.buffer.is_empty() {
                        true => break,
                        false => {}
                    }
                    match self.in_think {
                        false => {
                            match self.buffer.find("<think>") {
                                Some(start_idx) => {
                                    let mut before = self.buffer[..start_idx].to_string();
                                    self.strip_leading_newlines(&mut before, true);
                                    match before.is_empty() {
                                        true => {}
                                        false => out.push(ChatStreamChunk::Content(before)),
                                    }
                                    self.buffer =
                                        self.buffer[start_idx + "<think>".len()..].to_string();
                                    self.in_think = true;
                                    continue;
                                }
                                None => {}
                            }

                            let tail_len =
                                longest_suffix_matching_prefix(&self.buffer, "<think>");
                            match tail_len > 0 {
                                true => {
                                    match self.buffer.len() > tail_len {
                                        true => {
                                            let mut safe = self.buffer
                                                [..self.buffer.len() - tail_len]
                                                .to_string();
                                            self.buffer = self.buffer
                                                [self.buffer.len() - tail_len..]
                                                .to_string();
                                            self.strip_leading_newlines(&mut safe, false);
                                            match safe.is_empty() {
                                                true => {}
                                                false => out.push(ChatStreamChunk::Content(safe)),
                                            }
                                        }
                                        false => {}
                                    }
                                    break;
                                }
                                false => {
                                    let mut content = std::mem::take(&mut self.buffer);
                                    self.strip_leading_newlines(&mut content, false);
                                    match content.is_empty() {
                                        true => {}
                                        false => out.push(ChatStreamChunk::Content(content)),
                                    }
                                    break;
                                }
                            }
                        }
                        true => {
                            match self.buffer.find("</think>") {
                                Some(end_idx) => {
                                    let reasoning = self.buffer[..end_idx].to_string();
                                    match reasoning.is_empty() {
                                        true => {}
                                        false => out.push(ChatStreamChunk::Reasoning(reasoning)),
                                    }
                                    self.buffer =
                                        self.buffer[end_idx + "</think>".len()..].to_string();
                                    self.in_think = false;
                                    self.skip_leading_newlines = true;
                                    continue;
                                }
                                None => {}
                            }

                            let tail_len =
                                longest_suffix_matching_prefix(&self.buffer, "</think>");
                            match tail_len > 0 {
                                true => {
                                    match self.buffer.len() > tail_len {
                                        true => {
                                            let safe = self.buffer[..self.buffer.len() - tail_len]
                                                .to_string();
                                            self.buffer = self.buffer
                                                [self.buffer.len() - tail_len..]
                                                .to_string();
                                            match safe.is_empty() {
                                                true => {}
                                                false => {
                                                    out.push(ChatStreamChunk::Reasoning(safe))
                                                }
                                            }
                                        }
                                        false => {}
                                    }
                                    break;
                                }
                                false => {
                                    let reasoning = std::mem::take(&mut self.buffer);
                                    match reasoning.is_empty() {
                                        true => {}
                                        false => out.push(ChatStreamChunk::Reasoning(reasoning)),
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                out
            }
        }
    }

    pub fn flush_buffer(&mut self) -> Vec<ChatStreamChunk> {
        let mut out = Vec::new();
        match self.buffer.is_empty() {
            true => {}
            false => {
                let mut text = std::mem::take(&mut self.buffer);
                match self.in_think {
                    true => match text.is_empty() {
                        true => {}
                        false => out.push(ChatStreamChunk::Reasoning(text)),
                    },
                    false => {
                        self.strip_leading_newlines(&mut text, true);
                        match text.is_empty() {
                            true => {}
                            false => out.push(ChatStreamChunk::Content(text)),
                        }
                    }
                }
            }
        }
        out
    }
}

fn longest_suffix_matching_prefix(buffer: &str, tag: &str) -> usize {
    let max_len = (tag.len() - 1).min(buffer.len());
    (1..=max_len)
        .rev()
        .find(|&len| {
            buffer
                .get(buffer.len() - len..)
                .is_some_and(|suffix| tag.starts_with(suffix))
        })
        .unwrap_or(0)
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
            match pending.pop_front() {
                Some(chunk) => return Some((Ok(chunk), (stream, filter, pending, done))),
                None => {}
            }

            match done {
                true => return None,
                false => {}
            }

            loop {
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        let processed = filter.process_chunk(chunk);
                        for c in processed {
                            pending.push_back(c);
                        }
                        match pending.pop_front() {
                            Some(first) => {
                                return Some((Ok(first), (stream, filter, pending, done)))
                            }
                            None => {}
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
                        return pending
                            .pop_front()
                            .map(|first| (Ok(first), (stream, filter, pending, done)));
                    }
                }
            }
        },
    )
}

pub fn merge_reasoning(existing: Option<String>, extracted: Option<String>) -> Option<String> {
    let clean_existing = existing.filter(|s| !s.trim().is_empty());
    let clean_extracted = extracted.filter(|s| !s.trim().is_empty());
    match (clean_existing, clean_extracted) {
        (Some(a), Some(b)) => Some(format!("{}\n\n---\n\n{}", a.trim(), b.trim())),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub fn parse_fallback_tool_calls(content: &str) -> Vec<ToolCallRequest> {
    let mut tool_calls = Vec::new();

    for (start_tag, end_tag) in FALLBACK_TOOL_BLOCK_DELIMITERS {
        let mut search_str = content;
        while let Some(start_idx) = search_str.find(start_tag) {
            let after_start = &search_str[start_idx + start_tag.len()..];
            match after_start.find(end_tag) {
                Some(end_idx) => {
                    let raw_inner = after_start[..end_idx].trim();
                    match parse_lenient_tool_json(raw_inner).and_then(|val| extract_tool_call(&val))
                    {
                        Some(tc) => tool_calls.push(tc),
                        None => {}
                    }
                    search_str = &after_start[end_idx + end_tag.len()..];
                }
                None => break,
            }
        }
    }

    match tool_calls.is_empty() {
        true => {
            let trimmed = content.trim();
            match trimmed.starts_with('{') && trimmed.ends_with('}') {
                true => {
                    match parse_lenient_tool_json(trimmed).and_then(|val| extract_tool_call(&val)) {
                        Some(tc) => tool_calls.push(tc),
                        None => {}
                    }
                }
                false => {}
            }
        }
        false => {}
    }

    tool_calls
}

fn parse_lenient_tool_json(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .or_else(|| {
            let normalized = normalize_arrow_map_syntax(trimmed);
            serde_json::from_str::<serde_json::Value>(&normalized).ok()
        })
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
        match (in_double, in_single) {
            (true, _) => {
                out.push(c);
                match (c == '\\' && i + 1 < chars.len(), c == '"') {
                    (true, _) => {
                        i += 1;
                        out.push(chars[i]);
                    }
                    (false, true) => {
                        in_double = false;
                    }
                    (false, false) => {}
                }
                i += 1;
                continue;
            }
            (false, true) => {
                match (c == '\\' && i + 1 < chars.len(), c) {
                    (true, _) => {
                        i += 1;
                        let escaped = chars[i];
                        match escaped == '"' {
                            true => out.push('\\'),
                            false => {}
                        }
                        out.push(escaped);
                    }
                    (false, '\'') => {
                        out.push('"');
                        in_single = false;
                    }
                    (false, '"') => {
                        out.push('\\');
                        out.push('"');
                    }
                    (false, _) => out.push(c),
                }
                i += 1;
                continue;
            }
            (false, false) => {}
        }

        match c {
            '"' => {
                in_double = true;
                out.push(c);
                i += 1;
                continue;
            }
            '\'' => {
                in_single = true;
                out.push('"');
                i += 1;
                continue;
            }
            '=' => match i + 1 < chars.len() && chars[i + 1] == '>' {
                true => {
                    out.push(':');
                    i += 2;
                    continue;
                }
                false => {}
            },
            _ => {}
        }

        match c.is_ascii_alphabetic() || c == '_' {
            true => {
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
                let is_arrow = lookahead + 1 < chars.len()
                    && chars[lookahead] == '='
                    && chars[lookahead + 1] == '>';
                let is_colon = lookahead < chars.len() && chars[lookahead] == ':';
                match (is_arrow, is_colon) {
                    (true, _) => {
                        out.push('"');
                        out.push_str(&ident);
                        out.push_str("\":");
                        i = lookahead + 2;
                    }
                    (false, true) => {
                        out.push('"');
                        out.push_str(&ident);
                        out.push_str("\":");
                        i = lookahead + 1;
                    }
                    (false, false) => {
                        out.push_str(&ident);
                    }
                }
                continue;
            }
            false => {
                out.push(c);
                i += 1;
            }
        }
    }

    out
}

fn extract_tool_call(val: &serde_json::Value) -> Option<ToolCallRequest> {
    let name_str = TOOL_NAME_KEYS
        .iter()
        .find_map(|key| val.get(*key).and_then(|v| v.as_str()))
        .or_else(|| {
            val.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
        })?;

    let name_trimmed = name_str.trim();
    (!name_trimmed.is_empty()).then_some(())?;

    let uuid = uuid::Uuid::new_v4().to_string();
    let raw_args = TOOL_ARG_KEYS
        .iter()
        .find_map(|key| val.get(*key))
        .or_else(|| val.get("function").and_then(|f| f.get("arguments")));

    let args = match raw_args {
        Some(args_val) => match args_val.as_str() {
            Some(args_str) => parse_lenient_tool_json(args_str)
                .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new())),
            None => args_val.clone(),
        },
        None => {
            let mut map = val.as_object().cloned().unwrap_or_default();
            for key in TOOL_NAME_KEYS {
                map.remove(*key);
            }
            serde_json::Value::Object(map)
        }
    };

    Some(ToolCallRequest {
        id: format!("call_{}", uuid),
        name: name_trimmed.to_string(),
        arguments: args,
    })
}

#[cfg(test)]
#[path = "openai_tests.rs"]
mod tests;

fn is_reasoning_model(model: &str) -> bool {
    let m = model.to_lowercase();
    REASONING_MODEL_INCLUSIONS
        .iter()
        .any(|needle| m.contains(needle))
        && !REASONING_MODEL_EXCLUSIONS
            .iter()
            .any(|needle| m.contains(needle))
}
