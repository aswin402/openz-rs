use crate::config::loader::{load_config, save_config, resolve_path};
use crate::config::schema::{Config, ProviderConfig};
use clap::{Parser, Subcommand};
use inquire::{Text, Select, Password, Confirm, PasswordDisplayMode};
use anyhow::{Result, anyhow};
// Provider resolution now handled by crate::providers::resolver
use crate::tools::ToolRegistry;
use crate::tools::filesystem::{ReadFileTool, WriteFileTool, ListDirTool, PatchFileTool, FindFilesTool, ReplaceLinesTool, ZenflowEditTool};
use crate::tools::db_inspector::{DbInspectorTool, DbWriteTool};
use crate::tools::system_info::SystemInfoTool;
use crate::tools::network::CheckPortTool;
use crate::tools::doc_reader::DocReaderTool;
use crate::tools::wasm_sandbox::WasmSandboxTool;
use crate::tools::js_format::JsFormatTool;
use crate::tools::semantic_search::SemanticSearchTool;
use crate::tools::shared_memory::{StoreMemoryTool, RecallMemoryTool, ClearMemoryTool, ArchiveResearchTool, SearchResearchTool};
use crate::tools::notes::IndexNotesTool;
use crate::tools::social_search::SocialSearchTool;
use crate::tools::rust_docs::RustDocsTool;
use crate::tools::shell::{ExecCommandTool, PythonSandboxTool};
use crate::tools::web::WebFetchTool;
use crate::tools::subagent::{DelegateTaskTool, OptimizeSubagentTool, CreateSubagentTool, DeleteSubagentTool, ParallelResearchTool};
use crate::tools::cron::{ScheduleJobTool, ListJobsTool, RemoveJobTool};
use crate::tools::remote::SendRemoteInputTool;
use crate::session::SessionManager;
use crate::agent::AgentLoop;
use crate::agent::style::*;
use crate::channels::{CliChannel, WsGateway, TelegramChannel, DiscordChannel, WhatsAppChannel, EmailChannel, Channel};
use crate::cron::scheduler::start_scheduler;

#[allow(unused_macros)]
macro_rules! println {
    () => {
        crate::tui_println!()
    };
    ($($arg:tt)*) => {
        crate::tui_println!($($arg)*)
    };
}

#[allow(unused_macros)]
macro_rules! print {
    () => {
        crate::tui_print!()
    };
    ($($arg:tt)*) => {
        crate::tui_print!($($arg)*)
    };
}

#[allow(unused_macros)]
macro_rules! eprintln {
    () => {
        crate::tui_println!()
    };
    ($($arg:tt)*) => {
        crate::tui_println!($($arg)*)
    };
}

#[allow(unused_macros)]
macro_rules! eprint {
    () => {
        crate::tui_print!()
    };
    ($($arg:tt)*) => {
        crate::tui_print!($($arg)*)
    };
}

static IS_SILENT_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_silent_mode() -> bool {
    IS_SILENT_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Parser)]
#[command(name = "openz", version = env!("CARGO_PKG_VERSION"), about = "OpenZ - Rebranded Ultra-Lightweight Personal AI Agent")]
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
    Gateway {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },

    /// Start the Telegram bot listener
    Telegram {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },

    /// Start the Discord bot listener
    Discord {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },

    /// Start the WhatsApp API/webhook listener
    Whatsapp {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },

    /// Start the Email polling listener
    Email {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },

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

    /// Stateful Event Workflow (SOP) Engine commands
    Sop {
        #[command(subcommand)]
        action: SopAction,
    },

    /// View real-time structured logs in a live TUI viewer
    Logs {
        /// Path to a specific log file (default: ~/.openz/openz.log)
        #[arg(long, short)]
        path: Option<std::path::PathBuf>,

        /// Number of historical lines to load on startup
        #[arg(long, short, default_value = "0")]
        tail: usize,

        /// Filter log output to a specific session key prefix.
        /// e.g. --session cli   --session gateway   --session telegram   --session auto
        #[arg(long, short)]
        session: Option<String>,

        /// Filter log output by level (error, warn, info, debug, trace)
        #[arg(long, short = 'l')]
        level: Option<String>,
    },

    /// View the OpenZ changelog and version release details
    Changelog,

    /// Configure response streaming preference via a wizard
    Streaming,
}

#[derive(Subcommand)]
pub enum SopAction {
    /// List all standard operating procedures (SOPs)
    List,

    /// List all SOP execution instances
    Instances,

    /// Trigger a standard operating procedure (SOP)
    Trigger {
        /// The ID of the SOP template
        sop_id: String,

        /// Optional JSON payload string or file path containing payload
        payload: Option<String>,
    },

    /// Resume a failed SOP instance
    Resume {
        /// The unique ID of the SOP instance
        instance_id: String,
    },

    /// Simulate a standard operating procedure (SOP) execution (dry-run)
    Simulate {
        /// The ID of the SOP template
        sop_id: String,

        /// Optional JSON payload string or file path containing payload
        payload: Option<String>,
    },
}

/// Sub-actions available on channel commands (e.g. `openz telegram logs`).
#[derive(Subcommand)]
pub enum ChannelAction {
    /// Stream live logs for this channel only
    Logs {
        /// Number of historical lines to load on startup
        #[arg(long, short, default_value = "0")]
        tail: usize,
    },
}

