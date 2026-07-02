pub mod args;
pub mod builder;
pub mod onboard;
pub mod configure;
pub mod agent;
pub mod channels;
pub mod sop;
pub mod logs;
pub mod streaming;
pub mod changelog;

use anyhow::Result;
pub use args::{CliArgs, Command, ChannelAction, SopAction};
pub use builder::build_agent_loop;
pub use agent::{load_session_history, archive_current_session};

static IS_SILENT_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_silent_mode() -> bool {
    IS_SILENT_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_silent_mode(val: bool) {
    IS_SILENT_MODE.store(val, std::sync::atomic::Ordering::Relaxed);
}

pub async fn run_cli() -> Result<()> {
    // Custom version logo interception
    for arg in std::env::args() {
        if arg == "--version" || arg == "-V" {
            let logo = format!(
                "openz v{}\n",
                env!("CARGO_PKG_VERSION")
            );
            print!("{}", logo);
            std::process::exit(0);
        }
    }
    Ok(())
}
