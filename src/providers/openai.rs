use crate::providers::{LLMProvider, LLMResponse, GenerationSettings, ToolCallRequest};
use crate::session::Message;
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct OpenAIProvider {
    pub client: Client,
    pub api_key: String,
    pub api_base: String,
    pub model: String,
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
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
            client: Client::builder().use_rustls_tls().build().unwrap_or_default(),
            api_key,
            api_base,
            model,
        }
    }

    fn serialize_messages(
        model: &str,
        system_prompt: &str,
        messages: &[Message],
    ) -> Vec<OpenAIMessage> {
        let mut api_messages = Vec::new();
        
        if !system_prompt.is_empty() {
            api_messages.push(OpenAIMessage {
                role: "system".to_string(),
                content: serde_json::Value::String(system_prompt.to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
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

            let parts = crate::providers::parse_multimodal_content(&msg.content);
            let has_images = parts.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }));
            let supports_vision = crate::providers::model_supports_vision(model);

            let content_value = if !supports_vision || !has_images {
                serde_json::Value::String(msg.content.clone())
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
        let api_messages = Self::serialize_messages(&self.model, system_prompt, messages);

        let body = OpenAIRequest {
            model: self.model.clone(),
            messages: api_messages,
            temperature: Some(settings.temperature),
            max_tokens: Some(settings.max_tokens),
            tools: tools.to_vec(),
        };

        let is_azure = self.api_base.contains("/openai/deployments") || self.api_base.contains("azure");
        let url = if is_azure {
            self.api_base.clone()
        } else {
            format!("{}/chat/completions", self.api_base)
        };

        let mut req = self.client.post(&url);
        if is_azure {
            req = req.header("api-key", &self.api_key);
        } else {
            req = req.bearer_auth(&self.api_key);
        }

        let res = req.json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let error_text = res.text().await?;
            return Err(anyhow!("OpenAI API error: {}", error_text));
        }

        let response: OpenAIResponse = res.json().await?;
        let choice = response.choices.first().ok_or_else(|| anyhow!("No choices returned"))?;
        
        let tool_calls = choice.message.tool_calls.as_ref().map(|calls| {
            calls.iter().map(|call| {
                let args_parsed = serde_json::from_str(&call.function.arguments)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                ToolCallRequest {
                    id: call.id.clone(),
                    name: call.function.name.clone(),
                    arguments: args_parsed,
                }
            }).collect()
        }).unwrap_or_default();

        Ok(LLMResponse {
            content: choice.message.content.clone(),
            tool_calls,
            finish_reason: choice.finish_reason.clone().unwrap_or_else(|| "stop".to_string()),
            reasoning_content: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_messages_vision_fallback() {
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
            system_prompt,
            &messages,
        );
        
        assert_eq!(serialized_non_vision.len(), 2);
        assert_eq!(serialized_non_vision[0].role, "system");
        assert_eq!(serialized_non_vision[1].role, "user");
        
        // It must be serialized as String, keeping the original content (including the tag)
        let content_str = serialized_non_vision[1].content.as_str().unwrap();
        assert_eq!(content_str, msg_content);

        // Test vision model (gpt-4o)
        let serialized_vision = OpenAIProvider::serialize_messages(
            "gpt-4o",
            system_prompt,
            &messages,
        );
        
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