pub async fn run_cli() -> Result<()> {
    // Intercept version flags for custom themed print
    for arg in std::env::args() {
        if arg == "--version" || arg == "-V" {
            let logo = format!(
                r#"{white}     ██████╗ ██████╗ ███████╗███╗   ██╗{orange}███████╗
{white}    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║{orange}╚══███╔╝
{white}    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║{orange}  ███╔╝
{white}    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║{orange} ███╔╝
{white}    ╚██████╔╝██║     ███████╗██║ ╚████║{orange}███████╗
{white}     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝{orange}╚══════╝
{orange}openz v{version}{reset}
"#,
                white = crate::agent::style::colors::LIGHT_WHITE,
                orange = crate::agent::style::colors::RED_ORANGE,
                reset = crate::agent::style::colors::COLOR_RESET,
                version = env!("CARGO_PKG_VERSION")
            );
            print!("{}", logo);
            std::process::exit(0);
        }
    }

    let args = CliArgs::parse();

    // Startup garbage collection of stale git worktrees and temporary resources
    crate::tools::subagent::cleanup_stale_resources();
    
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
                Command::Gateway { action } => {
            match action {
                Some(ChannelAction::Logs { tail }) => {
                    handle_logs(None, tail, Some("gateway".to_string()), None).await?;
                }
                None => handle_gateway().await?,
            }
        }
        Command::Telegram { action } => {
            match action {
                Some(ChannelAction::Logs { tail }) => {
                    handle_logs(None, tail, Some("telegram".to_string()), None).await?;
                }
                None => handle_telegram().await?,
            }
        }
        Command::Discord { action } => {
            match action {
                Some(ChannelAction::Logs { tail }) => {
                    handle_logs(None, tail, Some("discord".to_string()), None).await?;
                }
                None => handle_discord().await?,
            }
        }
        Command::Whatsapp { action } => {
            match action {
                Some(ChannelAction::Logs { tail }) => {
                    handle_logs(None, tail, Some("whatsapp".to_string()), None).await?;
                }
                None => handle_whatsapp().await?,
            }
        }
        Command::Email { action } => {
            match action {
                Some(ChannelAction::Logs { tail }) => {
                    handle_logs(None, tail, Some("email".to_string()), None).await?;
                }
                None => handle_email().await?,
            }
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
            // Bind a port guard to keep the port reserved, same as find_free_port() does
            let port_guard = std::net::TcpListener::bind(format!("127.0.0.1:{}", port))
                .map_err(|e| anyhow!("Cannot bind to port {}: {}", port, e))?;
            crate::tools::mcp::run_mcp_bridge(port, port_guard, command, args, rx).await?;
        }
        Command::Sop { action } => {
            handle_sop(action).await?;
        }
        Command::Logs { path, tail, session, level } => {
            handle_logs(path, tail, session, level).await?;
        }
        Command::Changelog => {
            handle_changelog().await?;
        }
        Command::Streaming => {
            handle_streaming().await?;
        }
    }
    
    Ok(())
}

async fn handle_logs(
    path: Option<std::path::PathBuf>,
    tail: usize,
    session: Option<String>,
    level: Option<String>,
) -> Result<()> {
    let filter = crate::logs::SessionFilter::from_opt(session.as_deref());
    let level_filter = crate::logs::LogLevelFilter::from_opt(level.as_deref());
    crate::logs::run_logs_viewer(path, tail, filter, level_filter).await
}

