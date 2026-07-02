use anyhow::{Result, anyhow};
use crate::config::loader::load_config;
use crate::config::schema::Config;
use crate::cron::scheduler::start_scheduler;
use crate::cli::builder::build_agent_loop;
use crate::channels::{
    WsGateway, TelegramChannel, DiscordChannel, WhatsAppChannel, EmailChannel, Channel,
};

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

pub async fn handle_gateway() -> Result<()> {
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

pub async fn handle_telegram() -> Result<()> {
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

pub async fn handle_discord() -> Result<()> {
    let config = load_config()?;
    let dc_config = config.channels.discord.clone().unwrap_or_default();
    
    let token = if dc_config.bot_token.is_empty() {
        std::env::var("DISCORD_BOT_TOKEN").map_err(|_| anyhow!("DISCORD_BOT_TOKEN environment variable or config parameter not set."))?
    } else {
        dc_config.bot_token
    };
    
    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = DiscordChannel::new(token, agent_loop);
    
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

pub async fn handle_whatsapp() -> Result<()> {
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
    let channel = WhatsAppChannel::new(key, phone_id, agent_loop);
    
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

pub async fn handle_email() -> Result<()> {
    let config = load_config()?;
    start_scheduler(config.clone());
    let agent_loop = build_agent_loop(config.clone()).await?;
    let channel = EmailChannel::new(agent_loop);
    
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

pub fn is_telegram_configured(config: &Config) -> bool {
    if let Some(ref tg) = config.channels.telegram {
        !tg.bot_token.is_empty()
    } else {
        false
    }
}

pub fn is_email_configured(config: &Config) -> bool {
    if let Some(ref em) = config.channels.email {
        em.enabled && !em.imap_server.is_empty() && !em.username.is_empty()
    } else {
        false
    }
}
