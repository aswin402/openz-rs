use crate::session::Message;
use anyhow::Result;
use serde::{Deserialize, Serialize};

static HTTP_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

const MULTIMODAL_CONNECT_TIMEOUT_SECS: u64 = 10;
const MULTIMODAL_READ_TIMEOUT_SECS: u64 = 30;
const MULTIMODAL_TOTAL_TIMEOUT_SECS: u64 = 60;

fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        crate::core::http::http_client_with_read_timeout(
            std::time::Duration::from_secs(MULTIMODAL_CONNECT_TIMEOUT_SECS),
            std::time::Duration::from_secs(MULTIMODAL_READ_TIMEOUT_SECS),
            std::time::Duration::from_secs(MULTIMODAL_TOTAL_TIMEOUT_SECS),
        )
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

const MAX_MULTIMODAL_IMAGE_BYTES: usize = 20 * 1024 * 1024;

const IMAGE_MAGIC_SIGNATURES: &[(&[u8], &str)] = &[
    (b"\x89PNG\r\n\x1a\n", "image/png"),
    (b"\xff\xd8\xff", "image/jpeg"),
    (b"GIF87a", "image/gif"),
    (b"GIF89a", "image/gif"),
];

fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    let prefix_match = IMAGE_MAGIC_SIGNATURES
        .iter()
        .find(|(sig, _)| bytes.starts_with(sig))
        .map(|(_, mime)| *mime);

    let webp_match = (bytes.len() >= 12
        && bytes.starts_with(b"RIFF")
        && &bytes[8..12] == b"WEBP")
        .then_some("image/webp");

    prefix_match.or(webp_match)
}

fn find_next_markdown_image(text: &str) -> Option<(usize, usize, &str)> {
    let mut search_offset = 0;
    while let Some(rel_start) = text[search_offset..].find("![") {
        let start = search_offset + rel_start;
        let after_bang = start + 2;
        let candidate = text[after_bang..].find("](").and_then(|rel_mid| {
            let alt_slice = &text[after_bang..after_bang + rel_mid];
            let target_start = after_bang + rel_mid + 2;
            (!alt_slice.contains('\n'))
                .then(|| text[target_start..].find(')'))
                .flatten()
                .and_then(|rel_end| {
                    let target = &text[target_start..target_start + rel_end];
                    (!target.is_empty() && !target.contains('\n'))
                        .then_some((start, target_start + rel_end + 1, target.trim()))
                })
        });
        match candidate {
            Some(found) => return Some(found),
            None => search_offset = after_bang,
        }
    }
    None
}

async fn fetch_remote_image(url_str: &str) -> Option<(String, Vec<u8>)> {
    let parsed_url = reqwest::Url::parse(url_str).ok()?;
    let url_for_validation = parsed_url.clone();
    tokio::task::spawn_blocking(move || crate::tools::web::validate_url_sync(&url_for_validation))
        .await
        .ok()?
        .ok()?;

    let resp = get_http_client().get(parsed_url).send().await.ok()?;
    let content_len_ok = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
        .is_none_or(|len| len <= MAX_MULTIMODAL_IMAGE_BYTES);
    content_len_ok.then_some(())?;

    let header_mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .and_then(|ct| ct.split(';').next())
        .map(str::trim)
        .filter(|ct| ct.starts_with("image/"))
        .map(str::to_string);

    let bytes = resp.bytes().await.ok()?;
    (bytes.len() <= MAX_MULTIMODAL_IMAGE_BYTES).then_some(())?;
    let data = bytes.to_vec();
    let mime = detect_image_mime(&data)
        .map(str::to_string)
        .or(header_mime)?;
    Some((mime, data))
}

fn load_local_image(raw_target: &str) -> Option<(String, Vec<u8>)> {
    let clean_path = raw_target.trim_start_matches("file://");
    let resolved_path = crate::config::loader::resolve_path(clean_path);
    crate::config::loader::verify_safe_path(&resolved_path).ok()?;
    (resolved_path.exists() && resolved_path.is_file()).then_some(())?;
    let metadata = std::fs::metadata(&resolved_path).ok()?;
    (metadata.len() <= MAX_MULTIMODAL_IMAGE_BYTES as u64).then_some(())?;
    let data = std::fs::read(&resolved_path).ok()?;
    (data.len() <= MAX_MULTIMODAL_IMAGE_BYTES).then_some(())?;
    let mime = detect_image_mime(&data)?.to_string();
    Some((mime, data))
}