async fn handle_changelog() -> Result<()> {
    println!("{purple}=== OpenZ System Specifications & Changelog ==={reset}\n", purple = AURA_PURPLE, reset = COLOR_RESET);
    
    println!("{bold}📊 Hardware Footprint & Specifications:{reset}", bold = COLOR_BOLD, reset = COLOR_RESET);
    println!("  {blue}• ROM (Binary Size):{reset}   ~10 MB - 15 MB (optimized Rust binary)", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("  {blue}• RAM (Cloud Mode):{reset}    ~15 MB - 30 MB (remote vector embeddings & LLM APIs)", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("  {blue}• RAM (Local Mode):{reset}    ~200 MB+ (local ONNX embedding model loaded)", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("  {blue}• CPU Footprint:{reset}       0% when idle (Tokio async event-driven architecture)", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("  {blue}• Startup Speed:{reset}       < 5 ms boot-to-prompt speed", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("  {blue}• Inspired By:{reset}         hermes-agent, Zeroclaw, Nanobot, loops!, DOX, codegraph, tantivy, lancedb,", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("                         surrealdb, petgraph, sentrux, tree-sitter-graph, mistral.rs, agentgateway,");
    println!("                         cowork-forge, openhuman, mcp-rust-sdk, wasserstein-agents, gsd-browser,");
    println!("                         chromewright, sediment, ClawDB, ferres-db, native-devtools-mcp,");
    println!("                         tokio-cron-scheduler, grpc-rust, mcp-searxng, searxng-mcp, opendocswork-mcp,");
    println!("                         slack-mcp-server, task-master, langgraph, crawl4ai, websurfx, headroom,");
    println!("                         rust-mcp-filesystem, novada-mcp, obscura, crawlee, katana, librefang,");
    println!("                         openmetadata, youtube-transcript-api, semble, deep-research, ocrs,");
    println!("                         agent-skills, superpowers, OpenMemory, SkillSpector, OpenHands, deer-flow,");
    println!("                         multica, ast-grep, caveman, graphify, notify, mcp-everything, mcp-memory,");
    println!("                         mcp-sequentialthinking, mcp-git, mcp-fetch, mcp-time, openfang\n");

    println!("{bold}⚡ Key Capabilities & Subsystems:{reset}", bold = COLOR_BOLD, reset = COLOR_RESET);
    println!("  {gold}1. Memory & Skill Self-Improvement Curator{reset}", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("     Asynchronously analyzes conversations to extract Tier 1 memory facts and Tier 2");
    println!("     procedural skills (stored in a SQLite database). Throttled to avoid wasteful LLM calls");
    println!("     on simple turns and limit stale skill clean-ups to once every 24 hours.");
    println!("  {gold}2. Native Compiler Auto-Healing{reset}", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("     `compiler_auto_heal` tool compiles code natively, reads stderr compiler errors,");
    println!("     and prompts the LLM to fix syntax or borrow checker issues in a loop until green.");
    println!("  {gold}3. Stateful SOP Workflow Engine{reset}", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("     Executes multi-step Directed Acyclic Graph (DAG) procedures like `ship-pr-until-green`.");
    println!("  {gold}4. Pluggable Channel Adapters{reset}", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("     Operates concurrently via Console TUI, WebSocket, Telegram, Discord, WhatsApp, and Email.");
    println!("  {gold}5. Security Guard & Subprocess BPF Sandbox{reset}", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("     Intercepts destructive commands and sandboxes subprocesses using seccomp BPF filters.");
    println!("  {gold}6. Startup Resource Clean-up{reset}", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("     Auto-prunes stale git worktrees and temporary workspaces to keep disk ROM footprint low.\n");

    println!("{bold}🔌 Model Context Protocol (MCP) Integration:{reset}", bold = COLOR_BOLD, reset = COLOR_RESET);
    println!("  OpenZ integrates with MCP servers using Stdio JSON-RPC or an in-process gRPC Tonic bridge.");
    println!("  {blue}• headroom:{reset}          Runs `scope_context` to scan directory trees for local `AGENTS.md` rules.", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("  {blue}• office:{reset}            Extracts text structures/tables from `.docx`, `.xlsx`, and `.pptx`.", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("  {blue}• sequential-thinking:{reset} Allows the model to plan and think logically before code edits.", blue = AURA_BLUE, reset = COLOR_RESET);
    println!("  {blue}• memory:{reset}            Semantic entity-relationship knowledge graph database.\n", blue = AURA_BLUE, reset = COLOR_RESET);

    println!("{bold}🔧 Core Native Tools & Usages:{reset}", bold = COLOR_BOLD, reset = COLOR_RESET);
    println!("  {gold}• Filesystem:{reset}         `read_file`, `write_file`, `patch_file`, `list_dir`, `grep_search`, `code_outline`", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Browsing & Web:{reset}     `web_search` (Tavily), `web_fetch`, `crawl_website` (spider-rs), `gsd_browser` (Playwright)", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Graphics & Video:{reset}   `generate_mermaid` (SVG renderer), `generate_video` (wavyte), `image_generator` (PNG)", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Task & Automation:{reset}  `delegate_task` (isolated subagent), `trigger_sop` (workflow engine), `schedule_job` (cron)", gold = AURA_GOLD, reset = COLOR_RESET);
    println!("  {gold}• Shell & Code:{reset}       `exec_command` (sandboxed), `wasm_sandbox` (wasmtime), `cargo_manager`, `js_format`\n", gold = AURA_GOLD, reset = COLOR_RESET);

    println!("{bold}📅 Version Release History:{reset}", bold = COLOR_BOLD, reset = COLOR_RESET);
    
    println!("  {green}[v0.0.14] - Current Release{reset}", green = AURA_GREEN, reset = COLOR_RESET);
    println!("    • Implemented incremental session saving to disk to prevent data loss on early command cancellation.");
    println!("    • Added print_session_history to render previous messages and tool runs when starting/switching sessions.");
    
    println!("  {slate}[v0.0.13]{reset}", slate = AURA_SLATE, reset = COLOR_RESET);
    println!("    • Configured separate tracing-subscriber layers to prevent ANSI escape code log pollution.");
    println!("    • Aligned default log path resolution with OPENZ_CONFIG_DIR customization.");
    println!("    • Changed logs tail default value to 0 to only show real-time stream logs by default.");
    println!("    • Corrected double caret typo in context compactor backtrace regex.");
    
    println!("  {slate}[v0.0.12]{reset}", slate = AURA_SLATE, reset = COLOR_RESET);
    println!("    • Made the OpenZ agent system prompt aware of its creator (Aswin), inspirations, specifications, features, and `changelog` command.");
    println!("    • Updated README.md documentation for the `changelog` command.");
    println!("    • Staged and committed all outstanding code changes and version bump to GitHub.");
    
    println!("  {slate}[v0.0.11]{reset}", slate = AURA_SLATE, reset = COLOR_RESET);
    println!("    • Added `openz changelog` command and root `CHANGELOG.md` file.");
    println!("    • Implemented Curator and Archival Throttling (reducing context & API token usage).");
    println!("    • Added Cloud-First Embeddings with remote prioritize and a `cloud_only` low-RAM mode.");
    println!("    • Added native compiler auto-healing (`CompilerAutoHealTool`).");
    println!("    • Added automatic workspace clean-up to purge stale git worktrees on boot.");
    println!("    • Added `--low-resource` flag to build/update scripts to throttle memory & CPU.");
    println!("    • Configured Cargo.toml release profile (codegen-units, LTO, stripping) to natively limit compilation RAM.");
    
    println!("  {slate}[v0.0.10]{reset}", slate = AURA_SLATE, reset = COLOR_RESET);
    println!("    • SQLite backend database migration (`~/.openz/memory.db`).");
    println!("    • Structural repository semantic indexing using `ast_grep` & vector embeddings.");
    println!("    • Added `mermaid_designer` subagent to generate SVG flowcharts.");
    
    println!("  {slate}[v0.0.9]{reset}", slate = AURA_SLATE, reset = COLOR_RESET);
    println!("    • Cryptographic Merkle Hash-Chain ledger (`/audit` command).");
    println!("    • WhatsApp Axum webhook receiver channel adapter.");
    println!("    • Dynamic assistant auto-continuation for response truncation.");

    println!("\n{slate}For the full changelog details, please refer to: {reset}{bold}CHANGELOG.md{reset}\n", slate = AURA_SLATE, reset = COLOR_RESET, bold = COLOR_BOLD);
    
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

    let mut config = load_config().unwrap_or_else(|_| Config::default());
    
    let p_config = Some(ProviderConfig {
        api_key,
        api_base,
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
    let resolved = crate::providers::resolver::resolve_provider_full(&config, &config.agents.defaults.model)?;
    let provider = resolved.instance;
    
    let sessions_dir = resolve_path("~/.openz/sessions");
    let session_manager = SessionManager::new(sessions_dir);

    let registry = ToolRegistry::new_with_context(config.clone(), provider.clone(), session_manager.clone());
    registry.register(std::sync::Arc::new(ReadFileTool));
    registry.register(std::sync::Arc::new(FindFilesTool));
    registry.register(std::sync::Arc::new(DocReaderTool));
    registry.register(std::sync::Arc::new(WasmSandboxTool));
    registry.register(std::sync::Arc::new(JsFormatTool));
    registry.register(std::sync::Arc::new(SemanticSearchTool));
    registry.register(std::sync::Arc::new(StoreMemoryTool));
    registry.register(std::sync::Arc::new(RecallMemoryTool));
    registry.register(std::sync::Arc::new(ClearMemoryTool));
    registry.register(std::sync::Arc::new(ArchiveResearchTool));
    registry.register(std::sync::Arc::new(SearchResearchTool));
    registry.register(std::sync::Arc::new(ZenflowEditTool { provider: provider.clone() }));
    registry.register(std::sync::Arc::new(PythonSandboxTool));
    registry.register(std::sync::Arc::new(RustDocsTool::new()));
    registry.register(std::sync::Arc::new(WriteFileTool));
    registry.register(std::sync::Arc::new(PatchFileTool));
    registry.register(std::sync::Arc::new(ReplaceLinesTool));
    registry.register(std::sync::Arc::new(ListDirTool));
    registry.register(std::sync::Arc::new(ExecCommandTool));
    registry.register(std::sync::Arc::new(WebFetchTool::new()));
    registry.register(std::sync::Arc::new(DelegateTaskTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
        parent_tools: Vec::new(),
        cancellation_token: crate::tools::subagent::CancellationToken::new(),
    }));
    registry.register(std::sync::Arc::new(ParallelResearchTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
        parent_tools: Vec::new(),
        cancellation_token: crate::tools::subagent::CancellationToken::new(),
    }));
    registry.register(std::sync::Arc::new(crate::tools::subagent::EvaluatorOptimizerLoopTool {
        config: config.clone(),
        parent_provider: provider.clone(),
        session_manager: session_manager.clone(),
        parent_tools: Vec::new(),
        cancellation_token: crate::tools::subagent::CancellationToken::new(),
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
    registry.register(std::sync::Arc::new(crate::tools::github::GitProviderTool));
    registry.register(std::sync::Arc::new(crate::tools::outline::CodeOutlineTool));
    registry.register(std::sync::Arc::new(DbInspectorTool));
    registry.register(std::sync::Arc::new(DbWriteTool));
    registry.register(std::sync::Arc::new(SystemInfoTool));
    registry.register(std::sync::Arc::new(CheckPortTool));
    registry.register(std::sync::Arc::new(crate::tools::cargo_manager::CargoManagerTool::new(provider.clone())));
    registry.register(std::sync::Arc::new(crate::tools::clipboard::ClipboardTool));
    registry.register(std::sync::Arc::new(crate::tools::open::OpenTool));
    registry.register(std::sync::Arc::new(crate::tools::watcher::FileWatcherTool));
    registry.register(std::sync::Arc::new(crate::tools::ast_grep::AstGrepTool));
    registry.register(std::sync::Arc::new(crate::tools::ast_grep::IndexCodebaseTool));
    registry.register(std::sync::Arc::new(crate::tools::gsd_browser::GsdBrowserTool));
    registry.register(std::sync::Arc::new(crate::tools::web_search::WebSearchTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::onpkg::OnpkgTool));
    registry.register(std::sync::Arc::new(crate::tools::image_generator::GenerateImageTool));
    registry.register(std::sync::Arc::new(crate::tools::crawl::CrawlSiteTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::obscura::ObscuraBrowserTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::firefox::FirefoxBrowserTool::new()));
    registry.register(std::sync::Arc::new(IndexNotesTool));
    registry.register(std::sync::Arc::new(SocialSearchTool::new()));
    registry.register(std::sync::Arc::new(crate::tools::template_compiler::CompileTemplateTool));
    registry.register(std::sync::Arc::new(crate::tools::mermaid::MermaidRendererTool));
    registry.register(std::sync::Arc::new(crate::tools::video::VideoGeneratorTool));
    registry.register(std::sync::Arc::new(crate::tools::html_video::HtmlToVideoTool));
    registry.register(std::sync::Arc::new(crate::tools::svg_animator::SvgAnimatorTool));
    registry.register(std::sync::Arc::new(crate::tools::sop::TriggerSopTool { config: config.clone() }));
    registry.register(std::sync::Arc::new(crate::tools::compiler_auto_heal::CompilerAutoHealTool {
        config: config.clone(),
        provider: provider.clone(),
    }));

    // ── MCP: lazy registration ────────────────────────────────────────────────
    // Phase 1 (now): Spawn a background task per MCP server that connects,
    //                fetches the tool schema, and registers LazyMcpToolWrapper
    //                stubs. The agent loop starts immediately — no blocking.
    // Phase 2 (on call): LazyMcpToolWrapper::call() re-uses the already-alive
    //                    client (fast path) or spawns on first use (slow path).

    let silent = is_silent_mode();

    let has_any_mcp = config.mcp_servers.values().any(|c| c.enabled);

    if has_any_mcp {
        tracing::info!("Setting up MCP servers (background)...");
    }

    // Collect enabled servers for the background task
    let mcp_configs: Vec<(String, crate::config::schema::McpServerConfig)> = config
        .mcp_servers
        .iter()
        .filter(|(_, c)| c.enabled)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let registry_bg = registry.clone();
    let num_configs = mcp_configs.len() as u32;

    let _mcp_handle = tokio::spawn(async move {
        if !silent {
            crate::channels::cli::init_mcp_progress(num_configs);
        }

        let mut servers_loaded = 0u32;
        let mut servers_failed = 0u32;

        let mut tasks = Vec::new();
        for (name, mcp_config) in mcp_configs {
            let registry_bg = registry_bg.clone();
            tasks.push(tokio::spawn(async move {
                let name_clone = name.clone();
                let mcp_config_clone = mcp_config.clone();
                let result = tokio::time::timeout(std::time::Duration::from_secs(15), async {
                    let mcp_client = crate::tools::mcp::McpClient::spawn(&mcp_config_clone.command, &mcp_config_clone.args).await?;
                    if name_clone == "memory" {
                        crate::tools::mcp::set_memory_mcp_client(mcp_client.clone());
                    }
                    let tools = mcp_client.list_tools().await?;
                    Ok::<_, anyhow::Error>(tools)
                }).await;

                match result {
                    Ok(Ok(tools)) => {
                        if !silent {
                            crate::channels::cli::increment_mcp_loaded();
                        }
                        let mut count = 0;
                        for t in tools {
                            if let (Some(t_name), Some(desc)) = (
                                t.get("name").and_then(|v| v.as_str()),
                                t.get("description").and_then(|v| v.as_str()),
                            ) {
                                let params = t.get("inputSchema").cloned().unwrap_or(
                                    serde_json::json!({"type": "object", "properties": {}})
                                );
                                let wrapper = crate::tools::mcp::LazyMcpToolWrapper {
                                    server_name: name_clone.clone(),
                                    command: mcp_config_clone.command.clone(),
                                    args: mcp_config_clone.args.clone(),
                                    name: t_name.to_string(),
                                    description: desc.to_string(),
                                    parameters: params,
                                    is_memory_server: name_clone == "memory",
                                };
                                registry_bg.register(std::sync::Arc::new(wrapper));
                                count += 1;
                            }
                        }
                        Ok::<usize, anyhow::Error>(count)
                    }
                    Ok(Err(e)) => {
                        if !silent {
                            crate::channels::cli::increment_mcp_failed();
                        }
                        tracing::error!("Failed starting MCP server {}: {:?}", name_clone, e);
                        Err(e)
                    }
                    Err(elapsed) => {
                        if !silent {
                            crate::channels::cli::increment_mcp_failed();
                        }
                        tracing::error!("Timed out starting MCP server {} after 15s: {:?}", name_clone, elapsed);
                        Err(anyhow::anyhow!("Timed out starting MCP server {}: {:?}", name_clone, elapsed))
                    }
                }
            }));
        }

        let results = futures_util::future::join_all(tasks).await;
        for res in results {
            match res {
                Ok(Ok(_count)) => {
                    servers_loaded += 1;
                }
                _ => {
                    servers_failed += 1;
                }
            }
        }

        // Update the status bar pill — the render loop reads these atomics every redraw
        if !silent {
            crate::channels::cli::set_mcp_status(servers_loaded, servers_failed);
            crate::channels::cli::set_mcp_done();
        }

        if has_any_mcp {
            crate::tools::mcp::start_mcp_health_checks();
        }
    });

    Ok(AgentLoop::new(config, provider, registry, session_manager))
}




#[derive(serde::Deserialize)]
struct SessionMetadataOnly {
    key: String,
    updated_at: chrono::DateTime<chrono::Utc>,
    messages: Vec<MessageMetadataOnly>,
}

#[derive(serde::Deserialize)]
struct MessageMetadataOnly {
    role: String,
    content: String,
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
                if let Ok(session) = serde_json::from_str::<SessionMetadataOnly>(&content) {
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
            if selected_item.key != "cli:direct" {
                archive_current_session(&session_manager)?;
                let mut session = session_manager.load(&selected_item.key)?;
                session.key = "cli:direct".to_string();
                session_manager.save(&session)?;
            }
        }
    }

    let agent_loop = build_agent_loop(config.clone()).await?;

    // Mark silent mode for background channels via thread-safe AtomicBool
    IS_SILENT_MODE.store(true, std::sync::atomic::Ordering::Relaxed);

    // Auto-start WebSocket gateway in the background if enabled and configured to start on TUI
    if let Some(ws_config) = &config.channels.websocket {
        if ws_config.enabled && ws_config.start_on_tui {
            let config_clone = config.clone();
            let ws_config_clone = ws_config.clone();
            tokio::spawn(async move {
                if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                    let gateway = WsGateway::new(ws_config_clone, agent_loop);
                    let _ = gateway.start().await;
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
                    if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                        let channel = TelegramChannel::new(token, agent_loop);
                        let _ = channel.start().await;
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
                    if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                        let channel = DiscordChannel::new(token, agent_loop);
                        let _ = channel.start().await;
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
                if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                    let channel = WhatsAppChannel::new(
                        wa_config_clone.api_key,
                        wa_config_clone.phone_number_id,
                        agent_loop,
                    );
                    let _ = channel.start().await;
                }
            });
        }
    }

    // Auto-start Email channel in the background if enabled
    if let Some(email_config) = &config.channels.email {
        if email_config.enabled {
            let config_clone = config.clone();
            tokio::spawn(async move {
                if let Ok(agent_loop) = build_agent_loop(config_clone).await {
                    let channel = EmailChannel::new(agent_loop);
                    let _ = channel.start().await;
                }
            });
        }
    }

    let channel = CliChannel::new(agent_loop, defaults);
    let mut shutdown_rx = match crate::shutdown::receiver() {
        Some(rx) => rx,
        None => {
            let (_, rx) = tokio::sync::watch::channel(false);
            rx
        }
    };

    tokio::select! {
        res = channel.start() => {
            if let Err(e) = res {
                eprintln!("TUI error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\r\nExiting OpenZ...");
        }
        _ = shutdown_rx.changed() => {
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

async fn handle_email() -> Result<()> {
    let config = load_config()?;
    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = crate::channels::EmailChannel::new(agent_loop);
    
    tokio::select! {
        res = channel.start() => {
            if let Err(e) = res {
                eprintln!("Email error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\r\nExiting Email channel...");
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
        "cerebras" => config.providers.cerebras.clone(),
        "google_ai_studio" => config.providers.google_ai_studio.clone(),
        "cohere" => config.providers.cohere.clone(),
        "llm7" => config.providers.llm7.clone(),
        "sambanova" => config.providers.sambanova.clone(),
        "huggingface" => config.providers.huggingface.clone(),
        _ => return,
    }.unwrap_or_else(|| ProviderConfig {
        api_key: None,
        api_base: None,
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
            "cerebras" => "https://api.cerebras.ai/v1",
            "google_ai_studio" => "https://generativelanguage.googleapis.com/v1beta/openai/",
            "cohere" => "https://api.cohere.com/v1",
            "llm7" => "https://token.llm7.io/v1",
            "sambanova" => "https://api.sambanova.ai/v1",
            "huggingface" => "https://api-inference.huggingface.co/v1",
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
        "cerebras" => config.providers.cerebras = Some(p_config),
        "google_ai_studio" => config.providers.google_ai_studio = Some(p_config),
        "cohere" => config.providers.cohere = Some(p_config),
        "llm7" => config.providers.llm7 = Some(p_config),
        "sambanova" => config.providers.sambanova = Some(p_config),
        "huggingface" => config.providers.huggingface = Some(p_config),
        _ => {}
    }
}

fn is_telegram_configured(config: &Config) -> bool {
    if let Some(ref tg) = config.channels.telegram {
        !tg.bot_token.is_empty()
    } else {
        false
    }
}

fn is_email_configured(config: &Config) -> bool {
    if let Some(ref em) = config.channels.email {
        em.enabled && !em.imap_server.is_empty() && !em.username.is_empty()
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

        if is_email_configured(&config) {
            configure_options.push("Email (configured)".to_string());
        } else {
            configure_options.push("Email".to_string());
        }

        if config.agents.defaults.enable_sandbox {
            configure_options.push("Sandbox (seccomp) (enabled)".to_string());
        } else {
            configure_options.push("Sandbox (seccomp) (disabled)".to_string());
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
            5 => {
                handle_email_submenu(&mut config).await?;
            }
            6 => {
                handle_sandbox_submenu(&mut config).await?;
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
        ProviderInfo { name: "cerebras", display: "Cerebras" },
        ProviderInfo { name: "google_ai_studio", display: "Google AI Studio (Gemini)" },
        ProviderInfo { name: "cohere", display: "Cohere" },
        ProviderInfo { name: "llm7", display: "LLM7 (token.llm7.io)" },
        ProviderInfo { name: "sambanova", display: "SambaNova" },
        ProviderInfo { name: "huggingface", display: "Hugging Face Inference" },
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

        let port_str = Text::new("Enter WhatsApp Webhook Port: ")
            .with_default(&wa.webhook_port.to_string())
            .prompt()?;
        if let Ok(p) = port_str.trim().parse::<u16>() {
            wa.webhook_port = p;
        }

        let token = Text::new("Enter WhatsApp Webhook Verification Token: ")
            .with_default(&wa.verify_token)
            .prompt()?;
        if !token.trim().is_empty() {
            wa.verify_token = token.trim().to_string();
        }
    }

    config.channels.whatsapp = Some(wa);
    save_config(config)?;
    println!("{}✓ WhatsApp channel configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}

async fn handle_email_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Email Channel Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    
    let mut em = config.channels.email.clone().unwrap_or_default();
    
    let enabled = Confirm::new("Enable Email channel?")
        .with_default(em.enabled)
        .prompt()?;
    em.enabled = enabled;

    if enabled {
        let imap_server = Text::new("Enter IMAP Server (e.g. imap.gmail.com): ")
            .with_default(&em.imap_server)
            .prompt()?;
        if !imap_server.trim().is_empty() {
            em.imap_server = imap_server.trim().to_string();
        }

        let imap_port_str = Text::new("Enter IMAP Port: ")
            .with_default(&em.imap_port.to_string())
            .prompt()?;
        if let Ok(p) = imap_port_str.trim().parse::<u16>() {
            em.imap_port = p;
        }

        let smtp_server = Text::new("Enter SMTP Server (e.g. smtp.gmail.com): ")
            .with_default(&em.smtp_server)
            .prompt()?;
        if !smtp_server.trim().is_empty() {
            em.smtp_server = smtp_server.trim().to_string();
        }

        let smtp_port_str = Text::new("Enter SMTP Port: ")
            .with_default(&em.smtp_port.to_string())
            .prompt()?;
        if let Ok(p) = smtp_port_str.trim().parse::<u16>() {
            em.smtp_port = p;
        }

        let username = Text::new("Enter Username / Email: ")
            .with_default(&em.username)
            .prompt()?;
        if !username.trim().is_empty() {
            em.username = username.trim().to_string();
        }

        let password = Password::new("Enter Password / App Password: ")
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        if !password.trim().is_empty() {
            em.password = password.trim().to_string();
        }

        let poll_str = Text::new("Enter Email Poll Interval (seconds): ")
            .with_default(&em.poll_interval_secs.to_string())
            .prompt()?;
        if let Ok(s) = poll_str.trim().parse::<u64>() {
            em.poll_interval_secs = s;
        }
    }

    config.channels.email = Some(em);
    save_config(config)?;
    println!("{}✓ Email channel configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}


async fn handle_sandbox_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Sandbox (seccomp) Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    println!("The process sandbox (seccomp) restricts system calls (network, browser, tools like ps/which) inside the command execution sandbox.");
    println!("Disabling it allows browser automation tools (gsd_browser, chromewright) and local compiler tools to run without seccomp blocking.");
    println!("Security is still enforced via openz's internal SecurityGuard prompt confirmations.");
    println!();
    
    let current = config.agents.defaults.enable_sandbox;
    let enable = Confirm::new("Enable process seccomp sandbox?")
        .with_default(current)
        .prompt()?;
        
    config.agents.defaults.enable_sandbox = enable;
    save_config(config)?;
    
    if enable {
        println!("{}✓ Sandbox enabled successfully!{}", EMERALD_GREEN, COLOR_RESET);
    } else {
        println!("{}✓ Sandbox disabled successfully! (Highly recommended for browser tools and developer shells){}", EMERALD_GREEN, COLOR_RESET);
    }
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    
    // Give the user a moment to see the success message
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    
    Ok(())
}

async fn handle_sop(action: SopAction) -> Result<()> {
    match action {
        SopAction::List => {
            let defs = crate::sop::load_definitions()?;
            println!("\n{}📋 Available Standard Operating Procedures (SOPs):{}\n", COLOR_BOLD, COLOR_RESET);
            if defs.is_empty() {
                println!("No SOP definitions found.");
            } else {
                for def in defs {
                    println!("{}• ID:{} {}", AURA_PURPLE, COLOR_RESET, def.id);
                    println!("  {}Name:{} {}", COLOR_BOLD, COLOR_RESET, def.name);
                    println!("  {}Description:{} {}", COLOR_BOLD, COLOR_RESET, def.description);
                    println!("  {}Steps:{}", COLOR_BOLD, COLOR_RESET);
                    for (i, step) in def.steps.iter().enumerate() {
                        let deps_str = if step.depends_on.is_empty() {
                            String::new()
                        } else {
                            format!(" [Depends on: {}]", step.depends_on.join(", "))
                        };
                        println!("    {}. {}{}: {}", i + 1, step.name, deps_str, step.description);
                    }
                    println!();
                }
            }
        }
        SopAction::Instances => {
            let instances = crate::sop::list_instances()?;
            println!("\n{}📋 SOP Execution Instances:{}\n", COLOR_BOLD, COLOR_RESET);
            if instances.is_empty() {
                println!("No SOP instances executed yet.");
            } else {
                for inst in instances {
                    let status_color = match inst.status {
                        crate::sop::SopStatus::Completed => EMERALD_GREEN,
                        crate::sop::SopStatus::Failed => ERROR_RED,
                        crate::sop::SopStatus::Running => LIGHT_WHITE,
                        _ => COLOR_RESET,
                    };
                    println!("{}• Instance ID:{} {}", AURA_PURPLE, COLOR_RESET, inst.id);
                    println!("  {}SOP ID:{} {}", COLOR_BOLD, COLOR_RESET, inst.sop_id);
                    println!("  {}Status:{} {:?}{}", COLOR_BOLD, status_color, inst.status, COLOR_RESET);
                    println!("  {}Current Step:{} {}/{}", COLOR_BOLD, COLOR_RESET, inst.current_step_index, inst.steps.len());
                    println!("  {}Started At:{} {}", COLOR_BOLD, COLOR_RESET, inst.started_at);
                    if let Some(ref completed) = inst.completed_at {
                        println!("  {}Completed At:{} {}", COLOR_BOLD, COLOR_RESET, completed);
                    }
                    println!();
                }
            }
        }
        SopAction::Trigger { sop_id, payload } => {
            let config = load_config()?;
            let payload_value = if let Some(p) = payload {
                let p_trimmed = p.trim();
                if p_trimmed.starts_with('{') || p_trimmed.starts_with('[') {
                    serde_json::from_str(p_trimmed)?
                } else {
                    // Try parsing as file path
                    let path = std::path::Path::new(p_trimmed);
                    if path.exists() {
                        let content = std::fs::read_to_string(path)?;
                        serde_json::from_str(&content)?
                    } else {
                        anyhow::bail!("Payload must be a valid JSON string or path to a JSON file");
                    }
                }
            } else {
                serde_json::json!({})
            };

            println!("Triggering SOP '{}'...", sop_id);
            match crate::sop::engine::trigger_sop(config, sop_id.clone(), payload_value).await {
                Ok(instance_id) => {
                    println!("{}✓ SOP successfully triggered!{}", EMERALD_GREEN, COLOR_RESET);
                    println!("Instance ID: {}", instance_id);
                }
                Err(e) => {
                    eprintln!("{}❌ Failed to trigger SOP: {}{}", ERROR_RED, e, COLOR_RESET);
                }
            }
        }
        SopAction::Resume { instance_id } => {
            let config = load_config()?;
            println!("Resuming SOP instance '{}'...", instance_id);
            match crate::sop::engine::resume_sop(config, instance_id.clone()).await {
                Ok(_) => {
                    println!("{}✓ SOP instance resume initiated successfully!{}", EMERALD_GREEN, COLOR_RESET);
                }
                Err(e) => {
                    eprintln!("{}❌ Failed to resume SOP: {}{}", ERROR_RED, e, COLOR_RESET);
                }
            }
        }
        SopAction::Simulate { sop_id, payload } => {
            let config = load_config()?;
            let payload_value = if let Some(p) = payload {
                let p_trimmed = p.trim();
                if p_trimmed.starts_with('{') || p_trimmed.starts_with('[') {
                    serde_json::from_str(p_trimmed)?
                } else {
                    let path = std::path::Path::new(p_trimmed);
                    if path.exists() {
                        let content = std::fs::read_to_string(path)?;
                        serde_json::from_str(&content)?
                    } else {
                        anyhow::bail!("Payload must be a valid JSON string or path to a JSON file");
                    }
                }
            } else {
                serde_json::json!({})
            };

            println!("Simulating SOP '{}'...", sop_id);
            match crate::sop::engine::trigger_sop_simulation(config, sop_id.clone(), payload_value).await {
                Ok(instance_id) => {
                    println!("{}✓ SOP simulation finished successfully!{}", EMERALD_GREEN, COLOR_RESET);
                    println!("Simulated Instance ID: {}", instance_id);
                }
                Err(e) => {
                    eprintln!("{}❌ Failed to simulate SOP: {}{}", ERROR_RED, e, COLOR_RESET);
                }
            }
        }
    }
    Ok(())
}

async fn handle_streaming() -> Result<()> {
    let mut config = load_config()?;
    let current_status = if config.agents.defaults.streaming {
        "Enabled"
    } else {
        "Disabled"
    };

    println!("{}◇ OpenZ Response Streaming Wizard{}", COLOR_BOLD, COLOR_RESET);
    println!("Current status: {}{}{}\r\n", RED_ORANGE, current_status, COLOR_RESET);
    println!("Streaming prints response chunks in real-time. However, keeping it disabled");
    println!("is highly recommended for unstable or rate-limited API gateways (like OpenCode Zen)");
    println!("to avoid early cut-offs or connection drops.\r\n");

    let options = vec![
        "Enable streaming (globally)".to_string(),
        "Disable streaming (globally)".to_string(),
        "Exit".to_string(),
    ];

    let choice = Select::new("Choose option:", options).prompt()?;

    if choice.starts_with("Enable") {
        config.agents.defaults.streaming = true;
        save_config(&config)?;
        println!("{}✓ Response streaming has been ENABLED globally for OpenZ.{}", EMERALD_GREEN, COLOR_RESET);
    } else if choice.starts_with("Disable") {
        config.agents.defaults.streaming = false;
        save_config(&config)?;
        println!("{}✓ Response streaming has been DISABLED globally for OpenZ.{}", EMERALD_GREEN, COLOR_RESET);
    } else {
        println!("No changes made.");
    }

    Ok(())
}


