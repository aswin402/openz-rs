pub mod agent;
pub mod args;
pub mod builder;
pub mod changelog;
pub mod channels;
pub mod configure;
pub mod doctor;
pub mod logs;
pub mod onboard;
pub mod headless;
pub mod sop;
pub mod streaming;
pub mod tools;
pub(crate) mod tool_registration;

#[cfg(test)]
mod headless_tests;

use crate::print;
pub use agent::{archive_current_session, load_session_history};
use anyhow::Result;
pub use args::{ChannelAction, CliArgs, Command, HeadlessArgs, SopAction};
pub use builder::build_agent_loop;
pub use channels::{
    handle_discord, handle_email, handle_gateway, handle_ratatui_tui, handle_telegram,
    handle_whatsapp, is_email_configured, is_telegram_configured,
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

async fn run_headless_with_prompt(
    prompt: Option<String>,
    output_format: Option<String>,
    yes: bool,
) -> Result<()> {
    let headless_args = args::HeadlessArgs {
        prompt,
        output_format: output_format.unwrap_or_else(|| "text".to_string()),
        yes,
        ..Default::default()
    };
    headless::handle_headless(headless_args).await
}

async fn run_ratatui_and_exit() -> Result<()> {
    channels::handle_ratatui_tui().await?;
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
    std::process::exit(0);
}

async fn dispatch_channel_action<F>(
    action: Option<ChannelAction>,
    channel_name: &str,
    start_fut: F,
) -> Result<()>
where
    F: std::future::Future<Output = Result<()>>,
{
    match action {
        Some(ChannelAction::Logs { tail }) => {
            logs::handle_logs(None, tail, Some(channel_name.to_string()), None, false, None).await
        }
        None => start_fut.await,
    }
}

pub async fn run_cli() -> Result<()> {
    // Intercept top-level version flags only before `--` and outside passthrough subcommands
    let args_before_sep: Vec<String> = std::env::args()
        .skip(1)
        .take_while(|arg| arg != "--")
        .collect();
    let first_subcommand = args_before_sep
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(String::as_str);
    let should_print_version = (!matches!(first_subcommand, Some("mcp-bridge" | "run")))
        && args_before_sep
            .iter()
            .any(|arg| matches!(arg.as_str(), "--version" | "-V"));

    match should_print_version {
        true => {
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
        false => {}
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
        Some(Command::Run(mut headless_args)) => {
            match (
                headless_args.prompt.is_none(),
                headless_args.prompt_flag.is_none(),
            ) {
                (true, true) => headless_args.prompt = args.prompt,
                _ => {}
            }
            headless_args.merge_global(args.output_format, args.yes);
            return headless::handle_headless(headless_args).await;
        }
        None => {
            use crossterm::tty::IsTty;
            match (args.prompt, std::io::stdin().is_tty()) {
                (Some(prompt), _) => {
                    return run_headless_with_prompt(Some(prompt), args.output_format, args.yes)
                        .await;
                }
                (None, false) => {
                    return run_headless_with_prompt(None, args.output_format, args.yes).await;
                }
                (None, true) => {
                    run_ratatui_and_exit().await?;
                }
            }
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
            run_ratatui_and_exit().await?;
        }
        Some(Command::Gateway { action }) => {
            dispatch_channel_action(action, "gateway", channels::handle_gateway()).await?;
        }
        Some(Command::Telegram { action }) => {
            dispatch_channel_action(action, "telegram", channels::handle_telegram()).await?;
        }
        Some(Command::Discord { action }) => {
            dispatch_channel_action(action, "discord", channels::handle_discord()).await?;
        }
        Some(Command::Whatsapp { action }) => {
            dispatch_channel_action(action, "whatsapp", channels::handle_whatsapp()).await?;
        }
        Some(Command::Email { action }) => {
            dispatch_channel_action(action, "email", channels::handle_email()).await?;
        }
        Some(Command::Subagent) => {
            let config = crate::config::loader::load_config()?;
            crate::subagents::run_subagent_manager(config).await?;
        }
        Some(Command::Doctor {
            scrub_secrets,
            clean_target,
        }) => {
            doctor::handle_doctor(scrub_secrets, clean_target).await?;
        }
        Some(Command::McpBridge { port, command_args }) => match command_args.as_slice() {
            [] => {
                return Err(anyhow::anyhow!(
                    "No target command specified. Usage: openz mcp-bridge --port <port> -- <command> [args...]"
                ));
            }
            [command, args @ ..] => {
                let (_tx, rx) = tokio::sync::oneshot::channel();
                let port_guard = std::net::TcpListener::bind(format!("127.0.0.1:{}", port))
                    .map_err(|e| anyhow::anyhow!("Cannot bind to port {}: {}", port, e))?;
                crate::tools::mcp::run_mcp_bridge(port, port_guard, command, args, rx).await?;
            }
        },
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
        for &name in crate::config::loader::RUNTIME_SQLITE_DB_FILENAMES {
            let path = crate::config::loader::runtime_db_path(name);
            match (path.exists(), rusqlite::Connection::open(&path)) {
                (true, Ok(conn)) => {
                    let _ = conn.execute_batch("PRAGMA incremental_vacuum(50); PRAGMA optimize;");
                }
                _ => {}
            }
        }
    });
}
