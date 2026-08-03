pub mod agent;
pub mod args;
pub mod builder;
pub mod changelog;
pub mod channels;
pub mod configure;
pub mod doctor;
pub mod logs;
pub mod onboard;
pub mod sop;
pub mod streaming;
pub mod tools;

use crate::print;
pub use agent::{archive_current_session, load_session_history};
use anyhow::Result;
pub use args::{ChannelAction, CliArgs, Command, SopAction};
pub use builder::build_agent_loop;
pub use channels::{
    handle_discord, handle_email, handle_gateway, handle_ratatui_tui, handle_telegram, handle_whatsapp,
    is_email_configured, is_telegram_configured,
};
use clap::Parser;
pub use doctor::handle_doctor;
pub use logs::handle_logs;
pub use sop::handle_sop;

static IS_SILENT_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_silent_mode() -> bool {
    IS_SILENT_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_silent_mode(val: bool) {
    IS_SILENT_MODE.store(val, std::sync::atomic::Ordering::Relaxed);
}

pub async fn run_cli() -> Result<()> {
    // Intercept version flags
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
    crate::tools::subagent::cleanup_stale_resources();

    // Start background FastEmbed ONNX model eviction task (frees ~130 MB RAM after 5 min idle)
    crate::tools::shared_memory::start_model_eviction();

    // Start background SQLite vacuum & optimizer (reclaims unused disk pages after startup)
    start_database_auto_optimizer();

    // Surface (and offer to migrate) any stale runtime DB files left in the
    // working directory instead of letting them shadow the global database.
    crate::config::loader::check_root_runtime_dbs();

    match args.command {
        None => {
            channels::handle_ratatui_tui().await?;
        }
        Some(Command::Onboard) => {
            onboard::handle_onboard().await?;
        }
        Some(Command::Configure) => {
            configure::handle_configure().await?;
            let _ = crossterm::terminal::disable_raw_mode();
            std::process::exit(0);
        }
        Some(Command::Agent) => {
            agent::handle_agent().await?;
        }
        Some(Command::Gateway { action }) => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("gateway".to_string()), None, false, None)
                    .await?;
            }
            None => channels::handle_gateway().await?,
        },
        Some(Command::Telegram { action }) => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("telegram".to_string()), None, false, None)
                    .await?;
            }
            None => channels::handle_telegram().await?,
        },
        Some(Command::Discord { action }) => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("discord".to_string()), None, false, None)
                    .await?;
            }
            None => channels::handle_discord().await?,
        },
        Some(Command::Whatsapp { action }) => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("whatsapp".to_string()), None, false, None)
                    .await?;
            }
            None => channels::handle_whatsapp().await?,
        },
        Some(Command::Email { action }) => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("email".to_string()), None, false, None).await?;
            }
            None => channels::handle_email().await?,
        },
        Some(Command::Subagent) => {
            let config = crate::config::loader::load_config()?;
            crate::subagents::run_subagent_manager(config).await?;
        }
        Some(Command::Doctor { scrub_secrets }) => {
            doctor::handle_doctor(scrub_secrets).await?;
        }
        Some(Command::McpBridge { port, command_args }) => {
            if command_args.is_empty() {
                return Err(anyhow::anyhow!("No target command specified. Usage: openz mcp-bridge --port <port> -- <command> [args...]"));
            }
            let command = &command_args[0];
            let args = &command_args[1..];
            let (_tx, rx) = tokio::sync::oneshot::channel();
            let port_guard = std::net::TcpListener::bind(format!("127.0.0.1:{}", port))
                .map_err(|e| anyhow::anyhow!("Cannot bind to port {}: {}", port, e))?;
            crate::tools::mcp::run_mcp_bridge(port, port_guard, command, args, rx).await?;
        }
        Some(Command::Sop { action }) => {
            sop::handle_sop(action).await?;
        }
        Some(Command::Logs {
            path,
            tail,
            session,
            level,
            global,
            search,
        }) => {
            logs::handle_logs(path, tail, session, level, global, search).await?;
        }
        Some(Command::Changelog) => {
            changelog::handle_changelog().await?;
        }
        Some(Command::Streaming) => {
            streaming::handle_streaming().await?;
        }
    }
    Ok(())
}

/// Spawns a background task that performs incremental SQLite maintenance
/// (incremental_vacuum and optimize) 10 seconds after startup to reclaim unused space.
pub fn start_database_auto_optimizer() {
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let db_names = [
            "memory.db",
            "docs.db",
            "graph_memory.db",
            "ccr_cache.db",
            "context-bus.db",
            "logs.db",
            "thoughts.db",
            "embeddings_cache.db",
        ];
        for name in db_names {
            let path = crate::config::loader::runtime_db_path(name);
            if path.exists() {
                if let Ok(conn) = rusqlite::Connection::open(&path) {
                    let _ = conn.execute_batch("PRAGMA incremental_vacuum(50); PRAGMA optimize;");
                }
            }
        }
    });
}
