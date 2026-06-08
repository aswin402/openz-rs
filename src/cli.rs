use crate::config::loader::{load_config, save_config, resolve_path};
use crate::config::schema::{Config, ProviderConfig, WebSocketChannelConfig};
use clap::{Parser, Subcommand};
use inquire::{Text, Select, Password};
use anyhow::{Result, anyhow};
use std::sync::Arc;
use crate::providers::{openai::OpenAIProvider, anthropic::AnthropicProvider, LLMProvider};
use crate::tools::ToolRegistry;
use crate::tools::filesystem::{ReadFileTool, WriteFileTool, ListDirTool};
use crate::tools::shell::ExecCommandTool;
use crate::tools::web::WebFetchTool;
use crate::tools::subagent::{DelegateTaskTool, OptimizeSubagentTool, CreateSubagentTool, DeleteSubagentTool};
use crate::tools::cron::{ScheduleJobTool, ListJobsTool, RemoveJobTool};
use crate::tools::remote::SendRemoteInputTool;
use crate::session::SessionManager;
use crate::agent::AgentLoop;
use crate::channels::{CliChannel, WsGateway, TelegramChannel, Channel};
use crate::cron::scheduler::start_scheduler;

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

    /// Configure OpenZ settings, providers, and gateway/channels
    Configure,
    
    /// Chat with the agent in the terminal
    Agent,
    
    /// Start the WebSocket and WebUI gateway server
    Gateway,

    /// Start the Telegram bot listener
    Telegram,

    /// Start the Discord bot listener
    Discord,

    /// Start the WhatsApp API/webhook listener
    Whatsapp,

    /// Manage, configure, and design custom subagents
    Subagent,
}

