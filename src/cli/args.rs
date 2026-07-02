use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "openz", version = env!("CARGO_PKG_VERSION"), about = "OpenZ - Rebranded Ultra-Lightweight Personal AI Agent")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    Onboard,
    Configure,
    Agent,
    Gateway {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },
    Telegram {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },
    Discord {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },
    Whatsapp {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },
    Email {
        #[command(subcommand)]
        action: Option<ChannelAction>,
    },
    Subagent,
    McpBridge {
        #[arg(long)]
        port: u16,
        #[arg(last = true)]
        command_args: Vec<String>,
    },
    Sop {
        #[command(subcommand)]
        action: SopAction,
    },
    Logs {
        #[arg(long, short)]
        path: Option<PathBuf>,
        #[arg(long, short, default_value = "0")]
        tail: usize,
        #[arg(long, short)]
        session: Option<String>,
        #[arg(long, short)]
        level: Option<String>,
    },
    Changelog,
    Streaming,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ChannelAction {
    Logs {
        #[arg(long, short, default_value = "0")]
        tail: usize,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum SopAction {
    List,
    Instances,
    Trigger {
        sop_id: String,
        payload: Option<String>,
    },
    Resume {
        #[arg(long, short)]
        instance_id: String,
    },
    Simulate {
        sop_id: String,
        payload: Option<String>,
    },
}
