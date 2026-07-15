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
    300
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
    #[serde(
        default = "default_workspace_skills_enabled",
        alias = "workspace_skills_enabled"
    )]
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

struct ProviderDef {
    names: &'static [&'static str],
    env_keys: &'static [&'static str],
    default_base: &'static str,
    config: fn(&ProvidersConfig) -> Option<&ProviderConfig>,
    local: bool,
}

fn provider_openai(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.openai.as_ref()
}
fn provider_anthropic(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.anthropic.as_ref()
}
fn provider_openrouter(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.openrouter.as_ref()
}
fn provider_deepseek(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.deepseek.as_ref()
}
fn provider_groq(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.groq.as_ref()
}
fn provider_ollama(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.ollama.as_ref()
}
fn provider_minimax(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.minimax.as_ref()
}
fn provider_mistral(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.mistral.as_ref()
}
fn provider_z_ai(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.z_ai.as_ref()
}
fn provider_nvidia(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.nvidia.as_ref()
}
fn provider_opencode_zen(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.opencode_zen.as_ref()
}
fn provider_cerebras(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.cerebras.as_ref()
}
fn provider_google_ai_studio(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.google_ai_studio.as_ref()
}
fn provider_cohere(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.cohere.as_ref()
}
fn provider_llm7(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.llm7.as_ref()
}
fn provider_sambanova(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.sambanova.as_ref()
}
fn provider_huggingface(providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    providers.huggingface.as_ref()
}
fn provider_none(_providers: &ProvidersConfig) -> Option<&ProviderConfig> {
    None
}

const PROVIDER_DEFS: &[ProviderDef] = &[
    ProviderDef {
        names: &["anthropic"],
        env_keys: &["ANTHROPIC_API_KEY"],
        default_base: "https://api.anthropic.com",
        config: provider_anthropic,
        local: false,
    },
    ProviderDef {
        names: &["openai"],
        env_keys: &["OPENAI_API_KEY"],
        default_base: "https://api.openai.com/v1",
        config: provider_openai,
        local: false,
    },
    ProviderDef {
        names: &["openrouter"],
        env_keys: &["OPENROUTER_API_KEY"],
        default_base: "https://openrouter.ai/api/v1",
        config: provider_openrouter,
        local: false,
    },
    ProviderDef {
        names: &["deepseek"],
        env_keys: &["DEEPSEEK_API_KEY"],
        default_base: "https://api.deepseek.com/v1",
        config: provider_deepseek,
        local: false,
    },
    ProviderDef {
        names: &["groq"],
        env_keys: &["GROQ_API_KEY"],
        default_base: "https://api.groq.com/openai/v1",
        config: provider_groq,
        local: false,
    },
    ProviderDef {
        names: &["ollama_local"],
        env_keys: &[],
        default_base: "http://localhost:11434/v1",
        config: provider_none,
        local: true,
    },
    ProviderDef {
        names: &["ollama"],
        env_keys: &[],
        default_base: "http://localhost:11434/v1",
        config: provider_ollama,
        local: true,
    },
    ProviderDef {
        names: &["minimax"],
        env_keys: &["MINIMAX_API_KEY"],
        default_base: "https://api.minimax.io/v1",
        config: provider_minimax,
        local: false,
    },
    ProviderDef {
        names: &["mistral"],
        env_keys: &["MISTRAL_API_KEY"],
        default_base: "https://api.mistral.ai/v1",
        config: provider_mistral,
        local: false,
    },
    ProviderDef {
        names: &["z.ai", "z_ai"],
        env_keys: &["Z_AI_API_KEY"],
        default_base: "https://api.z.ai/api/paas/v4/",
        config: provider_z_ai,
        local: false,
    },
    ProviderDef {
        names: &["nvidia"],
        env_keys: &["NVIDIA_API_KEY"],
        default_base: "https://integrate.api.nvidia.com/v1",
        config: provider_nvidia,
        local: false,
    },
    ProviderDef {
        names: &["opencode_zen", "opencode zen"],
        env_keys: &["OPENCODE_ZEN_API_KEY"],
        default_base: "https://opencode.ai/zen/v1",
        config: provider_opencode_zen,
        local: false,
    },
    ProviderDef {
        names: &["cerebras"],
        env_keys: &["CEREBRAS_API_KEY", "CEBRAS_API_KEY"],
        default_base: "https://api.cerebras.ai/v1",
        config: provider_cerebras,
        local: false,
    },
    ProviderDef {
        names: &["google_ai_studio", "google ai studio"],
        env_keys: &["GOOGLE_AI_STUDIO_API_KEY"],
        default_base: "https://generativelanguage.googleapis.com/v1beta/openai/",
        config: provider_google_ai_studio,
        local: false,
    },
    ProviderDef {
        names: &["cohere"],
        env_keys: &["COHERE_API_KEY"],
        default_base: "https://api.cohere.com/v1",
        config: provider_cohere,
        local: false,
    },
    ProviderDef {
        names: &["llm7"],
        env_keys: &["LLM7_API_KEY"],
        default_base: "https://token.llm7.io/v1",
        config: provider_llm7,
        local: false,
    },
    ProviderDef {
        names: &["sambanova"],
        env_keys: &["SAMBANOVA_API_KEY"],
        default_base: "https://api.sambanova.ai/v1",
        config: provider_sambanova,
        local: false,
    },
    ProviderDef {
        names: &["huggingface"],
        env_keys: &["HUGGINGFACE_API_KEY"],
        default_base: "https://api-inference.huggingface.co/v1",
        config: provider_huggingface,
        local: false,
    },
];

