use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_type: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deepseek: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groq: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ollama: Option<ProviderConfig>,
    #[serde(flatten)]
    pub others: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefaults {
    #[serde(default = "default_workspace")]
    pub workspace: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_bot_name")]
    pub bot_name: String,
    #[serde(default = "default_bot_icon")]
    pub bot_icon: String,
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,
    #[serde(default = "default_max_tool_iterations")]
    pub max_tool_iterations: usize,
    #[serde(default)]
    pub fallback_models: Vec<serde_json::Value>,
}

fn default_workspace() -> String {
    "~/.openz/workspace".to_string()
}
fn default_model() -> String {
    "anthropic/claude-3-5-sonnet".to_string()
}
fn default_provider() -> String {
    "auto".to_string()
}
fn default_max_tokens() -> usize {
    1024
}
fn default_temperature() -> f32 {
    0.1
}
fn default_bot_name() -> String {
    "openz".to_string()
}
fn default_bot_icon() -> String {
    "⚡".to_string()
}
fn default_max_messages() -> usize {
    120
}
fn default_max_tool_iterations() -> usize {
    200
}

impl Default for AgentDefaults {
    fn default() -> Self {
        AgentDefaults {
            workspace: default_workspace(),
            model: default_model(),
            provider: default_provider(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            bot_name: default_bot_name(),
            bot_icon: default_bot_icon(),
            max_messages: default_max_messages(),
            max_tool_iterations: default_max_tool_iterations(),
            fallback_models: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    #[serde(default)]
    pub defaults: AgentDefaults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ws_port")]
    pub port: u16,
    #[serde(default = "default_ws_host")]
    pub host: String,
}

fn default_ws_port() -> u16 {
    8765
}
fn default_ws_host() -> String {
    "127.0.0.1".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelegramChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelsConfig {
    #[serde(default)]
    pub websocket: Option<WebSocketChannelConfig>,
    #[serde(default)]
    pub telegram: Option<TelegramChannelConfig>,
    #[serde(flatten)]
    pub others: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub channels: ChannelsConfig,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        ProvidersConfig {
            openai: None,
            anthropic: None,
            openrouter: None,
            deepseek: None,
            groq: None,
            ollama: None,
            others: HashMap::new(),
        }
    }
}

impl Default for AgentsConfig {
    fn default() -> Self {
        AgentsConfig {
            defaults: AgentDefaults::default(),
        }
    }
}

impl Default for ChannelsConfig {
    fn default() -> Self {
        ChannelsConfig {
            websocket: Some(WebSocketChannelConfig {
                enabled: true,
                port: default_ws_port(),
                host: default_ws_host(),
            }),
            telegram: Some(TelegramChannelConfig {
                enabled: false,
                bot_token: String::new(),
            }),
            others: HashMap::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            providers: ProvidersConfig::default(),
            agents: AgentsConfig::default(),
            channels: ChannelsConfig::default(),
        }
    }
}
