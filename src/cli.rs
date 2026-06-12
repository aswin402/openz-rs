use crate::config::loader::{load_config, save_config, resolve_path};
use crate::config::schema::{Config, ProviderConfig};
use clap::{Parser, Subcommand};
use inquire::{Text, Select, Password, Confirm, PasswordDisplayMode};
use anyhow::{Result, anyhow};
use std::sync::Arc;
use crate::providers::{openai::OpenAIProvider, anthropic::AnthropicProvider, LLMProvider};
use crate::tools::ToolRegistry;
use crate::tools::filesystem::{ReadFileTool, WriteFileTool, ListDirTool, PatchFileTool, FindFilesTool};
use crate::tools::doc_reader::DocReaderTool;
use crate::tools::wasm_sandbox::WasmSandboxTool;
use crate::tools::semantic_search::SemanticSearchTool;
use crate::tools::rust_docs::RustDocsTool;
use crate::tools::shell::ExecCommandTool;
use crate::tools::web::WebFetchTool;
use crate::tools::subagent::{DelegateTaskTool, OptimizeSubagentTool, CreateSubagentTool, DeleteSubagentTool};
use crate::tools::cron::{ScheduleJobTool, ListJobsTool, RemoveJobTool};
use crate::tools::remote::SendRemoteInputTool;
use crate::session::SessionManager;
use crate::agent::AgentLoop;
use crate::agent::style::*;
use crate::channels::{CliChannel, WsGateway, TelegramChannel, DiscordChannel, WhatsAppChannel, Channel};
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

    /// Run a gRPC-to-Stdio MCP server bridge
    McpBridge {
        /// TCP port to host the gRPC MCP server on
        #[arg(long)]
        port: u16,
        
        /// Command and arguments of the stdio MCP server to bridge
        #[arg(last = true)]
        command_args: Vec<String>,
    },
}