pub async fn parse_multimodal_content(text: &str) -> Vec<ContentPart> {
    let mut parts = Vec::new();
    let mut cursor = 0;

    while let Some((rel_start, rel_end, path_or_url)) = find_next_markdown_image(&text[cursor..]) {
        let match_start = cursor + rel_start;
        let match_end = cursor + rel_end;
        let before = &text[cursor..match_start];
        match before.is_empty() {
            true => {}
            false => parts.push(ContentPart::Text(before.to_string())),
        }

        let is_remote =
            path_or_url.starts_with("http://") || path_or_url.starts_with("https://");
        let resolved = match is_remote {
            true => fetch_remote_image(path_or_url).await,
            false => load_local_image(path_or_url),
        };

        match resolved {
            Some((mime_type, data)) => {
                use base64::{engine::general_purpose, Engine as _};
                let base64_data = general_purpose::STANDARD.encode(data);
                parts.push(ContentPart::Image {
                    mime_type,
                    base64_data,
                });
            }
            None => {
                parts.push(ContentPart::Text(text[match_start..match_end].to_string()));
            }
        }

        cursor = match_end;
    }

    let remaining = &text[cursor..];
    match remaining.is_empty() {
        true => {}
        false => parts.push(ContentPart::Text(remaining.to_string())),
    }

    match parts.is_empty() && !text.is_empty() {
        true => parts.push(ContentPart::Text(text.to_string())),
        false => {}
    }

    parts
}

pub fn sanitize_tool_messages(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .fold(Vec::with_capacity(messages.len()), |mut sanitized, msg| {
            match msg.role.as_str() {
                "tool" => match msg.extra.get("tool_call_id").and_then(|v| v.as_str()) {
                    None => {
                        tracing::warn!("Discarded tool message missing tool_call_id");
                    }
                    Some(id) => {
                        let matched_tool_name = sanitized
                            .iter()
                            .rev()
                            .filter(|prev| prev.role == "assistant")
                            .find_map(|prev| {
                                prev.extra
                                    .get("tool_calls")
                                    .and_then(|v| v.as_array())
                                    .and_then(|calls| {
                                        calls
                                            .iter()
                                            .find(|call| {
                                                call.get("id").and_then(|v| v.as_str())
                                                    == Some(id)
                                            })
                                            .map(|call| {
                                                call.get("function")
                                                    .and_then(|f| f.get("name"))
                                                    .and_then(|v| v.as_str())
                                                    .map(str::to_string)
                                            })
                                    })
                            });
                        match matched_tool_name {
                            Some(tool_name) => {
                                let mut sanitized_msg = msg.clone();
                                let needs_name = sanitized_msg
                                    .extra
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .is_none_or(|name| name.trim().is_empty());
                                match needs_name {
                                    true => {
                                        let resolved_name =
                                            tool_name.unwrap_or_else(|| "tool".to_string());
                                        sanitized_msg.extra.insert(
                                            "name".to_string(),
                                            serde_json::Value::String(resolved_name),
                                        );
                                    }
                                    false => {}
                                }
                                sanitized.push(sanitized_msg);
                            }
                            None => {
                                tracing::warn!(
                                    "Discarded orphaned tool message with tool_call_id: {}",
                                    id
                                );
                            }
                        }
                    }
                },
                _ => sanitized.push(msg.clone()),
            }
            sanitized
        })
}

const VISION_EXACT_MODELS: &[&str] = &["mivi"];
const VISION_PREFIX_MODELS: &[&str] = &["mivi/", "o3"];
const VISION_SUBSTRING_MODELS: &[&str] = &[
    "gpt-4o",
    "gpt-4-turbo",
    "gpt-4-vision-preview",
    "claude-3",
    "claude-4",
    "gemini",
    "paligemma",
    "gemma-4",
    "omni",
    "pixtral",
    "qvq",
    "qwen-vl",
    "deepseek-vl",
];
const VISION_TOKEN_KEYWORDS: &[&str] = &["vision", "vl", "vlm", "multimodal"];

pub fn model_supports_vision(model: &str) -> bool {
    let m = model.to_lowercase();
    let exact_match = VISION_EXACT_MODELS.contains(&m.as_str());
    let prefix_match = VISION_PREFIX_MODELS.iter().any(|p| m.starts_with(p));
    let o1_full_match = m.starts_with("o1") && !m.contains("mini") && !m.contains("preview");
    let substring_match = VISION_SUBSTRING_MODELS.iter().any(|s| m.contains(s));
    let token_match = m
        .split(['/', '-', '_', ':'])
        .any(|word| VISION_TOKEN_KEYWORDS.contains(&word));

    exact_match || prefix_match || o1_full_match || substring_match || token_match
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
        match resp.reasoning_content {
            Some(reasoning) => chunks.push(ChatStreamChunk::Reasoning(reasoning)),
            None => {}
        }
        match resp.content {
            Some(content) => chunks.push(ChatStreamChunk::Content(content)),
            None => {}
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
pub mod context_registry;
pub mod model_prefs;
pub mod ollama_manager;
pub mod openai;
pub mod resolver;
pub mod risk;
pub(crate) mod transport;

pub use context_registry::DynamicContextRegistry;

#[cfg(test)]
pub mod mock;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
mod context_registry_tests;

