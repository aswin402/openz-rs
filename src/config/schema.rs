use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimax: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mistral: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "z.ai")]
    pub z_ai: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<ProviderConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "opencode_zen"
    )]
    pub opencode_zen: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cerebras: Option<ProviderConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "google_ai_studio"
    )]
    pub google_ai_studio: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohere: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm7: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sambanova: Option<ProviderConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "huggingface"
    )]
    pub huggingface: Option<ProviderConfig>,
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
    #[serde(default = "default_max_tokens", alias = "max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_bot_name", alias = "bot_name")]
    pub bot_name: String,
    #[serde(default = "default_bot_icon", alias = "bot_icon")]
    pub bot_icon: String,
    #[serde(default = "default_max_messages", alias = "max_messages")]
    pub max_messages: usize,
    #[serde(default = "default_max_tool_iterations", alias = "max_tool_iterations")]
    pub max_tool_iterations: usize,
    #[serde(default = "default_fallback_models", alias = "fallback_models")]
    pub fallback_models: Vec<serde_json::Value>,
    #[serde(default = "default_caveman_mode", alias = "caveman_mode")]
    pub caveman_mode: bool,
    #[serde(default, alias = "context_limit")]
    pub context_limit: Option<usize>,
    #[serde(default = "default_security_mode", alias = "security_mode")]
    pub security_mode: String,
    #[serde(default, alias = "tool_output_limit")]
    pub tool_output_limit: Option<usize>,
    #[serde(default = "default_enable_sandbox", alias = "enable_sandbox")]
    pub enable_sandbox: bool,
    #[serde(default = "default_tool_timeout_secs", alias = "tool_timeout_secs")]
    pub tool_timeout_secs: u64,
    #[serde(default = "default_streaming", alias = "streaming")]
    pub streaming: bool,
    #[serde(
        default = "default_show_tool_router_status",
        alias = "show_tool_router_status"
    )]
    pub show_tool_router_status: bool,
    #[serde(default = "default_min_free_disk_gb", alias = "min_free_disk_gb")]
    pub min_free_disk_gb: f64,
    #[serde(default = "default_allow_network_tools", alias = "allow_network_tools")]
    pub allow_network_tools: bool,
    #[serde(
        default = "default_max_concurrent_process_tools",
        alias = "max_concurrent_process_tools"
    )]
    pub max_concurrent_process_tools: usize,
    #[serde(
        default = "default_warn_before_expensive_tools",
        alias = "warn_before_expensive_tools"
    )]
    pub warn_before_expensive_tools: bool,
}

fn default_enable_sandbox() -> bool {
    false
}

fn default_streaming() -> bool {
    true
}

fn default_show_tool_router_status() -> bool {
    false
}

fn default_min_free_disk_gb() -> f64 {
    2.0
}

fn default_allow_network_tools() -> bool {
    true
}

fn default_max_concurrent_process_tools() -> usize {
    3
}

fn default_warn_before_expensive_tools() -> bool {
    true
}

fn default_tool_timeout_secs() -> u64 {
    120
}

fn default_security_mode() -> String {
    "normal".to_string()
}

fn default_caveman_mode() -> bool {
    true
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
    4096
}
fn default_temperature() -> f32 {
    0.1
}
fn default_bot_name() -> String {
    "openz".to_string()
}
fn default_bot_icon() -> String {
    "◇".to_string()
}
fn default_max_messages() -> usize {
    120
}
fn default_max_tool_iterations() -> usize {
    200
}

fn default_fallback_models() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!("gpt-4o"),
        serde_json::json!("claude-3-5-haiku"),
        serde_json::json!("openrouter/free"),
    ]
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
            fallback_models: default_fallback_models(),
            caveman_mode: true,
            context_limit: None,
            security_mode: default_security_mode(),
            tool_output_limit: None,
            enable_sandbox: default_enable_sandbox(),
            tool_timeout_secs: default_tool_timeout_secs(),
            show_tool_router_status: default_show_tool_router_status(),
            min_free_disk_gb: default_min_free_disk_gb(),
            allow_network_tools: default_allow_network_tools(),
            max_concurrent_process_tools: default_max_concurrent_process_tools(),
            warn_before_expensive_tools: default_warn_before_expensive_tools(),
            streaming: default_streaming(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    #[serde(default)]
    pub start_on_boot: bool,
    #[serde(default)]
    pub start_on_tui: bool,
}