pub async fn run_cli() -> Result<()> {
    // Intercept version flags for custom themed print
    for arg in std::env::args() {
        if arg == "--version" || arg == "-V" {
            let logo = format!(
                r#"
{}  ___  ____  _____ _   _ _____ 
 / _ \|  _ \| ____| \ | |___  /
| | | | |_) |  _| |  \| |  / /  
| |_| |  __/| |___| |\  | / /__ 
 \___/|_|   |_____|_| \_/_____|{}

{}OpenZ AI Agent Framework - v{}{}
{}Rebranded Ultra-Lightweight Personal AI Agent in Rust{}
"#,
                crate::agent::style::colors::AURA_PURPLE,
                crate::agent::style::colors::COLOR_RESET,
                crate::agent::style::colors::COLOR_BOLD,
                env!("CARGO_PKG_VERSION"),
                crate::agent::style::colors::COLOR_RESET,
                crate::agent::style::colors::AURA_SLATE,
                crate::agent::style::colors::COLOR_RESET
            );
            print!("{}", logo);
            std::process::exit(0);
        }
    }

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
        Command::McpBridge { port, command_args } => {
            if command_args.is_empty() {
                return Err(anyhow!("No target command specified. Usage: openz mcp-bridge --port <port> -- <command> [args...]"));
            }
            let command = &command_args[0];
            let args = &command_args[1..];
            let (_tx, rx) = tokio::sync::oneshot::channel();
            crate::tools::mcp::run_mcp_bridge(port, command, args, rx).await?;
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
    registry.register(std::sync::Arc::new(FindFilesTool));
    registry.register(std::sync::Arc::new(DocReaderTool));
    registry.register(std::sync::Arc::new(WasmSandboxTool));
    registry.register(std::sync::Arc::new(SemanticSearchTool));
    registry.register(std::sync::Arc::new(RustDocsTool::new()));
    registry.register(std::sync::Arc::new(WriteFileTool));
    registry.register(std::sync::Arc::new(PatchFileTool));
    registry.register(std::sync::Arc::new(ListDirTool));
    registry.register(std::sync::Arc::new(ExecCommandTool));
    registry.register(std::sync::Arc::new(WebFetchTool::new()));
    registry.register(std::sync::Arc::new(DelegateTaskTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
        parent_tools: Vec::new(),
    }));

    registry.register(std::sync::Arc::new(OptimizeSubagentTool {
        config: config.clone(),
        parent_provider: provider.clone(),
    }));

    registry.register(std::sync::Arc::new(CreateSubagentTool {
        config: config.clone(),
    }));
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

    let silent = std::env::var("OPENZ_SILENT").is_ok();

    if has_any_mcp && !silent {
        println!("Setting up MCP servers...");
    }

    let emerald_green = "\x1b[38;2;16;185;129m";
    let error_red = "\x1b[38;2;239;68;68m";

    for (name, mcp_config) in &config.mcp_servers {
        if mcp_config.enabled {
            match crate::tools::mcp::McpClient::spawn(&mcp_config.command, &mcp_config.args).await {
                Ok(mcp_client) => {
                    if name == "memory" {
                        crate::tools::mcp::set_memory_mcp_client(mcp_client.clone());
                    }
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
                            if !silent {
                                println!("{}✓ {}{}", emerald_green, name, COLOR_RESET);
                            }
                        }
                        Err(_) => {
                            if !silent {
                                println!("{}✗ {} not connected{}", error_red, name, COLOR_RESET);
                            }
                        }
                    }
                }
                Err(_) => {
                    if !silent {
                        println!("{}✗ {} not connected{}", error_red, name, COLOR_RESET);
                    }
                }
            }
        }
    }

    if has_any_mcp && !silent {
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

    let agent_loop = build_agent_loop(config.clone()).await?;

    // Now, set the silent environment variable so any background channels are silent
    std::env::set_var("OPENZ_SILENT", "true");

    // Auto-start WebSocket gateway in the background if enabled and configured to start on TUI
    if let Some(ws_config) = &config.channels.websocket {
        if ws_config.enabled && ws_config.start_on_tui {
            let config_clone = config.clone();
            let ws_config_clone = ws_config.clone();
            tokio::spawn(async move {
                match build_agent_loop(config_clone).await {
                    Ok(agent_loop) => {
                        let gateway = WsGateway::new(ws_config_clone, agent_loop);
                        let _ = gateway.start().await;
                    }
                    _ => {}
                }
            });
        }
    }

    // Auto-start Telegram channel in the background if enabled
    if let Some(tg_config) = &config.channels.telegram {
        if tg_config.enabled {
            let token = if tg_config.bot_token.is_empty() {
                std::env::var("TELEGRAM_BOT_TOKEN").ok()
            } else {
                Some(tg_config.bot_token.clone())
            };
            if let Some(token) = token {
                let config_clone = config.clone();
                tokio::spawn(async move {
                    match build_agent_loop(config_clone).await {
                        Ok(agent_loop) => {
                            let channel = TelegramChannel::new(token, agent_loop);
                            let _ = channel.start().await;
                        }
                        _ => {}
                    }
                });
            }
        }
    }

    // Auto-start Discord channel in the background if enabled
    if let Some(dc_config) = &config.channels.discord {
        if dc_config.enabled {
            let token = if dc_config.bot_token.is_empty() {
                std::env::var("DISCORD_BOT_TOKEN").ok()
            } else {
                Some(dc_config.bot_token.clone())
            };
            if let Some(token) = token {
                let config_clone = config.clone();
                tokio::spawn(async move {
                    match build_agent_loop(config_clone).await {
                        Ok(agent_loop) => {
                            let channel = DiscordChannel::new(token, agent_loop);
                            let _ = channel.start().await;
                        }
                        _ => {}
                    }
                });
            }
        }
    }

    // Auto-start WhatsApp channel in the background if enabled
    if let Some(wa_config) = &config.channels.whatsapp {
        if wa_config.enabled {
            let config_clone = config.clone();
            let wa_config_clone = wa_config.clone();
            tokio::spawn(async move {
                match build_agent_loop(config_clone).await {
                    Ok(agent_loop) => {
                        let channel = WhatsAppChannel::new(
                            wa_config_clone.api_key,
                            wa_config_clone.phone_number_id,
                            agent_loop,
                        );
                        let _ = channel.start().await;
                    }
                    _ => {}
                }
            });
        }
    }

    let channel = CliChannel::new(agent_loop, defaults);
    tokio::select! {
        res = channel.start() => {
            if let Err(e) = res {
                eprintln!("TUI error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\r\nExiting OpenZ...");
        }
    }
    
    crate::channels::shutdown_gateways(&config).await;
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
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = TelegramChannel::new(token, agent_loop);
    
    tokio::select! {
        res = channel.start() => {
            if let Err(e) = res {
                eprintln!("Telegram error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\r\nExiting Telegram channel...");
        }
    }
    
    crate::channels::shutdown_gateways(&config).await;
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
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = crate::channels::DiscordChannel::new(token, agent_loop);
    
    tokio::select! {
        res = channel.start() => {
            if let Err(e) = res {
                eprintln!("Discord error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\r\nExiting Discord channel...");
        }
    }
    
    crate::channels::shutdown_gateways(&config).await;
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
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = crate::channels::WhatsAppChannel::new(key, phone_id, agent_loop);
    
    tokio::select! {
        res = channel.start() => {
            if let Err(e) = res {
                eprintln!("WhatsApp error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\r\nExiting WhatsApp channel...");
        }
    }
    
    crate::channels::shutdown_gateways(&config).await;
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

        if let Some(ref ws) = config.channels.websocket {
            if ws.enabled {
                configure_options.push("Gateway (WebSocket) (enabled)".to_string());
            } else {
                configure_options.push("Gateway (WebSocket)".to_string());
            }
        } else {
            configure_options.push("Gateway (WebSocket)".to_string());
        }
        
        if is_telegram_configured(&config) {
            configure_options.push("Telegram (configured)".to_string());
        } else {
            configure_options.push("Telegram".to_string());
        }

        if let Some(ref dc) = config.channels.discord {
            if dc.enabled && !dc.bot_token.trim().is_empty() {
                configure_options.push("Discord (configured)".to_string());
            } else {
                configure_options.push("Discord".to_string());
            }
        } else {
            configure_options.push("Discord".to_string());
        }

        if let Some(ref wa) = config.channels.whatsapp {
            if wa.enabled && !wa.api_key.trim().is_empty() {
                configure_options.push("WhatsApp (configured)".to_string());
            } else {
                configure_options.push("WhatsApp".to_string());
            }
        } else {
            configure_options.push("WhatsApp".to_string());
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
                handle_gateway_submenu(&mut config).await?;
            }
            2 => {
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
            3 => {
                handle_discord_submenu(&mut config).await?;
            }
            4 => {
                handle_whatsapp_submenu(&mut config).await?;
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
            if config.is_provider_configured(p.name) {
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
        
        if config.is_provider_configured(prov_info.name) {
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

fn setup_systemd_service() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not locate home directory"))?;
    let service_dir = home.join(".config").join("systemd").join("user");
    std::fs::create_dir_all(&service_dir)?;
    
    let exe_path = std::env::current_exe().unwrap_or_else(|_| home.join(".cargo").join("bin").join("openz"));
    
    let service_content = format!(
r#"[Unit]
Description=OpenZ WebSocket Gateway Daemon
After=network.target

[Service]
ExecStart={} gateway
Restart=on-failure

[Install]
WantedBy=default.target
"#,
        exe_path.to_string_lossy()
    );
    
    let service_file = service_dir.join("openz-gateway.service");
    std::fs::write(&service_file, service_content)?;
    
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
        
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "enable", "openz-gateway.service"])
        .output();
        
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "restart", "openz-gateway.service"])
        .output();
        
    Ok(())
}

fn disable_systemd_service() -> Result<()> {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "disable", "openz-gateway.service"])
        .output();
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "stop", "openz-gateway.service"])
        .output();
    Ok(())
}

