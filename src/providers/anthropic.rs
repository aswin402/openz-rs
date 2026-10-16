use crate::providers::circuit_breaker::CircuitBreaker;
use crate::providers::transport::{
    anthropic_messages_endpoint, build_provider_client, post_json_with_retry, ProviderAuth,
};
use crate::providers::{
    ContentPart, GenerationSettings, LLMProvider, LLMResponse, ToolCallRequest,
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
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
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
        };

        let url = anthropic_messages_endpoint(&self.api_base);
        let body_for_retry =
            serde_json::to_value(&body).map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;
        let response = post_json_with_retry(
            &self.client,
            &self.breaker,
            "anthropic",
            &url,
            &self.api_key,
            body_for_retry,
            ProviderAuth::Anthropic,
        )
        .await?;

        let anthropic_resp: AnthropicResponse = response.json().await?;

        let mut text_content = String::new();
        let mut tool_calls = Vec::new();

        for block in anthropic_resp.content {
            match block {
                ContentBlock::Text { text } => {
                    text_content.push_str(&text);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCallRequest {
                        id,
                        name,
                        arguments: input,
                    });
                }
            }
        }

        let finish_reason = match tool_calls.is_empty() {
            false => "tool_calls".to_string(),
            true => anthropic_resp
                .stop_reason
                .unwrap_or_else(|| "stop".to_string()),
        };

        let content = (!text_content.is_empty()).then_some(text_content);

        Ok(LLMResponse {
            content,
            tool_calls,
            finish_reason,
            reasoning_content: None,
        })
    }
}