pub async fn run_cli() -> Result<()> {
    let args = CliArgs::parse();
    
    match args.command {
        Command::Onboard => {
            handle_onboard().await?;
        }
        Command::Configure => {
            handle_configure().await?;
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
        Command::Discord => {
            handle_discord().await?;
        }
        Command::Whatsapp => {
            handle_whatsapp().await?;
        }
        Command::Subagent => {
            let config = load_config()?;
            crate::subagents::run_subagent_manager(config).await?;
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

pub async fn build_agent_loop(config: Config) -> Result<AgentLoop> {
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
    
    let sessions_dir = resolve_path("~/.openz/sessions");
    let session_manager = SessionManager::new(sessions_dir);

    let mut registry = ToolRegistry::new_with_context(config.clone(), provider.clone(), session_manager.clone());
    registry.register(std::sync::Arc::new(ReadFileTool));
    registry.register(std::sync::Arc::new(WriteFileTool));
    registry.register(std::sync::Arc::new(ListDirTool));
    registry.register(std::sync::Arc::new(ExecCommandTool));
    registry.register(std::sync::Arc::new(WebFetchTool::new()));
    registry.register(std::sync::Arc::new(DelegateTaskTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
    }));

    registry.register(std::sync::Arc::new(OptimizeSubagentTool {
        config: config.clone(),
        parent_provider: provider.clone(),
    }));

    registry.register(std::sync::Arc::new(CreateSubagentTool));
    registry.register(std::sync::Arc::new(DeleteSubagentTool));

    registry.register(std::sync::Arc::new(ScheduleJobTool));
    registry.register(std::sync::Arc::new(ListJobsTool));
    registry.register(std::sync::Arc::new(RemoveJobTool));
    registry.register(std::sync::Arc::new(SendRemoteInputTool));
    registry.register(std::sync::Arc::new(crate::tools::mcp_manager::ManageMcpTool));
    registry.register(std::sync::Arc::new(crate::tools::grep::GrepSearchTool));
    registry.register(std::sync::Arc::new(crate::tools::git_manager::GitManagerTool));
    registry.register(std::sync::Arc::new(crate::tools::outline::CodeOutlineTool));
    registry.register(std::sync::Arc::new(crate::tools::db_inspector::DbInspectorTool));
    registry.register(std::sync::Arc::new(crate::tools::cargo_manager::CargoManagerTool));
    registry.register(std::sync::Arc::new(crate::tools::clipboard::ClipboardTool));
    registry.register(std::sync::Arc::new(crate::tools::open::OpenTool));
    registry.register(std::sync::Arc::new(crate::tools::watcher::FileWatcherTool));

    // Register configured MCP tools
    for (name, mcp_config) in &config.mcp_servers {
        if mcp_config.enabled {
            println!("🔌 Spawning MCP server '{}' ({} {:?})...", name, mcp_config.command, mcp_config.args);
            match crate::tools::mcp::McpClient::spawn(&mcp_config.command, &mcp_config.args).await {
                Ok(mcp_client) => {
                    match mcp_client.list_tools().await {
                        Ok(tools) => {
                            for t in tools {
                                if let (Some(t_name), Some(desc)) = (t.get("name").and_then(|v| v.as_str()), t.get("description").and_then(|v| v.as_str())) {
                                    let params = t.get("inputSchema").cloned().unwrap_or(serde_json::json!({
                                        "type": "object",
                                        "properties": {}
                                    }));
                                    
                                    println!("   ↳ Registering MCP tool: {}", t_name);
                                    let wrapper = crate::tools::mcp::McpToolWrapper {
                                        client: mcp_client.clone(),
                                        name: t_name.to_string(),
                                        description: desc.to_string(),
                                        parameters: params,
                                    };
                                    registry.register(std::sync::Arc::new(wrapper));
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("⚠️ Failed to list tools from MCP server '{}': {}", name, e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to spawn MCP server '{}': {}", name, e);
                }
            }
        }
    }
    
    Ok(AgentLoop::new(config, provider, registry, session_manager))
}

async fn handle_agent() -> Result<()> {
    let config = load_config()?;
    let defaults = config.agents.defaults.clone();
    start_scheduler(config.clone());
    
    // Check if background gateway needs to start when TUI starts
    if let Some(ref ws_config) = config.channels.websocket {
        if ws_config.enabled && ws_config.start_on_tui {
            let bg_config = config.clone();
            tokio::spawn(async move {
                if let Ok(agent_loop) = build_agent_loop(bg_config.clone()).await {
                    let gateway = WsGateway::new(bg_config.channels.websocket.clone().unwrap(), agent_loop);
                    println!("🚀 Spawning background gateway (startOnTui enabled)...");
                    if let Err(e) = gateway.start().await {
                        eprintln!("⚠️ Background gateway error: {}", e);
                    }
                }
            });
        }
    }

    let agent_loop = build_agent_loop(config).await?;
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
            start_on_boot: false,
            start_on_tui: false,
        }
    });
    
    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config).await?;
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
    
    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config).await?;
    let channel = TelegramChannel::new(token, agent_loop);
    channel.start().await?;
    
    Ok(())
}

async fn handle_discord() -> Result<()> {
    let config = load_config()?;
    let dc_config = config.channels.discord.clone().unwrap_or_default();
    
    let token = if dc_config.bot_token.is_empty() {
        std::env::var("DISCORD_BOT_TOKEN").map_err(|_| anyhow!("DISCORD_BOT_TOKEN environment variable or config parameter not set."))?
    } else {
        dc_config.bot_token
    };
    
    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config).await?;
    let channel = crate::channels::DiscordChannel::new(token, agent_loop);
    channel.start().await?;
    
    Ok(())
}

async fn handle_whatsapp() -> Result<()> {
    let config = load_config()?;
    let wa_config = config.channels.whatsapp.clone().unwrap_or_default();
    
    let key = if wa_config.api_key.is_empty() {
        std::env::var("WHATSAPP_API_KEY").map_err(|_| anyhow!("WHATSAPP_API_KEY environment variable or config parameter not set."))?
    } else {
        wa_config.api_key
    };
    
    let phone_id = if wa_config.phone_number_id.is_empty() {
        std::env::var("WHATSAPP_PHONE_NUMBER_ID").map_err(|_| anyhow!("WHATSAPP_PHONE_NUMBER_ID environment variable or config parameter not set."))?
    } else {
        wa_config.phone_number_id
    };
    
    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config).await?;
    let channel = crate::channels::WhatsAppChannel::new(key, phone_id, agent_loop);
    channel.start().await?;
    
    Ok(())
}

fn setup_systemd_service() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    let service_dir = home.join(".config/systemd/user");
    std::fs::create_dir_all(&service_dir)?;
    
    let service_path = service_dir.join("openz-gateway.service");
    let current_exe = std::env::current_exe()?;
    let exe_path = current_exe.to_string_lossy();
    
    let service_content = format!(
        "[Unit]\n\
         Description=OpenZ Gateway Service\n\
         After=network.target\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart={} gateway\n\
         Restart=always\n\
         RestartSec=5\n\n\
         [Install]\n\
         WantedBy=default.target\n",
        exe_path
    );
    
    std::fs::write(&service_path, service_content)?;
    println!("📄 Written systemd user service unit to: {:?}", service_path);
    
    // Enable and start the service
    println!("⚙️ Reloading systemd user daemon and enabling/starting openz-gateway.service...");
    let _ = std::process::Command::new("systemctl")
        .args(&["--user", "daemon-reload"])
        .status();
    let _ = std::process::Command::new("systemctl")
        .args(&["--user", "enable", "openz-gateway.service"])
        .status();
    let status = std::process::Command::new("systemctl")
        .args(&["--user", "restart", "openz-gateway.service"])
        .status();
        
    if status.map(|s| s.success()).unwrap_or(false) {
        println!("✅ openz-gateway.service successfully started and enabled!");
    } else {
        println!("⚠️ Failed to restart openz-gateway.service via systemctl. Make sure systemd user services are supported on your system.");
    }
    
    Ok(())
}

