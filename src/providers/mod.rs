use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::session::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCallRequest>,
    pub finish_reason: String,
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GenerationSettings {
    pub temperature: f32,
    pub max_tokens: usize,
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ContentPart {
    Text(String),
    Image {
        mime_type: String,
        base64_data: String,
    },
}

pub fn parse_multimodal_content(text: &str) -> Vec<ContentPart> {
    let re = regex::Regex::new(r"!\[.*?\]\((.*?)\)").unwrap();
    let mut parts = Vec::new();
    let mut last_index = 0;

    for cap in re.captures_iter(text) {
        let mat = cap.get(0).unwrap();
        let path_or_url = cap.get(1).unwrap().as_str();

        // Push text preceding the match
        let before = &text[last_index..mat.start()];
        if !before.is_empty() {
            parts.push(ContentPart::Text(before.to_string()));
        }

        // Try reading image from path
        let clean_path = path_or_url.trim_start_matches("file://");
        let path = std::path::Path::new(clean_path);

        if path.exists() && path.is_file() {
            if let Ok(data) = std::fs::read(path) {
                let mime_type = match path.extension().and_then(|ext| ext.to_str()) {
                    Some("png") => "image/png",
                    Some("jpg") | Some("jpeg") => "image/jpeg",
                    Some("webp") => "image/webp",
                    Some("gif") => "image/gif",
                    _ => "image/png", // default fallback
                };
                use base64::{Engine as _, engine::general_purpose};
                let base64_data = general_purpose::STANDARD.encode(data);
                parts.push(ContentPart::Image {
                    mime_type: mime_type.to_string(),
                    base64_data,
                });
            } else {
                parts.push(ContentPart::Text(mat.as_str().to_string()));
            }
        } else {
            parts.push(ContentPart::Text(mat.as_str().to_string()));
        }

        last_index = mat.end();
    }

    let remaining = &text[last_index..];
    if !remaining.is_empty() {
        parts.push(ContentPart::Text(remaining.to_string()));
    }

    if parts.is_empty() && !text.is_empty() {
        parts.push(ContentPart::Text(text.to_string()));
    }

    parts
}

pub fn model_supports_vision(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("gpt-4o") 
        || m.contains("claude-3-5") 
        || m.contains("claude-3-opus") 
        || m.contains("gemini") 
        || m.contains("vision")
        || m.contains("pixtral")
        || m.contains("llama-3.2")
}

#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
    ) -> Result<LLMResponse>;
}

pub mod openai;
pub mod anthropic;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_supports_vision() {
        assert!(model_supports_vision("gpt-4o"));
        assert!(model_supports_vision("gpt-4o-mini"));
        assert!(model_supports_vision("claude-3-5-sonnet"));
        assert!(model_supports_vision("google/gemini-2.5-flash"));
        assert!(model_supports_vision("pixtral-12b"));
        
        assert!(!model_supports_vision("deepseek-chat"));
        assert!(!model_supports_vision("deepseek-v4-flash-free"));
        assert!(!model_supports_vision("gpt-3.5-turbo"));
    }
}
