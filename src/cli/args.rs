use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "openz", version = env!("CARGO_PKG_VERSION"), about = "OpenZ - Rebranded Ultra-Lightweight Personal AI Agent")]
pub struct CliArgs {
    #[arg(short = 'p', long = "prompt", global = true)]
    pub prompt: Option<String>,

    #[arg(long, global = true)]
    pub output_format: Option<String>,

    #[arg(short = 'y', long = "yes", global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    #[command(alias = "exec")]
    Run(HeadlessArgs),
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
    Doctor {
        #[arg(long)]
        scrub_secrets: bool,
        #[arg(long)]
        clean_target: bool,
    },
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
        #[arg(index = 1)]
        session: Option<String>,
        #[arg(long, short)]
        level: Option<String>,
        #[arg(long, short)]
        global: bool,
        #[arg(long, short)]
        search: Option<String>,
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

#[derive(Parser, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HeadlessArgs {
    #[arg(index = 1)]
    pub prompt: Option<String>,

    #[arg(short = 'p', long = "prompt")]
    pub prompt_flag: Option<String>,

    #[arg(long, default_value = "text")]
    pub output_format: String,

    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    #[arg(long)]
    pub allowed_tools: Option<String>,

    #[arg(long)]
    pub session: Option<String>,

    #[arg(long)]
    pub r#continue: bool,

    #[arg(short = 'm', long = "model")]
    pub model: Option<String>,

    #[arg(long)]
    pub provider: Option<String>,

    #[arg(long)]
    pub max_iterations: Option<usize>,

    #[arg(long)]
    pub timeout: Option<u64>,
}