fn provider_def(provider_name: &str) -> Option<&'static ProviderDef> {
    PROVIDER_DEFS
        .iter()
        .find(|def| def.names.iter().any(|name| *name == provider_name))
}

fn configured_key(provider: Option<&ProviderConfig>) -> Option<String> {
    provider
        .and_then(|p| p.api_key.clone())
        .filter(|key| !key.trim().is_empty())
}

fn env_key(env_keys: &[&str]) -> Option<String> {
    env_keys
        .iter()
        .find_map(|env| std::env::var(env).ok().filter(|key| !key.trim().is_empty()))
}

// ── Shared provider config resolution ─────────────────────────────────────
// Single source of truth for resolving provider API key + base URL.
// Used by: resolver.rs, channels/mod.rs (fetch_provider_models), cli/builder.rs.
impl Config {
    /// Resolve API key and base URL for a provider from config + env vars.
    /// Returns `(api_key, api_base)` — local providers may return an empty key.
    pub fn resolve_provider_config(&self, provider_name: &str) -> (String, String) {
        let Some(def) = provider_def(provider_name) else {
            return (String::new(), String::new());
        };
        let provider = (def.config)(&self.providers);
        let key = if def.local {
            String::new()
        } else {
            configured_key(provider)
                .or_else(|| env_key(def.env_keys))
                .unwrap_or_default()
        };
        let base = provider
            .and_then(|p| p.api_base.clone())
            .unwrap_or_else(|| def.default_base.to_string());
        (key, base)
    }

    pub fn is_provider_configured(&self, provider_name: &str) -> bool {
        let Some(def) = provider_def(provider_name) else {
            return false;
        };
        if provider_name == "ollama_local" {
            return true;
        }
        let provider = (def.config)(&self.providers);
        if def.local {
            provider.is_some()
        } else {
            configured_key(provider).is_some()
        }
    }

    pub fn is_provider_available(&self, provider_name: &str) -> bool {
        let Some(def) = provider_def(provider_name) else {
            return false;
        };
        self.is_provider_configured(provider_name) || env_key(def.env_keys).is_some()
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

#[cfg(test)]
mod provider_resolution_tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn blank_config() -> Config {
        Config {
            providers: ProvidersConfig::default(),
            agents: AgentsConfig::default(),
            channels: ChannelsConfig::default(),
            mcp_servers: HashMap::new(),
            embeddings: Some(EmbeddingsConfig::default()),
            skills: SkillsConfig::default(),
        }
    }

    #[test]
    fn resolve_provider_config_uses_table_aliases_and_defaults() {
        let _guard = env_lock().lock().unwrap();
        std::env::remove_var("Z_AI_API_KEY");
        let mut config = blank_config();
        config.providers.z_ai = Some(ProviderConfig {
            api_key: Some("z-key".to_string()),
            api_base: None,
            extra: HashMap::new(),
        });

        assert_eq!(
            config.resolve_provider_config("z_ai"),
            (
                "z-key".to_string(),
                "https://api.z.ai/api/paas/v4/".to_string()
            )
        );
    }

    #[test]
    fn provider_available_uses_env_keys_without_marking_unconfigured_local_available() {
        let _guard = env_lock().lock().unwrap();
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("CEREBRAS_API_KEY");
        std::env::remove_var("CEBRAS_API_KEY");
        let config = blank_config();

        assert!(config.is_provider_available("ollama_local"));
        assert!(!config.is_provider_available("ollama"));
        assert!(!config.is_provider_available("openai"));

        std::env::set_var("OPENAI_API_KEY", "test-key");
        assert!(config.is_provider_available("openai"));
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn cerebras_legacy_env_key_still_works() {
        let _guard = env_lock().lock().unwrap();
        std::env::remove_var("CEREBRAS_API_KEY");
        std::env::set_var("CEBRAS_API_KEY", "legacy-key");
        let config = blank_config();

        assert_eq!(
            config.resolve_provider_config("cerebras").0,
            "legacy-key".to_string()
        );
        assert!(config.is_provider_available("cerebras"));
        std::env::remove_var("CEBRAS_API_KEY");
    }
}
