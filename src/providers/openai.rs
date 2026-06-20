use crate::providers::{LLMProvider, LLMResponse, GenerationSettings, ToolCallRequest};
use crate::session::Message;
use crate::agent::AgentError;
use crate::providers::circuit_breaker::{CircuitBreaker, retry_with_backoff};
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
            client: Client::builder()
                .use_rustls_tls()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
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
        
        if !system_prompt.is_empty() {
            api_messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: serde_json::Value::String(system_prompt.to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            });
        }

        for msg in messages {
            let mut tool_call_id = None;
            let mut name = None;
            let role = msg.role.clone();
            
            if role == "tool" {
                tool_call_id = msg.extra.get("tool_call_id").and_then(|v| v.as_str().map(|s| s.to_string()));
            }

            if let Some(n) = msg.extra.get("name").and_then(|v| v.as_str()) {
                name = Some(n.to_string());
            }

            // Handle reasoning_content for assistant messages.
            // DeepSeek requires it as a dedicated top-level field; other providers
            // (OpenRouter, Groq, etc.) get it embedded as <think> tags in content.
            let reasoning_field: Option<String>;
            let mut text_content = msg.content.clone();
            if role == "assistant" {
                if let Some(reasoning) = msg.extra.get("reasoning_content").and_then(|v| v.as_str()) {
                    if !reasoning.is_empty() {
                        if is_deepseek {
                            // Pass as a separate field — DeepSeek API requirement
                            reasoning_field = Some(reasoning.to_string());
                        } else {
                            // Embed in content for other OpenAI-compatible providers
                            text_content = format!("<think>\n{}\n</think>\n\n{}", reasoning, text_content);
                            reasoning_field = None;
                        }
                    } else {
                        reasoning_field = None;
                    }
                } else {
                    reasoning_field = None;
                }
            } else {
                reasoning_field = None;
            }

            let parts = crate::providers::parse_multimodal_content(&text_content).await;
            let has_images = parts.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }));
            let supports_vision = crate::providers::model_supports_vision(model);

            let content_value = if !supports_vision || !has_images {
                serde_json::Value::String(text_content.clone())
            } else if parts.len() == 1 {
                match &parts[0] {
                    crate::providers::ContentPart::Text(t) => serde_json::Value::String(t.clone()),
                    crate::providers::ContentPart::Image { mime_type, base64_data } => {
                        serde_json::json!([
                            {
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{};base64,{}", mime_type, base64_data)
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
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{};base64,{}", mime_type, base64_data)
                                }
                            }));
                        }
                    }
                }
                serde_json::Value::Array(arr)
            };

            api_messages.push(OpenAIMessage {
                role,
                content: content_value,
                name,
                tool_calls: msg.extra.get("tool_calls").cloned().and_then(|v| v.as_array().cloned()),
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
        let api_messages = Self::serialize_messages(&self.model, &self.api_base, system_prompt, messages).await;

        let is_reasoning = is_reasoning_model(&self.model);
        let body = OpenAIRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: if is_reasoning { None } else { Some(settings.temperature) },
            max_tokens: if is_reasoning { None } else { Some(settings.max_tokens) },
            max_completion_tokens: if is_reasoning { Some(settings.max_tokens) } else { None },
            tools: tools.to_vec(),
        };

        let is_azure = self.api_base.contains("/openai/deployments") || self.api_base.contains("azure");
        let url = if is_azure {
            self.api_base.clone()
        } else {
            let base = self.api_base.trim_end_matches('/');
            format!("{}/chat/completions", base)
        };

        // Clone state for retry closure
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let url_for_retry = url.clone();
        let body_for_retry = serde_json::to_value(&body).map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;

        let response = retry_with_backoff(
            &self.breaker,
            3,
            Duration::from_secs(1),
            Duration::from_secs(30),
            "openai",
            || {
                let client = client.clone();
                let api_key = api_key.clone();
                let url = url_for_retry.clone();
                let json_body = body_for_retry.clone();
                async move {
                    let mut req = client.post(&url);
                    if is_azure {
                        req = req.header("api-key", &api_key);
                    } else {
                        req = req.bearer_auth(&api_key);
                    }
                    let res = req
                        .json(&json_body)
                        .send()
                        .await
                        .map_err(|e| (0u16, format!("Network error: {e}")))?;
                    if !res.status().is_success() {
                        let status = res.status().as_u16();
                        let error_text = res.text().await.unwrap_or_default();
                        Err((status, error_text))
                    } else {
                        Ok(res)
                    }
                }
            },
        ).await?;

        let response: OpenAIResponse = response.json().await?;
        let choice = response.choices.first().ok_or_else(|| AgentError::provider("openai", "No choices returned"))?;
        
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
            finish_reason: choice.finish_reason.clone().unwrap_or_else(|| "stop".to_string()),
            reasoning_content: choice.message.reasoning_content.clone(),
        })
    }
}

