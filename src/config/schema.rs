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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimax: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mistral: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "z.ai")]
    pub z_ai: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "opencode_zen")]
    pub opencode_zen: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cerebres: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "google_ai_studio")]
    pub google_ai_studio: Option<ProviderConfig>,
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
        serde_json::json!("openrouter/auto"),
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
pub struct Config {
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub channels: ChannelsConfig,
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
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
            minimax: None,
            mistral: None,
            z_ai: None,
            nvidia: None,
            opencode_zen: None,
            cerebres: None,
            google_ai_studio: None,
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
        let mut mcp_servers = HashMap::new();

        mcp_servers.insert("sequential-thinking".to_string(), McpServerConfig {
            command: "/home/aswin/programming/vscode/myProjects/target/release/mcp-server-sequential-thinking".to_string(),
            args: vec![],
            enabled: true,
        });

        mcp_servers.insert("memory".to_string(), McpServerConfig {
            command: "/home/aswin/programming/vscode/myProjects/target/release/openmemory_rs".to_string(),
            args: vec!["--grpc".to_string(), "50051".to_string()],
            enabled: true,
        });

        let office_bin = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("opendocswork-mcp");
            if p.exists() { p.to_string_lossy().to_string() } else { "opendocswork-mcp".to_string() }
        } else {
            "opendocswork-mcp".to_string()
        };

        mcp_servers.insert("office".to_string(), McpServerConfig {
            command: office_bin,
            args: vec!["--transport".to_string(), "stdio".to_string()],
            enabled: true,
        });

        let spreadsheet_bin = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("spreadsheet-mcp");
            if p.exists() { p.to_string_lossy().to_string() } else { "spreadsheet-mcp".to_string() }
        } else {
            "spreadsheet-mcp".to_string()
        };

        mcp_servers.insert("spreadsheet".to_string(), McpServerConfig {
            command: spreadsheet_bin,
            args: vec![],
            enabled: true,
        });

        let just_bin = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("just-mcp");
            if p.exists() { p.to_string_lossy().to_string() } else { "just-mcp".to_string() }
        } else {
            "just-mcp".to_string()
        };

        mcp_servers.insert("just".to_string(), McpServerConfig {
            command: just_bin,
            args: vec!["--stdio".to_string()],
            enabled: true,
        });

        let headroom_bin = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("headroom-mcp");
            if p.exists() { p.to_string_lossy().to_string() } else { "headroom-mcp".to_string() }
        } else {
            "headroom-mcp".to_string()
        };

        mcp_servers.insert("headroom".to_string(), McpServerConfig {
            command: headroom_bin,
            args: vec![],
            enabled: true,
        });

        let docs_mcp_bin = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("openz-docs-mcp");
            if p.exists() { p.to_string_lossy().to_string() } else { "openz-docs-mcp".to_string() }
        } else {
            "openz-docs-mcp".to_string()
        };

        mcp_servers.insert("docs".to_string(), McpServerConfig {
            command: docs_mcp_bin,
            args: vec![],
            enabled: true,
        });

        let github_mcp_bin = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("openz-github-mcp");
            if p.exists() { p.to_string_lossy().to_string() } else { "openz-github-mcp".to_string() }
        } else {
            "openz-github-mcp".to_string()
        };

        mcp_servers.insert("github".to_string(), McpServerConfig {
            command: github_mcp_bin,
            args: vec![],
            enabled: true,
        });

        let db_mcp_bin = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("database-mcp");
            if p.exists() { p.to_string_lossy().to_string() } else { "database-mcp".to_string() }
        } else {
            "database-mcp".to_string()
        };

        mcp_servers.insert("database".to_string(), McpServerConfig {
            command: db_mcp_bin,
            args: vec!["stdio".to_string()],
            enabled: true,
        });

        let chromewright_bin = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("chromewright");
            if p.exists() { p.to_string_lossy().to_string() } else { "chromewright".to_string() }
        } else {
            "chromewright".to_string()
        };

        mcp_servers.insert("browser".to_string(), McpServerConfig {
            command: chromewright_bin,
            args: vec!["--headless".to_string()],
            enabled: true,
        });

        let sediment_bin = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("sediment");
            if p.exists() { p.to_string_lossy().to_string() } else { "sediment".to_string() }
        } else {
            "sediment".to_string()
        };

        mcp_servers.insert("sediment".to_string(), McpServerConfig {
            command: sediment_bin,
            args: vec![],
            enabled: true,
        });

        Config {
            providers: ProvidersConfig::default(),
            agents: AgentsConfig::default(),
            channels: ChannelsConfig::default(),
            mcp_servers,
            extra: serde_json::Map::new(),
        }
    }
}

impl Config {
    pub fn is_provider_configured(&self, provider_name: &str) -> bool {
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
            "cerebres" => &self.providers.cerebres,
            "google_ai_studio" => &self.providers.google_ai_studio,
            _ => return false,
        };
        if provider_name == "ollama" {
            p_opt.is_some()
        } else if let Some(p) = p_opt {
            p.api_key.as_ref().map(|k| !k.trim().is_empty()).unwrap_or(false)
        } else {
            false
        }
    }

    pub fn is_provider_available(&self, provider_name: &str) -> bool {
        if self.is_provider_configured(provider_name) {
            return true;
        }
        if provider_name == "cerebres" {
            return std::env::var("CEREBRES_API_KEY").is_ok() || std::env::var("CEBRAS_API_KEY").is_ok();
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
            "cerebres",
            "minimax",
            "ollama",
        ];

        for &prov in providers_in_order {
            if self.is_provider_available(prov) {
                let model_name = match prov {
                    "google_ai_studio" => {
                        "google_ai_studio/gemini-2.5-flash".to_string()
                    }
                    "anthropic" => {
                        "anthropic/claude-3-5-sonnet".to_string()
                    }
                    "openai" => {
                        "openai/gpt-4o-mini".to_string()
                    }
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
                            "openrouter/auto".to_string()
                        }
                    }
                    "opencode_zen" => {
                        "opencode_zen/gpt-5.5".to_string()
                    }
                    "z.ai" => {
                        "z.ai/glm-4.7-flash".to_string()
                    }
                    "mistral" => {
                        if is_vision {
                            "mistral/pixtral-large-latest".to_string()
                        } else {
                            "mistral/mistral-large-latest".to_string()
                        }
                    }
                    "cerebres" => {
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