fn disable_systemd_service() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    let service_path = home.join(".config/systemd/user/openz-gateway.service");
    
    if service_path.exists() {
        println!("⚙️ Stopping and disabling openz-gateway.service...");
        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "stop", "openz-gateway.service"])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "disable", "openz-gateway.service"])
            .status();
        let _ = std::fs::remove_file(&service_path);
        let _ = std::process::Command::new("systemctl")
            .args(&["--user", "daemon-reload"])
            .status();
        println!("✅ systemd user service removed.");
    }
    Ok(())
}

async fn handle_configure() -> Result<()> {
    loop {
        println!("\n=== OpenZ Configuration Menu ===");
        let options = vec![
            "1. Providers (API keys, models)",
            "2. Gateway/WebSocket (Host, port, auto-start)",
            "3. Telegram (Bot token)",
            "4. Discord (Bot token)",
            "5. WhatsApp (API key, Phone ID)",
            "6. Exit & Save"
        ];
        let choice = Select::new("Select category to configure:", options).prompt()?;

        let mut config = load_config()?;

        match choice {
            c if c.starts_with("1.") => {
                handle_providers_config(&mut config).await?;
            }
            c if c.starts_with("2.") => {
                handle_gateway_config(&mut config).await?;
            }
            c if c.starts_with("3.") => {
                handle_telegram_config(&mut config).await?;
            }
            c if c.starts_with("4.") => {
                handle_discord_config(&mut config).await?;
            }
            c if c.starts_with("5.") => {
                handle_whatsapp_config(&mut config).await?;
            }
            _ => {
                save_config(&config)?;
                println!("✅ Configuration saved successfully!");
                break;
            }
        }
        
        save_config(&config)?;
    }
    Ok(())
}

async fn handle_providers_config(config: &mut Config) -> Result<()> {
    println!("\n--- Configure Provider ---");
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
    
    println!("✅ Provider configured.");
    Ok(())
}

