pub mod agent;
pub mod args;
pub mod builder;
pub mod changelog;
pub mod channels;
pub mod configure;
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
    handle_discord, handle_email, handle_gateway, handle_telegram, handle_whatsapp,
    is_email_configured, is_telegram_configured,
};
use clap::Parser;
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

    match args.command {
        Command::Onboard => {
            onboard::handle_onboard().await?;
        }
        Command::Configure => {
            configure::handle_configure().await?;
        }
        Command::Agent => {
            agent::handle_agent().await?;
        }
        Command::Gateway { action } => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("gateway".to_string()), None).await?;
            }
            None => channels::handle_gateway().await?,
        },
        Command::Telegram { action } => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("telegram".to_string()), None).await?;
            }
            None => channels::handle_telegram().await?,
        },
        Command::Discord { action } => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("discord".to_string()), None).await?;
            }
            None => channels::handle_discord().await?,
        },
        Command::Whatsapp { action } => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("whatsapp".to_string()), None).await?;
            }
            None => channels::handle_whatsapp().await?,
        },
        Command::Email { action } => match action {
            Some(ChannelAction::Logs { tail }) => {
                logs::handle_logs(None, tail, Some("email".to_string()), None).await?;
            }
            None => channels::handle_email().await?,
        },
        Command::Subagent => {
            let config = crate::config::loader::load_config()?;
            crate::subagents::run_subagent_manager(config).await?;
        }
        Command::McpBridge { port, command_args } => {
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
        Command::Sop { action } => {
            sop::handle_sop(action).await?;
        }
        Command::Logs {
            path,
            tail,
            session,
            level,
        } => {
            logs::handle_logs(path, tail, session, level).await?;
        }
        Command::Changelog => {
            changelog::handle_changelog().await?;
        }
        Command::Streaming => {
            streaming::handle_streaming().await?;
        }
    }
    Ok(())
}