async fn handle_gateway_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Gateway (WebSocket) Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    
    let mut ws = config.channels.websocket.clone().unwrap_or_else(|| {
        crate::config::schema::WebSocketChannelConfig {
            enabled: true,
            port: 8765,
            host: "127.0.0.1".to_string(),
            start_on_boot: false,
            start_on_tui: false,
        }
    });

    let enabled = Confirm::new("Enable WebSocket gateway?")
        .with_default(ws.enabled)
        .prompt()?;
    ws.enabled = enabled;

    if enabled {
        let port_input = Text::new(&format!("WebSocket Port [default: {}]:", ws.port)).prompt()?;
        if !port_input.trim().is_empty() {
            if let Ok(p) = port_input.trim().parse::<u16>() {
                ws.port = p;
            }
        }

        let host_input = Text::new(&format!("WebSocket Host [default: {}]:", ws.host)).prompt()?;
        if !host_input.trim().is_empty() {
            ws.host = host_input.trim().to_string();
        }

        let auto_start_options = vec![
            "None (Manual launch only)".to_string(),
            "Auto-start on TUI launch (TUI background thread)".to_string(),
            "System Boot Daemon (Installs systemd user service)".to_string(),
        ];

        let auto_choice = select_menu_custom(
            "Choose Gateway Auto-Start Preference:",
            &auto_start_options,
            "None (Manual launch only)",
            None,
            false,
        )?;

        match auto_choice {
            Some(2) => {
                ws.start_on_boot = true;
                ws.start_on_tui = false;
                println!("Installing and enabling systemd user service...");
                if let Err(e) = setup_systemd_service() {
                    eprintln!("{}✕ Failed to setup systemd service: {}{}", ERROR_RED, e, COLOR_RESET);
                } else {
                    println!("{}✓ systemd service openz-gateway.service installed and enabled successfully!{}", EMERALD_GREEN, COLOR_RESET);
                }
            }
            Some(1) => {
                ws.start_on_boot = false;
                ws.start_on_tui = true;
                let _ = disable_systemd_service();
                println!("Gateway configured to auto-start when terminal client launches.");
            }
            _ => {
                ws.start_on_boot = false;
                ws.start_on_tui = false;
                let _ = disable_systemd_service();
                println!("Gateway auto-start disabled (manual launch only).");
            }
        }
    } else {
        let _ = disable_systemd_service();
    }

    config.channels.websocket = Some(ws);
    save_config(config)?;
    println!("{}✓ Gateway configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}

async fn handle_discord_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Discord Bot Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    
    let mut dc = config.channels.discord.clone().unwrap_or_default();
    
    let enabled = Confirm::new("Enable Discord Bot channel?")
        .with_default(dc.enabled)
        .prompt()?;
    dc.enabled = enabled;

    if enabled {
        let token = Password::new("Paste Discord Bot Token: ")
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        if !token.trim().is_empty() {
            dc.bot_token = token.trim().to_string();
        }
    }

    config.channels.discord = Some(dc);
    save_config(config)?;
    println!("{}✓ Discord channel configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}

async fn handle_whatsapp_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- WhatsApp Channel Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    
    let mut wa = config.channels.whatsapp.clone().unwrap_or_default();
    
    let enabled = Confirm::new("Enable WhatsApp channel?")
        .with_default(wa.enabled)
        .prompt()?;
    wa.enabled = enabled;

    if enabled {
        let key = Password::new("Paste WhatsApp API Key: ")
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        if !key.trim().is_empty() {
            wa.api_key = key.trim().to_string();
        }

        let phone_id = Text::new("Enter WhatsApp Phone Number ID: ").prompt()?;
        if !phone_id.trim().is_empty() {
            wa.phone_number_id = phone_id.trim().to_string();
        }
    }

    config.channels.whatsapp = Some(wa);
    save_config(config)?;
    println!("{}✓ WhatsApp channel configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}


