use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "openz", version = env!("CARGO_PKG_VERSION"), about = "OpenZ - Rebranded Ultra-Lightweight Personal AI Agent")]
pub struct CliArgs {
    #[arg(short = 'p', long = "prompt")]
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
    #[command(hide = true)]
    Agent,
    #[command(name = "webui", visible_alias = "web", alias = "gateway")]
    Webui {
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
    #[command(alias = "skill")]
    Skills {
        #[command(subcommand)]
        action: Option<SkillsAction>,
    },
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
    /// Launch OpenZ's native low-RAM WebKit2GTK browser (GUI window or headless renderer)
    Browser {
        /// Optional initial URL or local HTML file path
        #[arg(index = 1)]
        url: Option<String>,
        /// Run offscreen without opening a desktop window
        #[arg(long)]
        headless: bool,
        /// Force visible OpenZ Browser GUI window
        #[arg(long)]
        gui: bool,
        /// Named session identifier
        #[arg(long, default_value = "default")]
        session: String,
        /// Optional persistent storage profile name
        #[arg(long)]
        profile: Option<String>,
        /// Viewport preset ('desktop', 'tablet', 'mobile', '1080p') or 'WIDTHxHEIGHT'
        #[arg(long)]
        size: Option<String>,
        /// Memory pressure ceiling in MB (default from config: 128)
        #[arg(long)]
        memory_mb: Option<u32>,
        /// Save one-shot PNG screenshot to path and exit
        #[arg(long)]
        screenshot: Option<PathBuf>,
        /// Save one-shot multi-page PDF to path and exit
        #[arg(long)]
        pdf: Option<PathBuf>,
        /// Print JSON WebKit2GTK runtime availability probe and exit
        #[arg(long)]
        probe: bool,
        /// Print full JSON browser status report (runtime + active sessions) and exit
        #[arg(long)]
        status: bool,
        #[arg(long, hide = true)]
        worker: bool,
        #[arg(long, hide = true)]
        socket: Option<PathBuf>,
        #[arg(long, hide = true)]
        width: Option<i32>,
        #[arg(long, hide = true)]
        height: Option<i32>,
        #[arg(long, hide = true)]
        idle_timeout: Option<u64>,
        #[arg(long, hide = true)]
        parent_death_sigterm: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ChannelAction {
    Logs {
        #[arg(long, short, default_value = "0")]
        tail: usize,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum SkillsAction {
    List {
        #[arg(long)]
        profile: Option<String>,
    },
    #[command(alias = "view")]
    Inspect {
        name: String,
        #[arg(long)]
        profile: Option<String>,
    },
    #[command(alias = "rm")]
    Delete {
        name: String,
        #[arg(long)]
        profile: Option<String>,
    },
    History {
        name: String,
        #[arg(long)]
        profile: Option<String>,
    },
    Rollback {
        name: String,
        #[arg(long)]
        revision_id: Option<i64>,
        #[arg(long)]
        profile: Option<String>,
    },
    #[command(alias = "archived")]
    Archive,
    #[command(alias = "unarchive")]
    Restore {
        name: String,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum SopAction {
    List,
    Instances,
    Trigger {
        sop_id: String,
        payload: Option<String>,
        #[arg(long, short = 'w', default_value_t = false)]
        wait: bool,
    },
    Resume {
        #[arg(long, short)]
        instance_id: String,
        #[arg(long, short = 'w', default_value_t = false)]
        wait: bool,
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

    #[arg(long)]
    pub wait_curator: bool,
}

impl HeadlessArgs {
    pub fn merge_global(&mut self, global_output_format: Option<String>, global_yes: bool) {
        match global_yes {
            true => self.yes = true,
            false => {}
        }
        match global_output_format {
            Some(fmt) => self.output_format = fmt,
            None => {}
        }
    }
}
