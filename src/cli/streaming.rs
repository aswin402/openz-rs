use anyhow::Result;
use inquire::Select;

use crate::agent::style::*;
use crate::config::loader::{load_config, save_config};
use crate::println;

pub async fn handle_streaming() -> Result<()> {
    let mut config = load_config()?;
    let current_status = match config.agents.defaults.streaming {
        true => "Enabled",
        false => "Disabled",
    };

    println!(
        "{}◇ OpenZ Response Streaming Wizard{}",
        COLOR_BOLD, COLOR_RESET
    );
    println!(
        "Current status: {}{}{}\r\n",
        RED_ORANGE, current_status, COLOR_RESET
    );
    println!("Streaming prints response chunks in real-time. However, keeping it disabled");
    println!("is highly recommended for unstable or rate-limited API gateways (like OpenCode Zen)");
    println!("to avoid early cut-offs or connection drops.\r\n");

    let options = vec![
        "Enable streaming (globally)".to_string(),
        "Disable streaming (globally)".to_string(),
        "Exit".to_string(),
    ];

    let choice = Select::new("Choose option:", options).prompt()?;

    match (
        choice.starts_with("Enable"),
        choice.starts_with("Disable"),
    ) {
        (true, _) => {
            config.agents.defaults.streaming = true;
            save_config(&config)?;
            println!(
                "{}✓ Response streaming has been ENABLED globally for OpenZ.{}",
                EMERALD_GREEN, COLOR_RESET
            );
        }
        (false, true) => {
            config.agents.defaults.streaming = false;
            save_config(&config)?;
            println!(
                "{}✓ Response streaming has been DISABLED globally for OpenZ.{}",
                EMERALD_GREEN, COLOR_RESET
            );
        }
        (false, false) => {
            println!("No changes made.");
        }
    }

    Ok(())
}