pub fn parse_fallback_tool_calls(content: &str) -> Vec<ToolCallRequest> {
    let mut tool_calls = Vec::new();

    // Look for ```json ... ``` blocks
    let start_tag = "```json";
    let end_tag = "```";

    let mut search_str = content;
    while let Some(start_idx) = search_str.find(start_tag) {
        let after_start = &search_str[start_idx + start_tag.len()..];
        if let Some(end_idx) = after_start.find(end_tag) {
            let json_str = after_start[..end_idx].trim();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                if let Some(tc) = extract_tool_call(&val) {
                    tool_calls.push(tc);
                }
            }
            search_str = &after_start[end_idx + end_tag.len()..];
        } else {
            break;
        }
    }

    // If no markdown JSON blocks found, maybe the whole content is raw JSON?
    if tool_calls.is_empty() {
        let trimmed = content.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(tc) = extract_tool_call(&val) {
                    tool_calls.push(tc);
                }
            }
        }
    }

    tool_calls
}

fn extract_tool_call(val: &serde_json::Value) -> Option<ToolCallRequest> {
    let name = val.get("name").and_then(|v| v.as_str())
        .or_else(|| val.get("function").and_then(|v| v.as_str()));

    if let Some(name_str) = name {
        let uuid = uuid::Uuid::new_v4().to_string();
        let args = if let Some(args_val) = val.get("arguments").or_else(|| val.get("parameters")) {
            args_val.clone()
        } else {
            // Treat the entire object as arguments, excluding metadata keys
            let mut map = val.as_object().cloned().unwrap_or_default();
            map.remove("name");
            map.remove("function");
            serde_json::Value::Object(map)
        };
        Some(ToolCallRequest {
            id: format!("call_{}", uuid),
            name: name_str.to_string(),
            arguments: args,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_serialize_messages_vision_fallback() {
        let system_prompt = "system instructions";
        
        let temp_dir = std::env::temp_dir();
        let test_img_path = temp_dir.join("test_img.png");
        std::fs::write(&test_img_path, vec![0; 100]).unwrap();
        
        let image_tag = format!("![](file://{})", test_img_path.to_string_lossy());
        let msg_content = format!("{} check this image", image_tag);
        
        let messages = vec![Message {
            role: "user".to_string(),
            content: msg_content.clone(),
            timestamp: None,
            extra: serde_json::Map::new(),
        }];

        // Test non-vision model (deepseek-v4-flash-free)
        let serialized_non_vision = OpenAIProvider::serialize_messages(
            "deepseek-v4-flash-free",
            "https://api.openai.com/v1",
            system_prompt,
            &messages,
        ).await;
        
        assert_eq!(serialized_non_vision.len(), 2);
        assert_eq!(serialized_non_vision[0].role, "system");
        assert_eq!(serialized_non_vision[1].role, "user");
        
        // It must be serialized as String, keeping the original content (including the tag)
        let content_str = serialized_non_vision[1].content.as_str().unwrap();
        assert_eq!(content_str, msg_content);

        // Test vision model (gpt-4o)
        let serialized_vision = OpenAIProvider::serialize_messages(
            "gpt-4o",
            "https://api.openai.com/v1",
            system_prompt,
            &messages,
        ).await;
        
        assert_eq!(serialized_vision.len(), 2);
        assert_eq!(serialized_vision[1].role, "user");
        
        // It must be serialized as Array containing text and image_url parts
        let content_array = serialized_vision[1].content.as_array().unwrap();
        assert_eq!(content_array.len(), 2);
        
        assert_eq!(content_array[0]["type"], "image_url");
        assert!(content_array[0]["image_url"]["url"].as_str().unwrap().starts_with("data:image/png;base64,"));
        
        assert_eq!(content_array[1]["type"], "text");
        assert_eq!(content_array[1]["text"], " check this image");

        // Cleanup
        let _ = std::fs::remove_file(test_img_path);
    }
}

fn is_reasoning_model(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("o1") || m.contains("o3") || m.contains("o4-mini")
}