async fn handle_gateway_config(config: &mut Config) -> Result<()> {
    println!("\n--- Configure Gateway/WebSocket ---");
    let mut ws = config.channels.websocket.clone().unwrap_or_else(|| {
        WebSocketChannelConfig {
            enabled: true,
            port: 8765,
            host: "127.0.0.1".to_string(),
            start_on_boot: false,
            start_on_tui: false,
        }
    });

    let enabled_options = vec!["Yes", "No"];
    let enabled_choice = Select::new("Enable Gateway/WebSocket channel?", enabled_options)
        .with_starting_cursor(if ws.enabled { 0 } else { 1 })
        .prompt()?;
    ws.enabled = enabled_choice == "Yes";

    let host_input = Text::new(&format!("Enter Host [default: {}]:", ws.host)).prompt()?;
    if !host_input.trim().is_empty() {
        ws.host = host_input.trim().to_string();
    }

    let port_input = Text::new(&format!("Enter Port [default: {}]:", ws.port)).prompt()?;
    if !port_input.trim().is_empty() {
        if let Ok(p) = port_input.trim().parse::<u16>() {
            ws.port = p;
        }
    }

    let auto_start_options = vec!(
        "1. Start gateway when system power on (systemd service)",
        "2. Start gateway when openz TUI starts",
        "3. Manual start only"
    );
    let default_cursor = if ws.start_on_boot { 0 } else if ws.start_on_tui { 1 } else { 2 };
    let auto_start_choice = Select::new("Select gateway auto-start preference:", auto_start_options)
        .with_starting_cursor(default_cursor)
        .prompt()?;

    if auto_start_choice.starts_with("1.") {
        ws.start_on_boot = true;
        ws.start_on_tui = false;
        setup_systemd_service()?;
    } else if auto_start_choice.starts_with("2.") {
        ws.start_on_boot = false;
        ws.start_on_tui = true;
        disable_systemd_service()?;
    } else {
        ws.start_on_boot = false;
        ws.start_on_tui = false;
        disable_systemd_service()?;
    }

    config.channels.websocket = Some(ws);
    println!("✅ Gateway configured.");
    Ok(())
}

async fn handle_telegram_config(config: &mut Config) -> Result<()> {
    println!("\n--- Configure Telegram Bot ---");
    let mut tg = config.channels.telegram.clone().unwrap_or_default();

    let enabled_options = vec!["Yes", "No"];
    let enabled_choice = Select::new("Enable Telegram channel?", enabled_options)
        .with_starting_cursor(if tg.enabled { 0 } else { 1 })
        .prompt()?;
    tg.enabled = enabled_choice == "Yes";

    let token_input = Password::new(&format!("Enter Telegram Bot Token [current: {}]:", if tg.bot_token.is_empty() { "None" } else { "********" })).prompt()?;
    if !token_input.trim().is_empty() {
        tg.bot_token = token_input.trim().to_string();
    }

    config.channels.telegram = Some(tg);
    println!("✅ Telegram bot configured.");
    Ok(())
}

async fn handle_discord_config(config: &mut Config) -> Result<()> {
    println!("\n--- Configure Discord Bot ---");
    let mut dc = config.channels.discord.clone().unwrap_or_default();

    let enabled_options = vec!["Yes", "No"];
    let enabled_choice = Select::new("Enable Discord channel?", enabled_options)
        .with_starting_cursor(if dc.enabled { 0 } else { 1 })
        .prompt()?;
    dc.enabled = enabled_choice == "Yes";

    let token_input = Password::new(&format!("Enter Discord Bot Token [current: {}]:", if dc.bot_token.is_empty() { "None" } else { "********" })).prompt()?;
    if !token_input.trim().is_empty() {
        dc.bot_token = token_input.trim().to_string();
    }

    config.channels.discord = Some(dc);
    println!("✅ Discord bot configured.");
    Ok(())
}

async fn handle_whatsapp_config(config: &mut Config) -> Result<()> {
    println!("\n--- Configure WhatsApp Business API ---");
    let mut wa = config.channels.whatsapp.clone().unwrap_or_default();

    let enabled_options = vec!["Yes", "No"];
    let enabled_choice = Select::new("Enable WhatsApp channel?", enabled_options)
        .with_starting_cursor(if wa.enabled { 0 } else { 1 })
        .prompt()?;
    wa.enabled = enabled_choice == "Yes";

    let key_input = Password::new(&format!("Enter WhatsApp API Key [current: {}]:", if wa.api_key.is_empty() { "None" } else { "********" })).prompt()?;
    if !key_input.trim().is_empty() {
        wa.api_key = key_input.trim().to_string();
    }

    let phone_input = Text::new(&format!("Enter WhatsApp Phone Number ID [current: {}]:", wa.phone_number_id)).prompt()?;
    if !phone_input.trim().is_empty() {
        wa.phone_number_id = phone_input.trim().to_string();
    }

    config.channels.whatsapp = Some(wa);
    println!("✅ WhatsApp channel configured.");
    Ok(())
}
