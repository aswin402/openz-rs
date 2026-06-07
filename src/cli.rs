use crate::config::loader::{load_config, save_config, resolve_path};
use crate::config::schema::{Config, ProviderConfig};
use clap::{Parser, Subcommand};
use inquire::{Text, Select, Password};
use anyhow::{Result, anyhow};
use std::sync::Arc;
use crate::providers::{openai::OpenAIProvider, anthropic::AnthropicProvider, LLMProvider};
use crate::tools::ToolRegistry;
use crate::tools::filesystem::{ReadFileTool, WriteFileTool, ListDirTool};
use crate::tools::shell::ExecCommandTool;
use crate::tools::web::WebFetchTool;
use crate::session::SessionManager;
use crate::agent::AgentLoop;
use crate::channels::{CliChannel, WsGateway, TelegramChannel};

#[derive(Parser)]
#[command(name = "openz", version = "0.1.0", about = "OpenZ - Rebranded Ultra-Lightweight Personal AI Agent")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Onboard and configure the AI agent settings
    Onboard,
    
    /// Chat with the agent in the terminal
    Agent,
    
    /// Start the WebSocket and WebUI gateway server
    Gateway,

    /// Start the Telegram bot listener
    Telegram,
}

pub async fn run_cli() -> Result<()> {
    let args = CliArgs::parse();
    
    match args.command {
        Command::Onboard => {
            handle_onboard().await?;
        }
        Command::Agent => {
            handle_agent().await?;
        }
        Command::Gateway => {
            handle_gateway().await?;
        }
        Command::Telegram => {
            handle_telegram().await?;
        }
    }
    
    Ok(())
}

async fn handle_onboard() -> Result<()> {
    println!("=== Welcome to the OpenZ Setup Wizard ===");
    
    let providers = vec!["anthropic", "openai", "openrouter", "deepseek", "groq", "ollama"];
    let provider_name = Select::new("Choose an LLM provider:", providers).prompt()?;
    
    let mut api_key = None;
    if provider_name != "ollama" {
        let key = Password::new(&format!("Enter API Key for {}:", provider_name)).prompt()?;
        if !key.trim().is_empty() {
            api_key = Some(key.trim().to_string());
        }
    }
    
    let default_base = match provider_name {
        "anthropic" => "https://api.anthropic.com",
        "openai" => "https://api.openai.com/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        "deepseek" => "https://api.deepseek.com/v1",
        "groq" => "https://api.groq.com/openai/v1",
        "ollama" => "http://localhost:11434/v1",
        _ => "",
    };
    
    let api_base_input = Text::new(&format!("Enter API Base URL [default: {}]:", default_base)).prompt()?;
    let api_base = if api_base_input.trim().is_empty() {
        Some(default_base.to_string())
    } else {
        Some(api_base_input.trim().to_string())
    };
    
    let default_model = match provider_name {
        "anthropic" => "claude-3-5-sonnet-20241022",
        "openai" => "gpt-4o",
        "openrouter" => "google/gemini-2.5-pro",
        "deepseek" => "deepseek-chat",
        "groq" => "llama3-70b-8192",
        "ollama" => "llama3",
        _ => "",
    };
    
    let model_input = Text::new(&format!("Enter LLM Model Name [default: {}]:", default_model)).prompt()?;
    let model = if model_input.trim().is_empty() {
        default_model.to_string()
    } else {
        model_input.trim().to_string()
    };
    
    let bot_name = Text::new("Enter Bot Name [default: openz]:").prompt()?;
    let bot_name = if bot_name.trim().is_empty() { "openz".to_string() } else { bot_name.trim().to_string() };
    
    let bot_icon = Text::new("Enter Bot Icon (Emoji/text) [default: ⚡]:").prompt()?;
    let bot_icon = if bot_icon.trim().is_empty() { "⚡".to_string() } else { bot_icon.trim().to_string() };

    let mut config = Config::default();
    
    let p_config = Some(ProviderConfig {
        api_key,
        api_base,
        api_type: None,
        extra: std::collections::HashMap::new(),
    });
    
    match provider_name {
        "anthropic" => config.providers.anthropic = p_config,
        "openai" => config.providers.openai = p_config,
        "openrouter" => config.providers.openrouter = p_config,
        "deepseek" => config.providers.deepseek = p_config,
        "groq" => config.providers.groq = p_config,
        "ollama" => config.providers.ollama = p_config,
        _ => {}
    }
    
    config.agents.defaults.provider = provider_name.to_string();
    config.agents.defaults.model = model;
    config.agents.defaults.bot_name = bot_name;
    config.agents.defaults.bot_icon = bot_icon;
    
    save_config(&config)?;
    
    let workspace = resolve_path(&config.agents.defaults.workspace);
    if !workspace.exists() {
        std::fs::create_dir_all(&workspace)?;
    }
    
    println!("\n✅ Onboarding complete! Settings saved to {:?}", crate::config::config_path());
    println!("You can now run 'openz agent' to start chatting.");
    
    Ok(())
}

