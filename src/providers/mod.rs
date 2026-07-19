use crate::session::Message;
use anyhow::Result;
use serde::{Deserialize, Serialize};

static HTTP_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

const MULTIMODAL_CONNECT_TIMEOUT_SECS: u64 = 10;
const MULTIMODAL_READ_TIMEOUT_SECS: u64 = 30;
const MULTIMODAL_TOTAL_TIMEOUT_SECS: u64 = 60;

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .use_rustls_tls()
            .connect_timeout(std::time::Duration::from_secs(
                MULTIMODAL_CONNECT_TIMEOUT_SECS,
            ))
            .read_timeout(std::time::Duration::from_secs(MULTIMODAL_READ_TIMEOUT_SECS))
            .timeout(std::time::Duration::from_secs(
                MULTIMODAL_TOTAL_TIMEOUT_SECS,
            ))
            .build()
            .unwrap_or_default()
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatStreamChunk {
    Content(String),
    Reasoning(String),
    ToolCall {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: Option<String>,
    },
    Done {
        finish_reason: Option<String>,
    },
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

pub async fn parse_multimodal_content(text: &str) -> Vec<ContentPart> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"!\[.*?\]\((.*?)\)")
            .expect("static multimodal image markdown regex must compile")
    });
    let mut parts = Vec::new();
    let mut last_index = 0;

    for cap in re.captures_iter(text) {
        let Some(mat) = cap.get(0) else {
            continue;
        };
        let Some(path_match) = cap.get(1) else {
            continue;
        };
        let path_or_url = path_match.as_str();

        // Push text preceding the match
        let before = &text[last_index..mat.start()];
        if !before.is_empty() {
            parts.push(ContentPart::Text(before.to_string()));
        }

        let mut image_data = None;
        let mut resolved_mime_type = "image/png".to_string();

        if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            // Fetch remote image asynchronously
            let client = get_http_client();

            if let Ok(resp) = client.get(path_or_url).send().await {
                let mut size_ok = true;
                if let Some(len_header) = resp.headers().get(reqwest::header::CONTENT_LENGTH) {
                    if let Ok(len_str) = len_header.to_str() {
                        if let Ok(len) = len_str.parse::<u64>() {
                            if len > 20 * 1024 * 1024 {
                                size_ok = false;
                            }
                        }
                    }
                }

                if size_ok {
                    if let Some(content_type) = resp
                        .headers()
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|h| h.to_str().ok().map(|s| s.to_string()))
                    {
                        if content_type.starts_with("image/") {
                            resolved_mime_type = content_type;
                        }
                    }
                    if let Ok(bytes) = resp.bytes().await {
                        if bytes.len() <= 20 * 1024 * 1024 {
                            image_data = Some(bytes.to_vec());
                        }
                    }
                }
            }
        } else {
            // Read local image
            let clean_path = path_or_url.trim_start_matches("file://");
            let path = std::path::Path::new(clean_path);

            if path.exists() && path.is_file() {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if metadata.len() <= 20 * 1024 * 1024 {
                        if let Ok(data) = std::fs::read(path) {
                            resolved_mime_type =
                                match path.extension().and_then(|ext| ext.to_str()) {
                                    Some("png") => "image/png",
                                    Some("jpg") | Some("jpeg") => "image/jpeg",
                                    Some("webp") => "image/webp",
                                    Some("gif") => "image/gif",
                                    _ => "image/png", // default fallback
                                }
                                .to_string();
                            image_data = Some(data);
                        }
                    }
                }
            }
        }

        if let Some(data) = image_data {
            use base64::{engine::general_purpose, Engine as _};
            let base64_data = general_purpose::STANDARD.encode(data);
            parts.push(ContentPart::Image {
                mime_type: resolved_mime_type,
                base64_data,
            });
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

    if m.contains("gpt-4o")
        || m.starts_with("o3")
        || m.contains("gpt-4-turbo")
        || m.contains("gpt-4-vision-preview")
    {
        return true;
    }
    if m.starts_with("o1") && !m.contains("mini") && !m.contains("preview") {
        return true;
    }
    if m.contains("claude-3") || m.contains("claude-4") {
        return true;
    }
    if m.contains("gemini") {
        return true;
    }
    if m.contains("paligemma") {
        return true;
    }
    if m.contains("gemma-4") {
        return true;
    }
    if m.contains("omni") {
        return true;
    }
    if m.contains("pixtral") {
        return true;
    }
    if m.contains("qvq") || m.contains("qwen-vl") || m.contains("deepseek-vl") {
        return true;
    }

    // Check if the model name has a distinct vision/multimodal word separated by -, _, or /
    // This avoids false positives like "supervision"
    let has_vision_word = m
        .split(['/', '-', '_', ':'])
        .any(|word| word == "vision" || word == "vl" || word == "vlm" || word == "multimodal");
    if has_vision_word {
        return true;
    }

    false
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

    async fn chat_stream(
        &self,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
    ) -> Result<std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<ChatStreamChunk>> + Send>>>
    {
        let resp = self.chat(system_prompt, messages, tools, settings).await?;
        let mut chunks = Vec::new();
        if let Some(reasoning) = resp.reasoning_content {
            chunks.push(ChatStreamChunk::Reasoning(reasoning));
        }
        if let Some(content) = resp.content {
            chunks.push(ChatStreamChunk::Content(content));
        }
        for (idx, tc) in resp.tool_calls.into_iter().enumerate() {
            chunks.push(ChatStreamChunk::ToolCall {
                index: idx,
                id: Some(tc.id),
                name: Some(tc.name),
                arguments: Some(tc.arguments.to_string()),
            });
        }
        chunks.push(ChatStreamChunk::Done {
            finish_reason: Some(resp.finish_reason),
        });
        let stream = futures_util::stream::iter(chunks.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}

pub mod anthropic;
pub mod circuit_breaker;
pub mod ollama_manager;
pub mod openai;
pub mod resolver;

#[cfg(test)]
pub mod mock;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_supports_vision() {
        // OpenAI
        assert!(model_supports_vision("gpt-4o"));
        assert!(model_supports_vision("gpt-4o-mini"));
        assert!(model_supports_vision("gpt-4-turbo"));
        assert!(model_supports_vision("gpt-4-vision-preview"));
        assert!(model_supports_vision("o1"));
        assert!(!model_supports_vision("o1-mini"));
        assert!(!model_supports_vision("o1-preview"));
        assert!(model_supports_vision("o3-mini"));
        // Anthropic
        assert!(model_supports_vision("claude-3-5-sonnet"));
        assert!(model_supports_vision("claude-3-opus"));
        assert!(model_supports_vision("claude-4-sonnet"));
        // Google
        assert!(model_supports_vision("google/gemini-2.5-flash"));
        assert!(model_supports_vision("google_ai_studio/gemini-2.0-flash"));
        assert!(model_supports_vision("nvidia/google/gemma-4-31b-it"));
        assert!(model_supports_vision("google/gemma-4-26b-a4b-it:free"));
        assert!(model_supports_vision(
            "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free"
        ));
        assert!(!model_supports_vision("nvidia/google/gemma-2-31b-it"));
        assert!(!model_supports_vision("google/gemma-2-27b-it"));
        // Meta Llama vision
        assert!(model_supports_vision("meta/llama-3.2-90b-vision"));
        assert!(model_supports_vision(
            "nvidia/meta/llama-3.2-90b-vision-instruct"
        ));
        // Mistral
        assert!(model_supports_vision("pixtral-12b"));
        assert!(model_supports_vision("pixtral-large-latest"));
        // Other vision models
        assert!(model_supports_vision("deepseek-vl"));
        assert!(model_supports_vision("qwen-vl-plus"));
        assert!(model_supports_vision("nvidia/nemotron-nano-12b-v2-vl:free"));
        assert!(model_supports_vision("qwen/qwen3-vl-32b-instruct"));
        assert!(model_supports_vision("qwen/qwen2.5-vl-72b-instruct"));
        assert!(model_supports_vision(
            "microsoft/phi-3-vision-128k-instruct"
        ));
        assert!(model_supports_vision("microsoft/phi-4-multimodal-instruct"));
        assert!(model_supports_vision(
            "nvidia/llama-3.1-nemotron-nano-vl-8b-v1"
        ));
        assert!(model_supports_vision(
            "nvidia/llama-3.2-nemoretriever-1b-vlm-embed-v1"
        ));
        assert!(model_supports_vision("baidu/ernie-4.5-vl-424b-a47b"));
        // Non-vision models
        assert!(!model_supports_vision("deepseek-chat"));
        assert!(!model_supports_vision("deepseek-v4-flash-free"));
        assert!(!model_supports_vision("gpt-3.5-turbo"));
        assert!(!model_supports_vision("llama-3.1-8b-instant"));
        assert!(!model_supports_vision("openrouter/free"));
        assert!(!model_supports_vision("my-supervision-model"));
    }
}
