use crate::channels::{
    Channel, DiscordChannel, EmailChannel, TelegramChannel, WhatsAppChannel, WsGateway,
};
use crate::cli::builder::build_agent_loop;
use crate::config::loader::load_config;
use crate::config::schema::Config;
use crate::cron::scheduler::start_scheduler;
use crate::{eprintln, println};
use anyhow::{anyhow, Result};

fn resolve_channel_secret(configured: String, env_var: &str) -> Result<String> {
    match configured.is_empty() {
        true => std::env::var(env_var)
            .map_err(|_| anyhow!("{env_var} environment variable or config parameter not set.")),
        false => Ok(configured),
    }
}

async fn run_channel_until_ctrl_c<C: Channel>(
    channel: C,
    channel_label: &str,
    config: &Config,
) -> ! {
    tokio::select! {
        biased;
        _ = tokio::signal::ctrl_c() => {
            println!("\r\nExiting {} channel...", channel_label);
        }
        res = channel.start() => {
            match res {
                Ok(()) => {}
                Err(e) => eprintln!("{} error: {}", channel_label, e),
            }
        }
    }

    crate::channels::shutdown_gateways_bounded(config).await;
    std::process::exit(0);
}

pub async fn handle_ratatui_tui() -> Result<()> {
    crate::channels::handle_ratatui_tui().await
}

pub async fn handle_gateway() -> Result<()> {
    let config = load_config()?;
    let ws_config = config.channels.websocket.clone().unwrap_or_default();

    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config).await?;
    let gateway = WsGateway::new(ws_config, agent_loop);

    tokio::select! {
        biased;
        _ = tokio::signal::ctrl_c() => {
            crate::println!("\r\nExiting Gateway server...");
        }
        res = gateway.start() => {
            match res {
                Ok(()) => {}
                Err(e) => crate::eprintln!("Gateway error: {}", e),
            }
        }
    }

    crate::shutdown::trigger();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    std::process::exit(0);
}

pub async fn handle_telegram() -> Result<()> {
    let config = load_config()?;
    let tg_config = config.channels.telegram.clone().unwrap_or_default();
    let token = resolve_channel_secret(tg_config.bot_token, "TELEGRAM_BOT_TOKEN")?;

    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = TelegramChannel::new(token, agent_loop);
    run_channel_until_ctrl_c(channel, "Telegram", &config).await
}

pub async fn handle_discord() -> Result<()> {
    let config = load_config()?;
    let dc_config = config.channels.discord.clone().unwrap_or_default();
    let token = resolve_channel_secret(dc_config.bot_token, "DISCORD_BOT_TOKEN")?;

    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = DiscordChannel::new(token, agent_loop);
    run_channel_until_ctrl_c(channel, "Discord", &config).await
}

pub async fn handle_whatsapp() -> Result<()> {
    let config = load_config()?;
    let wa_config = config.channels.whatsapp.clone().unwrap_or_default();
    let key = resolve_channel_secret(wa_config.api_key, "WHATSAPP_API_KEY")?;
    let phone_id = resolve_channel_secret(wa_config.phone_number_id, "WHATSAPP_PHONE_NUMBER_ID")?;

    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = WhatsAppChannel::new(key, phone_id, agent_loop);
    run_channel_until_ctrl_c(channel, "WhatsApp", &config).await
}

pub async fn handle_email() -> Result<()> {
    let config = load_config()?;
    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = EmailChannel::new(agent_loop);
    run_channel_until_ctrl_c(channel, "Email", &config).await
}

pub fn is_telegram_configured(config: &Config) -> bool {
    match config.channels.telegram {
        Some(ref tg) => !tg.bot_token.is_empty(),
        None => false,
    }
}

pub fn is_email_configured(config: &Config) -> bool {
    match config.channels.email {
        Some(ref em) => em.enabled && !em.imap_server.is_empty() && !em.username.is_empty(),
        None => false,
    }
}
