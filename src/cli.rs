use crate::config::loader::{load_config, save_config, resolve_path};
use crate::config::schema::{Config, ProviderConfig, WebSocketChannelConfig};
use clap::{Parser, Subcommand};
use inquire::{Text, Select, Password, Confirm, PasswordDisplayMode};
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
use crate::agent::style::*;
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
    
    let providers = vec!["anthropic", "openai", "openrouter", "deepseek", "groq", "ollama", "minimax"];
    let provider_name = Select::new("Choose an LLM provider:", providers).prompt()?;
    
    let mut api_key = None;
    if provider_name != "ollama" {
        let key = Password::new(&format!("Enter API Key for {}:", provider_name))
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
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
        "minimax" => "https://api.minimax.io/v1",
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
        "minimax" => "MiniMax-M3",
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
        "minimax" => config.providers.minimax = p_config,
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
        "minimax" => {
            let p = config.providers.minimax.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.minimax.io/v1".to_string());
            (key, base, defaults.model.clone())
        }
        "mistral" => {
            let p = config.providers.mistral.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("MISTRAL_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string());
            (key, base, defaults.model.clone())
        }
        "z.ai" => {
            let p = config.providers.z_ai.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("Z_AI_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.z.ai/api/paas/v4/".to_string());
            (key, base, defaults.model.clone())
        }
        "nvidia" => {
            let p = config.providers.nvidia.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("NVIDIA_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://integrate.api.nvidia.com/v1".to_string());
            (key, base, defaults.model.clone())
        }
        "opencode_zen" | "opencode zen" => {
            let p = config.providers.opencode_zen.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENCODE_ZEN_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://opencode.ai/zen/v1".to_string());
            (key, base, defaults.model.clone())
        }
        "cerebres" => {
            let p = config.providers.cerebres.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("CEREBRES_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.cerebras.ai/v1".to_string());
            (key, base, defaults.model.clone())
        }
        "google_ai_studio" | "google ai studio" => {
            let p = config.providers.google_ai_studio.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("GOOGLE_AI_STUDIO_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta/openai/".to_string());
            (key, base, defaults.model.clone())
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
    registry.register(std::sync::Arc::new(crate::tools::ast_grep::AstGrepTool));
    registry.register(std::sync::Arc::new(crate::tools::gsd_browser::GsdBrowserTool));
    registry.register(std::sync::Arc::new(crate::tools::web_search::WebSearchTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::onpkg::OnpkgTool));

    // Register configured MCP tools
    let mut total_tools = 0;
    let mut has_any_mcp = false;
    for (_, mcp_config) in &config.mcp_servers {
        if mcp_config.enabled {
            has_any_mcp = true;
            break;
        }
    }

    if has_any_mcp {
        println!("Setting up MCP servers...");
    }

    let emerald_green = "\x1b[38;2;16;185;129m";
    let error_red = "\x1b[38;2;239;68;68m";

    for (name, mcp_config) in &config.mcp_servers {
        if mcp_config.enabled {
            match crate::tools::mcp::McpClient::spawn(&mcp_config.command, &mcp_config.args).await {
                Ok(mcp_client) => {
                    match mcp_client.list_tools().await {
                        Ok(tools) => {
                            let mut count = 0;
                            for t in tools {
                                if let (Some(t_name), Some(desc)) = (t.get("name").and_then(|v| v.as_str()), t.get("description").and_then(|v| v.as_str())) {
                                    let params = t.get("inputSchema").cloned().unwrap_or(serde_json::json!({
                                        "type": "object",
                                        "properties": {}
                                    }));
                                    
                                    let wrapper = crate::tools::mcp::McpToolWrapper {
                                        client: mcp_client.clone(),
                                        name: t_name.to_string(),
                                        description: desc.to_string(),
                                        parameters: params,
                                    };
                                    registry.register(std::sync::Arc::new(wrapper));
                                    count += 1;
                                }
                            }
                            total_tools += count;
                            println!("{}✓ {}{}", emerald_green, name, COLOR_RESET);
                        }
                        Err(_) => {
                            println!("{}✗ {} not connected{}", error_red, name, COLOR_RESET);
                        }
                    }
                }
                Err(_) => {
                    println!("{}✗ {} not connected{}", error_red, name, COLOR_RESET);
                }
            }
        }
    }

    if has_any_mcp {
        println!("\n{} tools loaded\n", total_tools);
    }
    
    Ok(AgentLoop::new(config, provider, registry, session_manager))
}



pub fn load_session_history() -> Result<Vec<HistoryItem>> {
    let sessions_dir = resolve_path("~/.openz/sessions");
    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }
    
    let mut items = Vec::new();
    for entry in std::fs::read_dir(sessions_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(session) = serde_json::from_str::<crate::session::Session>(&content) {
                    if !session.messages.is_empty() {
                        let preview = session.messages.iter()
                            .find(|m| m.role == "user")
                            .map(|m| {
                                let mut text = m.content.clone();
                                if text.len() > 50 {
                                    text.truncate(47);
                                    text.push_str("...");
                                }
                                text
                            })
                            .unwrap_or_else(|| "Empty session".to_string());
                            
                        items.push(HistoryItem {
                            key: session.key.clone(),
                            display_title: preview,
                            updated_at: session.updated_at,
                        });
                    }
                }
            }
        }
    }
    
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(items)
}

