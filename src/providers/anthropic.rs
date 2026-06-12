use crate::providers::{LLMProvider, LLMResponse, GenerationSettings, ToolCallRequest};
use crate::session::Message;
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct AnthropicProvider {
    pub client: Client,
    pub api_key: String,
    pub api_base: String,
    pub model: String,
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

impl AnthropicProvider {
    pub fn new(api_key: String, api_base: String, model: String) -> Self {
        AnthropicProvider {
            client: Client::builder().use_rustls_tls().build().unwrap_or_default(),
            api_key,
            api_base,
            model,
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
        
        for msg in messages {
            let role = match msg.role.as_str() {
                "user" => "user",
                "assistant" => "assistant",
                "tool" => "user",
                _ => "user",
            };

            let content = if msg.role == "tool" {
                let tool_call_id = msg.extra.get("tool_call_id").and_then(|v| v.as_str()).unwrap_or("");
                serde_json::json!([
                    {
                        "type": "tool_result",
                        "tool_use_id": tool_call_id,
                        "content": msg.content
                    }
                ])
            } else if let Some(tool_calls) = msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
                let mut blocks = Vec::new();
                if !msg.content.is_empty() {
                    let parts = crate::providers::parse_multimodal_content(&msg.content);
                    let supports_vision = crate::providers::model_supports_vision(&self.model);

                    for part in parts {
                        match part {
                            crate::providers::ContentPart::Text(t) => {
                                blocks.push(serde_json::json!({
                                    "type": "text",
                                    "text": t
                                }));
                            }
                            crate::providers::ContentPart::Image { mime_type, base64_data } => {
                                if supports_vision {
                                    blocks.push(serde_json::json!({
                                        "type": "image",
                                        "source": {
                                            "type": "base64",
                                            "media_type": mime_type,
                                            "data": base64_data
                                        }
                                    }));
                                }
                            }
                        }
                    }
                }
                for call in tool_calls {
                    if let Some(call_obj) = call.as_object() {
                        let id = call_obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let func = call_obj.get("function").and_then(|v| v.as_object());
                        let name = func.and_then(|f| f.get("name")).and_then(|v| v.as_str()).unwrap_or("");
                        let args_str = func.and_then(|f| f.get("arguments")).and_then(|v| v.as_str()).unwrap_or("{}");
                        let args: serde_json::Value = serde_json::from_str(args_str).unwrap_or(serde_json::Value::Null);
                        blocks.push(serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": args
                        }));
                    }
                }
                serde_json::Value::Array(blocks)
            } else {
                let parts = crate::providers::parse_multimodal_content(&msg.content);
                let has_images = parts.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }));
                let supports_vision = crate::providers::model_supports_vision(&self.model);

                if !supports_vision || !has_images {
                    serde_json::Value::String(msg.content.clone())
                } else if parts.len() == 1 {
                    match &parts[0] {
                        crate::providers::ContentPart::Text(t) => serde_json::Value::String(t.clone()),
                        crate::providers::ContentPart::Image { mime_type, base64_data } => {
                            serde_json::json!([
                                {
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": mime_type,
                                        "data": base64_data
                                    }
                                }
                            ])
                        }
                    }
                } else {
                    let mut arr = Vec::new();
                    for part in parts {
                        match part {
                            crate::providers::ContentPart::Text(t) => {
                                arr.push(serde_json::json!({
                                    "type": "text",
                                    "text": t
                                }));
                            }
                            crate::providers::ContentPart::Image { mime_type, base64_data } => {
                                arr.push(serde_json::json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": mime_type,
                                        "data": base64_data
                                    }
                                }));
                            }
                        }
                    }
                    serde_json::Value::Array(arr)
                }
            };

            api_messages.push(AnthropicMessage {
                role: role.to_string(),
                content,
            });
        }

        let mut anthropic_tools = Vec::new();
        for tool in tools {
            if let Some(tool_obj) = tool.as_object() {
                let func = tool_obj.get("function").and_then(|v| v.as_object());
                if let Some(f) = func {
                    let name = f.get("name").cloned().unwrap_or(serde_json::Value::Null);
                    let desc = f.get("description").cloned().unwrap_or(serde_json::Value::Null);
                    let schema = f.get("parameters").cloned().unwrap_or(serde_json::Value::Null);
                    anthropic_tools.push(serde_json::json!({
                        "name": name,
                        "description": desc,
                        "input_schema": schema
                    }));
                }
            }
        }

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

        let res = self.client.post(&format!("{}/v1/messages", self.api_base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "prompt-caching-2024-07-31")
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let error_text = res.text().await?;
            return Err(anyhow!("Anthropic API error: {}", error_text));
        }

        let response: AnthropicResponse = res.json().await?;
        
        let mut text_content = String::new();
        let mut tool_calls = Vec::new();

        for block in response.content {
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

        let finish_reason = if !tool_calls.is_empty() {
            "tool_calls".to_string()
        } else {
            response.stop_reason.unwrap_or_else(|| "stop".to_string())
        };

        let content = if text_content.is_empty() { None } else { Some(text_content) };

        Ok(LLMResponse {
            content,
            tool_calls,
            finish_reason,
            reasoning_content: None,
        })
    }
}
