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