pub fn archive_current_session(session_manager: &SessionManager) -> Result<()> {
    if let Ok(mut current_session) = session_manager.load("cli:direct") {
        if !current_session.messages.is_empty() {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
            let archive_key = format!("cli:history_{}", timestamp);
            current_session.key = archive_key;
            session_manager.save(&current_session)?;
            
            let empty_session = crate::session::Session::new("cli:direct");
            session_manager.save(&empty_session)?;
        }
    }
    Ok(())
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

    let sessions_dir = resolve_path("~/.openz/sessions");
    let session_manager = SessionManager::new(sessions_dir);

    let history = load_session_history()?;
    if history.is_empty() {
        archive_current_session(&session_manager)?;
    } else {
        let selected = select_menu_with_history("Welcome to OpenZ! Select an option:", &history)?;
        if selected == 0 {
            archive_current_session(&session_manager)?;
        } else {
            let selected_item = &history[selected - 1];
            archive_current_session(&session_manager)?;
            let mut session = session_manager.load(&selected_item.key)?;
            session.key = "cli:direct".to_string();
            session_manager.save(&session)?;
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

fn update_provider_key(config: &mut Config, provider_name: &str, api_key: String) {
    let mut p_config = match provider_name {
        "anthropic" => config.providers.anthropic.clone(),
        "openai" => config.providers.openai.clone(),
        "openrouter" => config.providers.openrouter.clone(),
        "deepseek" => config.providers.deepseek.clone(),
        "groq" => config.providers.groq.clone(),
        "ollama" => config.providers.ollama.clone(),
        "minimax" => config.providers.minimax.clone(),
        "mistral" => config.providers.mistral.clone(),
        "z.ai" => config.providers.z_ai.clone(),
        "nvidia" => config.providers.nvidia.clone(),
        "opencode_zen" => config.providers.opencode_zen.clone(),
        "cerebres" => config.providers.cerebres.clone(),
        "google_ai_studio" => config.providers.google_ai_studio.clone(),
        _ => return,
    }.unwrap_or_else(|| ProviderConfig {
        api_key: None,
        api_base: None,
        api_type: None,
        extra: std::collections::HashMap::new(),
    });
    p_config.api_key = Some(api_key);
    
    if p_config.api_base.is_none() {
        let default_base = match provider_name {
            "anthropic" => "https://api.anthropic.com",
            "openai" => "https://api.openai.com/v1",
            "openrouter" => "https://openrouter.ai/api/v1",
            "deepseek" => "https://api.deepseek.com/v1",
            "groq" => "https://api.groq.com/openai/v1",
            "ollama" => "http://localhost:11434/v1",
            "minimax" => "https://api.minimax.io/v1",
            "mistral" => "https://api.mistral.ai/v1",
            "z.ai" => "https://api.z.ai/api/paas/v4/",
            "nvidia" => "https://integrate.api.nvidia.com/v1",
            "opencode_zen" => "https://opencode.ai/zen/v1",
            "cerebres" => "https://api.cerebras.ai/v1",
            "google_ai_studio" => "https://generativelanguage.googleapis.com/v1beta/openai/",
            _ => "",
        };
        p_config.api_base = Some(default_base.to_string());
    }

    match provider_name {
        "anthropic" => config.providers.anthropic = Some(p_config),
        "openai" => config.providers.openai = Some(p_config),
        "openrouter" => config.providers.openrouter = Some(p_config),
        "deepseek" => config.providers.deepseek = Some(p_config),
        "groq" => config.providers.groq = Some(p_config),
        "ollama" => config.providers.ollama = Some(p_config),
        "minimax" => config.providers.minimax = Some(p_config),
        "mistral" => config.providers.mistral = Some(p_config),
        "z.ai" => config.providers.z_ai = Some(p_config),
        "nvidia" => config.providers.nvidia = Some(p_config),
        "opencode_zen" => config.providers.opencode_zen = Some(p_config),
        "cerebres" => config.providers.cerebres = Some(p_config),
        "google_ai_studio" => config.providers.google_ai_studio = Some(p_config),
        _ => {}
    }
}

fn is_telegram_configured(config: &Config) -> bool {
    if let Some(ref tg) = config.channels.telegram {
        tg.enabled && !tg.bot_token.trim().is_empty()
    } else {
        false
    }
}

fn is_gateway_configured(config: &Config) -> bool {
    if let Some(ref ws) = config.channels.websocket {
        ws.enabled && (ws.start_on_boot || ws.start_on_tui)
    } else {
        false
    }
}

async fn handle_configure() -> Result<()> {
    let active_mdl = {
        let config = load_config()?;
        config.agents.defaults.model.clone()
    };

    loop {
        let mut config = load_config()?;
        
        let mut configure_options = vec![
            "Providers".to_string(),
        ];
        
        if is_telegram_configured(&config) {
            configure_options.push("Telegram (configured)".to_string());
        } else {
            configure_options.push("Telegram".to_string());
        }

        if is_gateway_configured(&config) {
            configure_options.push("Gateway (configured)".to_string());
        } else {
            configure_options.push("Gateway".to_string());
        }

        configure_options.push("Exit".to_string());

        let choice_idx = match select_menu_custom(
            "Choose configure category:",
            &configure_options,
            &active_mdl,
            Some("OpenZ Configuration"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break, // Exit on Esc
        };

        match choice_idx {
            0 => {
                handle_providers_submenu(&mut config, &active_mdl).await?;
            }
            1 => {
                if is_telegram_configured(&config) {
                    let reconfigure = Confirm::new("Telegram is already configured. Reconfigure?")
                        .with_default(false)
                        .prompt()?;
                    if !reconfigure {
                        continue;
                    }
                }
                handle_telegram_submenu(&mut config).await?;
            }
            2 => {
                if is_gateway_configured(&config) {
                    let reconfigure = Confirm::new("Gateway is already configured. Reconfigure?")
                        .with_default(false)
                        .prompt()?;
                    if !reconfigure {
                        continue;
                    }
                }
                handle_gateway_submenu(&mut config, &active_mdl).await?;
            }
            _ => {
                break;
            }
        }
        
        save_config(&config)?;
    }
    
    println!("{}✓ Configuration saved successfully!{}", EMERALD_GREEN, COLOR_RESET);
    Ok(())
}

fn is_provider_configured(config: &Config, provider_name: &str) -> bool {
    let p_opt = match provider_name {
        "anthropic" => &config.providers.anthropic,
        "openai" => &config.providers.openai,
        "openrouter" => &config.providers.openrouter,
        "deepseek" => &config.providers.deepseek,
        "groq" => &config.providers.groq,
        "ollama" => &config.providers.ollama,
        "minimax" => &config.providers.minimax,
        "mistral" => &config.providers.mistral,
        "z.ai" => &config.providers.z_ai,
        "nvidia" => &config.providers.nvidia,
        "opencode_zen" => &config.providers.opencode_zen,
        "cerebres" => &config.providers.cerebres,
        "google_ai_studio" => &config.providers.google_ai_studio,
        _ => return false,
    };
    if let Some(p) = p_opt {
        p.api_key.as_ref().map(|k| !k.trim().is_empty()).unwrap_or(false)
    } else {
        false
    }
}

async fn handle_providers_submenu(config: &mut Config, active_mdl: &str) -> Result<()> {
    struct ProviderInfo {
        name: &'static str,
        display: &'static str,
    }
    let provider_list = vec![
        ProviderInfo { name: "anthropic", display: "Anthropic (Claude)" },
        ProviderInfo { name: "openai", display: "OpenAI" },
        ProviderInfo { name: "openrouter", display: "OpenRouter" },
        ProviderInfo { name: "deepseek", display: "DeepSeek" },
        ProviderInfo { name: "groq", display: "Groq" },
        ProviderInfo { name: "ollama", display: "Ollama" },
        ProviderInfo { name: "minimax", display: "MiniMax" },
        ProviderInfo { name: "mistral", display: "Mistral AI" },
        ProviderInfo { name: "z.ai", display: "z.ai (Zhipu GLM)" },
        ProviderInfo { name: "nvidia", display: "NVIDIA NIM" },
        ProviderInfo { name: "opencode_zen", display: "OpenCode Zen" },
        ProviderInfo { name: "cerebres", display: "Cerebras" },
        ProviderInfo { name: "google_ai_studio", display: "Google AI Studio (Gemini)" },
    ];

    loop {
        let mut prov_options: Vec<String> = provider_list.iter().map(|p| {
            if is_provider_configured(config, p.name) {
                format!("{} (configured)", p.display)
            } else {
                p.display.to_string()
            }
        }).collect();
        prov_options.push("Back".to_string());

        let choice_idx = match select_menu_custom(
            "Select provider to configure:",
            &prov_options,
            active_mdl,
            Some("Providers Configuration"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break, // Go back on Esc
        };

        if choice_idx == provider_list.len() {
            break; // Back option
        }

        let prov_info = &provider_list[choice_idx];
        
        if is_provider_configured(config, prov_info.name) {
            let reconfigure = Confirm::new(&format!("{} is already configured. Reconfigure?", prov_info.display))
                .with_default(false)
                .prompt()?;
            if !reconfigure {
                continue;
            }
        }

        let key = Password::new(&format!("Enter API Key for {}:", prov_info.display))
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        if !key.trim().is_empty() {
            update_provider_key(config, prov_info.name, key.trim().to_string());
            save_config(config)?;
            println!("{}✓ API Key updated for {}!{}", EMERALD_GREEN, prov_info.display, COLOR_RESET);
        } else {
            println!("{}⚠️ API Key unchanged.{}", AURA_GOLD, COLOR_RESET);
        }
    }
    Ok(())
}

async fn handle_telegram_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Telegram Bot Setup Guide ---{}", COLOR_BOLD, COLOR_RESET);
    println!("  1. Open Telegram and search for {}@BotFather{}.", HEADING_BLUE, COLOR_RESET);
    println!("  2. Start a chat and send the command {}/newbot{}.", EMERALD_GREEN, COLOR_RESET);
    println!("  3. Choose a name and a username for your bot.");
    println!("  4. Copy the HTTP API token provided by BotFather.\n");

    let token = Password::new("Paste Telegram Bot Token: ")
        .without_confirmation()
        .with_display_mode(PasswordDisplayMode::Masked)
        .prompt()?;
    if !token.trim().is_empty() {
        let mut tg = config.channels.telegram.clone().unwrap_or_default();
        tg.enabled = true;
        tg.bot_token = token.trim().to_string();
        config.channels.telegram = Some(tg);
        save_config(config)?;
        println!("{}✓ Telegram bot configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    } else {
        println!("{}⚠️ Token unchanged.{}", AURA_GOLD, COLOR_RESET);
    }
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}

async fn handle_gateway_submenu(config: &mut Config, active_mdl: &str) -> Result<()> {
    let mut ws = config.channels.websocket.clone().unwrap_or_else(|| {
        WebSocketChannelConfig {
            enabled: true,
            port: 8765,
            host: "127.0.0.1".to_string(),
            start_on_boot: false,
            start_on_tui: false,
        }
    });

    let gateway_options = vec![
        "Start gateway when computer turns on".to_string(),
        "Start gateway when user starts openz in terminal".to_string(),
        "Back".to_string(),
    ];

    loop {
        let choice_idx = match select_menu_custom(
            "Select gateway preference:",
            &gateway_options,
            active_mdl,
            Some("Gateway Configuration"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break, // Go back on Esc
        };

        if choice_idx == 2 {
            break; // Back option
        }

        match choice_idx {
            0 => {
                ws.enabled = true;
                ws.start_on_boot = true;
                ws.start_on_tui = false;
                setup_systemd_service()?;
                println!("{}✓ Configured gateway to start on boot.{}", EMERALD_GREEN, COLOR_RESET);
            }
            1 => {
                ws.enabled = true;
                ws.start_on_boot = false;
                ws.start_on_tui = true;
                disable_systemd_service()?;
                println!("{}✓ Configured gateway to start when TUI starts.{}", EMERALD_GREEN, COLOR_RESET);
            }
            _ => {}
        }

        config.channels.websocket = Some(ws.clone());
        save_config(config)?;
    }
    Ok(())
}