fn default_ws_port() -> u16 {
    8765
}
fn default_ws_host() -> String {
    "127.0.0.1".to_string()
}
fn default_wa_webhook_port() -> u16 {
    8090
}
fn default_wa_verify_token() -> String {
    "openz".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelegramChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscordChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WhatsAppChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub phone_number_id: String,
    #[serde(default = "default_wa_webhook_port")]
    pub webhook_port: u16,
    #[serde(default = "default_wa_verify_token")]
    pub verify_token: String,
}

fn default_imap_port() -> u16 {
    993
}
fn default_smtp_port() -> u16 {
    465
}
fn default_poll_interval() -> u64 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmailChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub imap_server: String,
    #[serde(default = "default_imap_port")]
    pub imap_port: u16,
    #[serde(default)]
    pub smtp_server: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelsConfig {
    #[serde(default)]
    pub websocket: Option<WebSocketChannelConfig>,
    #[serde(default)]
    pub telegram: Option<TelegramChannelConfig>,
    #[serde(default)]
    pub discord: Option<DiscordChannelConfig>,
    #[serde(default)]
    pub whatsapp: Option<WhatsAppChannelConfig>,
    #[serde(default)]
    pub email: Option<EmailChannelConfig>,
    #[serde(flatten)]
    pub others: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerConfig {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsConfig {
    #[serde(default = "default_embeddings_mode")]
    pub mode: String, // "local", "cloud", "cloud_only"
    #[serde(default)]
    pub preferred_provider: Option<String>,
}

fn default_embeddings_mode() -> String {
    "local".to_string()
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        EmbeddingsConfig {
            mode: default_embeddings_mode(),
            preferred_provider: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfig {
    #[serde(default = "default_workspace_skills_enabled", alias = "workspace_skills_enabled")]
    pub workspace_skills_enabled: bool,
    #[serde(default, alias = "external_dirs")]
    pub external_dirs: Vec<String>,
    #[serde(default = "default_skill_write_approval", alias = "write_approval")]
    pub write_approval: bool,
}

fn default_workspace_skills_enabled() -> bool {
    true
}

fn default_skill_write_approval() -> bool {
    false
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            workspace_skills_enabled: default_workspace_skills_enabled(),
            external_dirs: Vec::new(),
            write_approval: default_skill_write_approval(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub channels: ChannelsConfig,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub embeddings: Option<EmbeddingsConfig>,
    #[serde(default)]
    pub skills: SkillsConfig,
}

impl Default for ChannelsConfig {
    fn default() -> Self {
        ChannelsConfig {
            websocket: Some(WebSocketChannelConfig {
                enabled: true,
                port: default_ws_port(),
                host: default_ws_host(),
                start_on_boot: false,
                start_on_tui: false,
            }),
            telegram: Some(TelegramChannelConfig {
                enabled: false,
                bot_token: String::new(),
            }),
            discord: Some(DiscordChannelConfig {
                enabled: false,
                bot_token: String::new(),
            }),
            whatsapp: Some(WhatsAppChannelConfig {
                enabled: false,
                api_key: String::new(),
                phone_number_id: String::new(),
                webhook_port: default_wa_webhook_port(),
                verify_token: default_wa_verify_token(),
            }),
            email: Some(EmailChannelConfig {
                enabled: false,
                imap_server: String::new(),
                imap_port: default_imap_port(),
                smtp_server: String::new(),
                smtp_port: default_smtp_port(),
                username: String::new(),
                password: String::new(),
                poll_interval_secs: default_poll_interval(),
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
            mcp_servers: HashMap::new(),
            embeddings: Some(EmbeddingsConfig::default()),
            skills: SkillsConfig::default(),
        }
    }
}

// ── Shared provider config resolution ─────────────────────────────────────
// Single source of truth for resolving provider API key + base URL.
// Used by: resolver.rs, channels/mod.rs (fetch_provider_models), cli/builder.rs.
impl Config {
    /// Resolve API key and base URL for a provider from config + env vars.
    /// Returns `(api_key, api_base)` — ollama may return empty key.
    pub fn resolve_provider_config(&self, provider_name: &str) -> (String, String) {
        match provider_name {
            "anthropic" => {
                let p = self.providers.anthropic.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.anthropic.com".to_string());
                (key, base)
            }
            "openai" => {
                let p = self.providers.openai.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
                (key, base)
            }
            "openrouter" => {
                let p = self.providers.openrouter.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
                (key, base)
            }
            "deepseek" => {
                let p = self.providers.deepseek.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string());
                (key, base)
            }
            "groq" => {
                let p = self.providers.groq.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("GROQ_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string());
                (key, base)
            }
            "ollama_local" => (String::new(), "http://localhost:11434/v1".to_string()),
            "ollama" => {
                let p = self.providers.ollama.as_ref();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
                (String::new(), base)
            }
            "minimax" => {
                let p = self.providers.minimax.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.minimax.io/v1".to_string());
                (key, base)
            }
            "mistral" => {
                let p = self.providers.mistral.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("MISTRAL_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string());
                (key, base)
            }
            "z.ai" | "z_ai" => {
                let p = self.providers.z_ai.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("Z_AI_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.z.ai/api/paas/v4/".to_string());
                (key, base)
            }
            "nvidia" => {
                let p = self.providers.nvidia.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("NVIDIA_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://integrate.api.nvidia.com/v1".to_string());
                (key, base)
            }
            "opencode_zen" | "opencode zen" => {
                let p = self.providers.opencode_zen.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("OPENCODE_ZEN_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://opencode.ai/zen/v1".to_string());
                (key, base)
            }
            "cerebras" => {
                let p = self.providers.cerebras.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("CEREBRAS_API_KEY").ok())
                    .or_else(|| std::env::var("CEBRAS_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.cerebras.ai/v1".to_string());
                (key, base)
            }
            "google_ai_studio" | "google ai studio" => {
                let p = self.providers.google_ai_studio.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("GOOGLE_AI_STUDIO_API_KEY").ok())
                    .unwrap_or_default();
                let base = p.and_then(|x| x.api_base.clone()).unwrap_or_else(|| {
                    "https://generativelanguage.googleapis.com/v1beta/openai/".to_string()
                });
                (key, base)
            }
            "cohere" => {
                let p = self.providers.cohere.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("COHERE_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.cohere.com/v1".to_string());
                (key, base)
            }
            "llm7" => {
                let p = self.providers.llm7.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("LLM7_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://token.llm7.io/v1".to_string());
                (key, base)
            }
            "sambanova" => {
                let p = self.providers.sambanova.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("SAMBANOVA_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api.sambanova.ai/v1".to_string());
                (key, base)
            }
            "huggingface" => {
                let p = self.providers.huggingface.as_ref();
                let key = p
                    .and_then(|x| x.api_key.clone())
                    .or_else(|| std::env::var("HUGGINGFACE_API_KEY").ok())
                    .unwrap_or_default();
                let base = p
                    .and_then(|x| x.api_base.clone())
                    .unwrap_or_else(|| "https://api-inference.huggingface.co/v1".to_string());
                (key, base)
            }
            _ => (String::new(), String::new()),
        }
    }

    pub fn is_provider_configured(&self, provider_name: &str) -> bool {
        if provider_name == "ollama_local" {
            return true;
        }
        let p_opt = match provider_name {
            "anthropic" => &self.providers.anthropic,
            "openai" => &self.providers.openai,
            "openrouter" => &self.providers.openrouter,
            "deepseek" => &self.providers.deepseek,
            "groq" => &self.providers.groq,
            "ollama" => &self.providers.ollama,
            "minimax" => &self.providers.minimax,
            "mistral" => &self.providers.mistral,
            "z.ai" => &self.providers.z_ai,
            "nvidia" => &self.providers.nvidia,
            "opencode_zen" => &self.providers.opencode_zen,
            "cerebras" => &self.providers.cerebras,
            "google_ai_studio" => &self.providers.google_ai_studio,
            "cohere" => &self.providers.cohere,
            "llm7" => &self.providers.llm7,
            "sambanova" => &self.providers.sambanova,
            "huggingface" => &self.providers.huggingface,
            _ => return false,
        };
        if provider_name == "ollama" {
            p_opt.is_some()
        } else if let Some(p) = p_opt {
            p.api_key
                .as_ref()
                .map(|k| !k.trim().is_empty())
                .unwrap_or(false)
        } else {
            false
        }
    }

    pub fn is_provider_available(&self, provider_name: &str) -> bool {
        if self.is_provider_configured(provider_name) {
            return true;
        }
        if provider_name == "cerebras" {
            return std::env::var("CEREBRAS_API_KEY").is_ok()
                || std::env::var("CEBRAS_API_KEY").is_ok();
        }
        let env_var = match provider_name {
            "anthropic" => "ANTHROPIC_API_KEY",
            "openai" => "OPENAI_API_KEY",
            "openrouter" => "OPENROUTER_API_KEY",
            "deepseek" => "DEEPSEEK_API_KEY",
            "groq" => "GROQ_API_KEY",
            "minimax" => "MINIMAX_API_KEY",
            "mistral" => "MISTRAL_API_KEY",
            "z.ai" => "Z_AI_API_KEY",
            "nvidia" => "NVIDIA_API_KEY",
            "opencode_zen" => "OPENCODE_ZEN_API_KEY",
            "google_ai_studio" => "GOOGLE_AI_STUDIO_API_KEY",
            "cohere" => "COHERE_API_KEY",
            "llm7" => "LLM7_API_KEY",
            "sambanova" => "SAMBANOVA_API_KEY",
            "huggingface" => "HUGGINGFACE_API_KEY",
            _ => "",
        };
        if !env_var.is_empty() && std::env::var(env_var).is_ok() {
            return true;
        }
        false
    }

    pub fn get_dynamic_fallbacks(&self, subagent_name: &str) -> Vec<String> {
        let is_vision = subagent_name == "vision_agent";
        let mut fallbacks = Vec::new();

        let providers_in_order = &[
            "google_ai_studio",
            "anthropic",
            "openai",
            "deepseek",
            "groq",
            "nvidia",
            "openrouter",
            "opencode_zen",
            "z.ai",
            "mistral",
            "cerebras",
            "minimax",
            "ollama",
        ];

        for &prov in providers_in_order {
            if self.is_provider_available(prov) {
                let model_name = match prov {
                    "google_ai_studio" => "google_ai_studio/gemini-2.5-flash".to_string(),
                    "anthropic" => "anthropic/claude-3-5-sonnet".to_string(),
                    "openai" => "openai/gpt-4o-mini".to_string(),
                    "deepseek" => {
                        if is_vision {
                            continue;
                        } else {
                            "deepseek/deepseek-chat".to_string()
                        }
                    }
                    "groq" => {
                        if is_vision {
                            continue;
                        } else {
                            "groq/llama-3.3-70b-versatile".to_string()
                        }
                    }
                    "nvidia" => {
                        if is_vision {
                            "nvidia/meta/llama-3.2-90b-vision-instruct".to_string()
                        } else {
                            "nvidia/meta/llama-3.3-70b-instruct".to_string()
                        }
                    }
                    "openrouter" => {
                        if is_vision {
                            "openrouter/google/gemini-2.0-flash-exp:free".to_string()
                        } else {
                            "openrouter/free".to_string()
                        }
                    }
                    "opencode_zen" => "opencode_zen/deepseek-v4-flash-free".to_string(),
                    "z.ai" => "z.ai/glm-4.7-flash".to_string(),
                    "mistral" => {
                        if is_vision {
                            "mistral/pixtral-large-latest".to_string()
                        } else {
                            "mistral/mistral-large-latest".to_string()
                        }
                    }
                    "cerebras" => {
                        if is_vision {
                            continue;
                        } else {
                            "cerebras/llama-3.3-70b".to_string()
                        }
                    }
                    "minimax" => {
                        if is_vision {
                            continue;
                        } else {
                            "minimax/MiniMax-M3".to_string()
                        }
                    }
                    "ollama" => {
                        if is_vision {
                            "ollama/llava".to_string()
                        } else {
                            "ollama/llama3".to_string()
                        }
                    }
                    _ => continue,
                };

                if !fallbacks.contains(&model_name) {
                    fallbacks.push(model_name);
                }
            }
        }

        fallbacks
    }
}