fn build_agent_loop(config: Config) -> Result<AgentLoop> {
    let defaults = &config.agents.defaults;
    let mut provider_name = defaults.provider.clone();
    
    if provider_name == "auto" {
        let model_lower = defaults.model.to_lowercase();
        if model_lower.starts_with("anthropic/") || model_lower.contains("claude") {
            provider_name = "anthropic".to_string();
        } else if model_lower.starts_with("openai/") || model_lower.contains("gpt") {
            provider_name = "openai".to_string();
        } else if model_lower.starts_with("deepseek/") || model_lower.contains("deepseek") {
            provider_name = "deepseek".to_string();
        } else if model_lower.starts_with("groq/") {
            provider_name = "groq".to_string();
        } else if model_lower.starts_with("openrouter/") {
            provider_name = "openrouter".to_string();
        } else if model_lower.starts_with("ollama/") || model_lower.contains("ollama") {
            provider_name = "ollama".to_string();
        } else {
            if config.providers.anthropic.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("ANTHROPIC_API_KEY").is_ok() {
                provider_name = "anthropic".to_string();
            } else if config.providers.openai.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENAI_API_KEY").is_ok() {
                provider_name = "openai".to_string();
            } else if config.providers.deepseek.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("DEEPSEEK_API_KEY").is_ok() {
                provider_name = "deepseek".to_string();
            } else if config.providers.openrouter.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENROUTER_API_KEY").is_ok() {
                provider_name = "openrouter".to_string();
            } else if config.providers.groq.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("GROQ_API_KEY").is_ok() {
                provider_name = "groq".to_string();
            } else {
                provider_name = "openai".to_string();
            }
        }
    }
    
    let (api_key, api_base, model) = match provider_name.as_str() {
        "anthropic" => {
            let p = config.providers.anthropic.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            (key, base, defaults.model.clone())
        }
        "openai" => {
            let p = config.providers.openai.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            (key, base, defaults.model.clone())
        }
        "openrouter" => {
            let p = config.providers.openrouter.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
            (key, base, defaults.model.clone())
        }
        "deepseek" => {
            let p = config.providers.deepseek.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string());
            (key, base, defaults.model.clone())
        }
        "groq" => {
            let p = config.providers.groq.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("GROQ_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string());
            (key, base, defaults.model.clone())
        }
        "ollama" => {
            let p = config.providers.ollama.as_ref();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
            (String::new(), base, defaults.model.clone())
        }
        _ => {
            return Err(anyhow!("Unsupported or unconfigured provider: {}", provider_name));
        }
    };

    if provider_name != "ollama" && api_key.is_empty() {
        return Err(anyhow!(
            "No API key found for provider '{}'. Please run setup wizard first via 'openz onboard' or set the appropriate environment variable (e.g. {}_API_KEY).",
            provider_name,
            provider_name.to_uppercase()
        ));
    }
    
    let provider: Arc<dyn LLMProvider> = if provider_name == "anthropic" {
        Arc::new(AnthropicProvider::new(api_key, api_base, model))
    } else {
        Arc::new(OpenAIProvider::new(api_key, api_base, model))
    };
    
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ReadFileTool));
    registry.register(Box::new(WriteFileTool));
    registry.register(Box::new(ListDirTool));
    registry.register(Box::new(ExecCommandTool));
    registry.register(Box::new(WebFetchTool::new()));
    
    let sessions_dir = resolve_path("~/.openz/sessions");
    let session_manager = SessionManager::new(sessions_dir);
    
    Ok(AgentLoop::new(config, provider, registry, session_manager))
}

async fn handle_agent() -> Result<()> {
    let config = load_config()?;
    let defaults = config.agents.defaults.clone();
    let agent_loop = build_agent_loop(config)?;
    let channel = CliChannel::new(agent_loop, defaults);
    channel.start().await?;
    Ok(())
}

async fn handle_gateway() -> Result<()> {
    let config = load_config()?;
    let ws_config = config.channels.websocket.clone().unwrap_or_else(|| {
        crate::config::schema::WebSocketChannelConfig {
            enabled: true,
            port: 8765,
            host: "127.0.0.1".to_string(),
        }
    });
    
    let agent_loop = build_agent_loop(config)?;
    let gateway = WsGateway::new(ws_config, agent_loop);
    gateway.start().await?;
    Ok(())
}

async fn handle_telegram() -> Result<()> {
    let config = load_config()?;
    let tg_config = config.channels.telegram.clone().unwrap_or_default();
    
    let token = if tg_config.bot_token.is_empty() {
        std::env::var("TELEGRAM_BOT_TOKEN").map_err(|_| anyhow!("TELEGRAM_BOT_TOKEN environment variable or config parameter not set."))?
    } else {
        tg_config.bot_token
    };
    
    let agent_loop = build_agent_loop(config)?;
    let channel = TelegramChannel::new(token, agent_loop);
    channel.start().await?;
    
    Ok(())
}
