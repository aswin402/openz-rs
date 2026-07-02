use anyhow::{Result, anyhow};
use inquire::{Text, Password, Confirm, PasswordDisplayMode};
use crate::config::loader::{load_config, save_config};
use crate::config::schema::{Config, ProviderConfig};
use crate::agent::style::*;

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

#[allow(unused_macros)]
macro_rules! eprint {
    () => {
        crate::tui_print!()
    };
    ($($arg:tt)*) => {
        crate::tui_print!($($arg)*)
    };
}

pub fn update_provider_key(config: &mut Config, provider_name: &str, api_key: String) {
    let mut p_config = match provider_name {
        "anthropic" => config.providers.anthropic.clone(),
        "openai" => config.providers.openai.clone(),
        "openrouter" => config.providers.openrouter.clone(),
        "deepseek" => config.providers.deepseek.clone(),
        "groq" => config.providers.groq.clone(),
        "ollama" => config.providers.ollama.clone(),
        "minimax" => config.providers.minimax.clone(),
        "mistral" => config.providers.mistral.clone(),
        "z.ai" => config.providers.z_ai.clone(),
        "nvidia" => config.providers.nvidia.clone(),
        "opencode_zen" => config.providers.opencode_zen.clone(),
        "cerebras" => config.providers.cerebras.clone(),
        "google_ai_studio" => config.providers.google_ai_studio.clone(),
        "cohere" => config.providers.cohere.clone(),
        "llm7" => config.providers.llm7.clone(),
        "sambanova" => config.providers.sambanova.clone(),
        "huggingface" => config.providers.huggingface.clone(),
        _ => return,
    }.unwrap_or_else(|| ProviderConfig {
        api_key: None,
        api_base: None,
        extra: std::collections::HashMap::new(),
    });
    p_config.api_key = Some(api_key);
    
    if p_config.api_base.is_none() {
        let default_base = match provider_name {
            "anthropic" => "https://api.anthropic.com",
            "openai" => "https://api.openai.com/v1",
            "openrouter" => "https://openrouter.ai/api/v1",
            "deepseek" => "https://api.deepseek.com/v1",
            "groq" => "https://api.groq.com/openai/v1",
            "ollama" => "http://localhost:11434/v1",
            "minimax" => "https://api.minimax.io/v1",
            "mistral" => "https://api.mistral.ai/v1",
            "z.ai" => "https://api.z.ai/api/paas/v4/",
            "nvidia" => "https://integrate.api.nvidia.com/v1",
            "opencode_zen" => "https://opencode.ai/zen/v1",
            "cerebras" => "https://api.cerebras.ai/v1",
            "google_ai_studio" => "https://generativelanguage.googleapis.com/v1beta/openai/",
            "cohere" => "https://api.cohere.com/v1",
            "llm7" => "https://token.llm7.io/v1",
            "sambanova" => "https://api.sambanova.ai/v1",
            "huggingface" => "https://api-inference.huggingface.co/v1",
            _ => "",
        };
        p_config.api_base = Some(default_base.to_string());
    }

    match provider_name {
        "anthropic" => config.providers.anthropic = Some(p_config),
        "openai" => config.providers.openai = Some(p_config),
        "openrouter" => config.providers.openrouter = Some(p_config),
        "deepseek" => config.providers.deepseek = Some(p_config),
        "groq" => config.providers.groq = Some(p_config),
        "ollama" => config.providers.ollama = Some(p_config),
        "minimax" => config.providers.minimax = Some(p_config),
        "mistral" => config.providers.mistral = Some(p_config),
        "z.ai" => config.providers.z_ai = Some(p_config),
        "nvidia" => config.providers.nvidia = Some(p_config),
        "opencode_zen" => config.providers.opencode_zen = Some(p_config),
        "cerebras" => config.providers.cerebras = Some(p_config),
        "google_ai_studio" => config.providers.google_ai_studio = Some(p_config),
        "cohere" => config.providers.cohere = Some(p_config),
        "llm7" => config.providers.llm7 = Some(p_config),
        "sambanova" => config.providers.sambanova = Some(p_config),
        "huggingface" => config.providers.huggingface = Some(p_config),
        _ => {}
    }
}

fn is_telegram_configured(config: &Config) -> bool {
    if let Some(ref tg) = config.channels.telegram {
        !tg.bot_token.is_empty()
    } else {
        false
    }
}

fn is_email_configured(config: &Config) -> bool {
    if let Some(ref em) = config.channels.email {
        em.enabled && !em.imap_server.is_empty() && !em.username.is_empty()
    } else {
        false
    }
}

pub async fn handle_configure() -> Result<()> {
    let active_mdl = {
        let config = load_config()?;
        config.agents.defaults.model.clone()
    };

    loop {
        let mut config = load_config()?;
        
        let mut configure_options = vec![
            "Providers".to_string(),
        ];

        if let Some(ref ws) = config.channels.websocket {
            if ws.enabled {
                configure_options.push("Gateway (WebSocket) (enabled)".to_string());
            } else {
                configure_options.push("Gateway (WebSocket)".to_string());
            }
        } else {
            configure_options.push("Gateway (WebSocket)".to_string());
        }
        
        if is_telegram_configured(&config) {
            configure_options.push("Telegram (configured)".to_string());
        } else {
            configure_options.push("Telegram".to_string());
        }

        if let Some(ref dc) = config.channels.discord {
            if dc.enabled && !dc.bot_token.trim().is_empty() {
                configure_options.push("Discord (configured)".to_string());
            } else {
                configure_options.push("Discord".to_string());
            }
        } else {
            configure_options.push("Discord".to_string());
        }

        if let Some(ref wa) = config.channels.whatsapp {
            if wa.enabled && !wa.api_key.trim().is_empty() {
                configure_options.push("WhatsApp (configured)".to_string());
            } else {
                configure_options.push("WhatsApp".to_string());
            }
        } else {
            configure_options.push("WhatsApp".to_string());
        }

        if is_email_configured(&config) {
            configure_options.push("Email (configured)".to_string());
        } else {
            configure_options.push("Email".to_string());
        }

        if config.agents.defaults.enable_sandbox {
            configure_options.push("Sandbox (seccomp) (enabled)".to_string());
        } else {
            configure_options.push("Sandbox (seccomp) (disabled)".to_string());
        }

        configure_options.push("Exit".to_string());

        let choice_idx = match select_menu_custom(
            "Choose configure category:",
            &configure_options,
            &active_mdl,
            Some("OpenZ Configuration"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break, // Exit on Esc
        };

        match choice_idx {
            0 => {
                handle_providers_submenu(&mut config, &active_mdl).await?;
            }
            1 => {
                handle_gateway_submenu(&mut config).await?;
            }
            2 => {
                if is_telegram_configured(&config) {
                    let reconfigure = Confirm::new("Telegram is already configured. Reconfigure?")
                        .with_default(false)
                        .prompt()?;
                    if !reconfigure {
                        continue;
                    }
                }
                handle_telegram_submenu(&mut config).await?;
            }
            3 => {
                handle_discord_submenu(&mut config).await?;
            }
            4 => {
                handle_whatsapp_submenu(&mut config).await?;
            }
            5 => {
                handle_email_submenu(&mut config).await?;
            }
            6 => {
                handle_sandbox_submenu(&mut config).await?;
            }
            _ => {
                break;
            }
        }
        
        save_config(&config)?;
    }
    
    println!("{}✓ Configuration saved successfully!{}", EMERALD_GREEN, COLOR_RESET);
    Ok(())
}

async fn handle_providers_submenu(config: &mut Config, active_mdl: &str) -> Result<()> {
    struct ProviderInfo {
        name: &'static str,
        display: &'static str,
    }
    let provider_list = vec![
        ProviderInfo { name: "anthropic", display: "Anthropic (Claude)" },
        ProviderInfo { name: "openai", display: "OpenAI" },
        ProviderInfo { name: "openrouter", display: "OpenRouter" },
        ProviderInfo { name: "deepseek", display: "DeepSeek" },
        ProviderInfo { name: "groq", display: "Groq" },
        ProviderInfo { name: "ollama", display: "Ollama" },
        ProviderInfo { name: "minimax", display: "MiniMax" },
        ProviderInfo { name: "mistral", display: "Mistral AI" },
        ProviderInfo { name: "z.ai", display: "z.ai (Zhipu GLM)" },
        ProviderInfo { name: "nvidia", display: "NVIDIA NIM" },
        ProviderInfo { name: "opencode_zen", display: "OpenCode Zen" },
        ProviderInfo { name: "cerebras", display: "Cerebras" },
        ProviderInfo { name: "google_ai_studio", display: "Google AI Studio (Gemini)" },
        ProviderInfo { name: "cohere", display: "Cohere" },
        ProviderInfo { name: "llm7", display: "LLM7 (token.llm7.io)" },
        ProviderInfo { name: "sambanova", display: "SambaNova" },
        ProviderInfo { name: "huggingface", display: "Hugging Face Inference" },
    ];

    loop {
        let mut prov_options: Vec<String> = provider_list.iter().map(|p| {
            if config.is_provider_configured(p.name) {
                format!("{} (configured)", p.display)
            } else {
                p.display.to_string()
            }
        }).collect();
        prov_options.push("Back".to_string());

        let choice_idx = match select_menu_custom(
            "Select provider to configure:",
            &prov_options,
            active_mdl,
            Some("Providers Configuration"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break, // Go back on Esc
        };

        if choice_idx == provider_list.len() {
            break; // Back option
        }

        let prov_info = &provider_list[choice_idx];
        
        if config.is_provider_configured(prov_info.name) {
            let reconfigure = Confirm::new(&format!("{} is already configured. Reconfigure?", prov_info.display))
                .with_default(false)
                .prompt()?;
            if !reconfigure {
                continue;
            }
        }

        let key = Password::new(&format!("Enter API Key for {}:", prov_info.display))
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        if !key.trim().is_empty() {
            update_provider_key(config, prov_info.name, key.trim().to_string());
            save_config(config)?;
            println!("{}✓ API Key updated for {}!{}", EMERALD_GREEN, prov_info.display, COLOR_RESET);
        } else {
            println!("{}⚠️ API Key unchanged.{}", AURA_GOLD, COLOR_RESET);
        }
    }
    Ok(())
}

async fn handle_telegram_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Telegram Bot Setup Guide ---{}", COLOR_BOLD, COLOR_RESET);
    println!("  1. Open Telegram and search for {}@BotFather{}.", HEADING_BLUE, COLOR_RESET);
    println!("  2. Start a chat and send the command {}/newbot{}.", EMERALD_GREEN, COLOR_RESET);
    println!("  3. Choose a name and a username for your bot.");
    println!("  4. Copy the HTTP API token provided by BotFather.\n");

    let token = Password::new("Paste Telegram Bot Token: ")
        .without_confirmation()
        .with_display_mode(PasswordDisplayMode::Masked)
        .prompt()?;
    if !token.trim().is_empty() {
        let mut tg = config.channels.telegram.clone().unwrap_or_default();
        tg.enabled = true;
        tg.bot_token = token.trim().to_string();
        config.channels.telegram = Some(tg);
        save_config(config)?;
        println!("{}✓ Telegram bot configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    } else {
        println!("{}⚠️ Token unchanged.{}", AURA_GOLD, COLOR_RESET);
    }
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}

fn setup_systemd_service() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not locate home directory"))?;
    let service_dir = home.join(".config").join("systemd").join("user");
    std::fs::create_dir_all(&service_dir)?;
    
    let exe_path = std::env::current_exe().unwrap_or_else(|_| home.join(".cargo").join("bin").join("openz"));
    
    let service_content = format!(
r#"[Unit]
Description=OpenZ WebSocket Gateway Daemon
After=network.target

[Service]
ExecStart={} gateway
Restart=on-failure

[Install]
WantedBy=default.target
"#,
        exe_path.to_string_lossy()
    );
    
    let service_file = service_dir.join("openz-gateway.service");
    std::fs::write(&service_file, service_content)?;
    
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
        
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "enable", "openz-gateway.service"])
        .output();
        
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "restart", "openz-gateway.service"])
        .output();
        
    Ok(())
}

fn disable_systemd_service() -> Result<()> {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "disable", "openz-gateway.service"])
        .output();
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "stop", "openz-gateway.service"])
        .output();
    Ok(())
}

async fn handle_gateway_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Gateway (WebSocket) Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    
    let mut ws = config.channels.websocket.clone().unwrap_or_else(|| {
        crate::config::schema::WebSocketChannelConfig {
            enabled: true,
            port: 8765,
            host: "127.0.0.1".to_string(),
            start_on_boot: false,
            start_on_tui: false,
        }
    });

    let enabled = Confirm::new("Enable WebSocket gateway?")
        .with_default(ws.enabled)
        .prompt()?;
    ws.enabled = enabled;

    if enabled {
        let port_input = Text::new(&format!("WebSocket Port [default: {}]:", ws.port)).prompt()?;
        if !port_input.trim().is_empty() {
            if let Ok(p) = port_input.trim().parse::<u16>() {
                ws.port = p;
            }
        }

        let host_input = Text::new(&format!("WebSocket Host [default: {}]:", ws.host)).prompt()?;
        if !host_input.trim().is_empty() {
            ws.host = host_input.trim().to_string();
        }

        let auto_start_options = vec![
            "None (Manual launch only)".to_string(),
            "Auto-start on TUI launch (TUI background thread)".to_string(),
            "System Boot Daemon (Installs systemd user service)".to_string(),
        ];

        let auto_choice = select_menu_custom(
            "Choose Gateway Auto-Start Preference:",
            &auto_start_options,
            "None (Manual launch only)",
            None,
            false,
        )?;

        match auto_choice {
            Some(2) => {
                ws.start_on_boot = true;
                ws.start_on_tui = false;
                println!("Installing and enabling systemd user service...");
                if let Err(e) = setup_systemd_service() {
                    eprintln!("{}✕ Failed to setup systemd service: {}{}", ERROR_RED, e, COLOR_RESET);
                } else {
                    println!("{}✓ systemd service openz-gateway.service installed and enabled successfully!{}", EMERALD_GREEN, COLOR_RESET);
                }
            }
            Some(1) => {
                ws.start_on_boot = false;
                ws.start_on_tui = true;
                let _ = disable_systemd_service();
                println!("Gateway configured to auto-start when terminal client launches.");
            }
            _ => {
                ws.start_on_boot = false;
                ws.start_on_tui = false;
                let _ = disable_systemd_service();
                println!("Gateway auto-start disabled (manual launch only).");
            }
        }
    } else {
        let _ = disable_systemd_service();
    }

    config.channels.websocket = Some(ws);
    save_config(config)?;
    println!("{}✓ Gateway configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}

async fn handle_discord_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Discord Bot Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    
    let mut dc = config.channels.discord.clone().unwrap_or_default();
    
    let enabled = Confirm::new("Enable Discord Bot channel?")
        .with_default(dc.enabled)
        .prompt()?;
    dc.enabled = enabled;

    if enabled {
        let token = Password::new("Paste Discord Bot Token: ")
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        if !token.trim().is_empty() {
            dc.bot_token = token.trim().to_string();
        }
    }

    config.channels.discord = Some(dc);
    save_config(config)?;
    println!("{}✓ Discord channel configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}

async fn handle_whatsapp_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- WhatsApp Channel Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    
    let mut wa = config.channels.whatsapp.clone().unwrap_or_default();
    
    let enabled = Confirm::new("Enable WhatsApp channel?")
        .with_default(wa.enabled)
        .prompt()?;
    wa.enabled = enabled;

    if enabled {
        let key = Password::new("Paste WhatsApp API Key: ")
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        if !key.trim().is_empty() {
            wa.api_key = key.trim().to_string();
        }

        let phone_id = Text::new("Enter WhatsApp Phone Number ID: ").prompt()?;
        if !phone_id.trim().is_empty() {
            wa.phone_number_id = phone_id.trim().to_string();
        }

        let port_str = Text::new("Enter WhatsApp Webhook Port: ")
            .with_default(&wa.webhook_port.to_string())
            .prompt()?;
        if let Ok(p) = port_str.trim().parse::<u16>() {
            wa.webhook_port = p;
        }

        let token = Text::new("Enter WhatsApp Webhook Verification Token: ")
            .with_default(&wa.verify_token)
            .prompt()?;
        if !token.trim().is_empty() {
            wa.verify_token = token.trim().to_string();
        }
    }

    config.channels.whatsapp = Some(wa);
    save_config(config)?;
    println!("{}✓ WhatsApp channel configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}

async fn handle_email_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Email Channel Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    
    let mut em = config.channels.email.clone().unwrap_or_default();
    
    let enabled = Confirm::new("Enable Email channel?")
        .with_default(em.enabled)
        .prompt()?;
    em.enabled = enabled;

    if enabled {
        let imap_server = Text::new("Enter IMAP Server (e.g. imap.gmail.com): ")
            .with_default(&em.imap_server)
            .prompt()?;
        if !imap_server.trim().is_empty() {
            em.imap_server = imap_server.trim().to_string();
        }

        let imap_port_str = Text::new("Enter IMAP Port: ")
            .with_default(&em.imap_port.to_string())
            .prompt()?;
        if let Ok(p) = imap_port_str.trim().parse::<u16>() {
            em.imap_port = p;
        }

        let smtp_server = Text::new("Enter SMTP Server (e.g. smtp.gmail.com): ")
            .with_default(&em.smtp_server)
            .prompt()?;
        if !smtp_server.trim().is_empty() {
            em.smtp_server = smtp_server.trim().to_string();
        }

        let smtp_port_str = Text::new("Enter SMTP Port: ")
            .with_default(&em.smtp_port.to_string())
            .prompt()?;
        if let Ok(p) = smtp_port_str.trim().parse::<u16>() {
            em.smtp_port = p;
        }

        let username = Text::new("Enter Username / Email: ")
            .with_default(&em.username)
            .prompt()?;
        if !username.trim().is_empty() {
            em.username = username.trim().to_string();
        }

        let password = Password::new("Enter Password / App Password: ")
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        if !password.trim().is_empty() {
            em.password = password.trim().to_string();
        }

        let poll_str = Text::new("Enter Email Poll Interval (seconds): ")
            .with_default(&em.poll_interval_secs.to_string())
            .prompt()?;
        if let Ok(s) = poll_str.trim().parse::<u64>() {
            em.poll_interval_secs = s;
        }
    }

    config.channels.email = Some(em);
    save_config(config)?;
    println!("{}✓ Email channel configured successfully!{}", EMERALD_GREEN, COLOR_RESET);
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    Ok(())
}

async fn handle_sandbox_submenu(config: &mut Config) -> Result<()> {
    println!("\n{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    println!("{}--- Sandbox (seccomp) Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    println!("The process sandbox (seccomp) restricts system calls (network, browser, tools like ps/which) inside the command execution sandbox.");
    println!("Disabling it allows browser automation tools (gsd_browser, chromewright) and local compiler tools to run without seccomp blocking.");
    println!("Security is still enforced via openz's internal SecurityGuard prompt confirmations.");
    println!();
    
    let current = config.agents.defaults.enable_sandbox;
    let enable = Confirm::new("Enable process seccomp sandbox?")
        .with_default(current)
        .prompt()?;
        
    config.agents.defaults.enable_sandbox = enable;
    save_config(config)?;
    
    if enable {
        println!("{}✓ Sandbox enabled successfully!{}", EMERALD_GREEN, COLOR_RESET);
    } else {
        println!("{}✓ Sandbox disabled successfully! (Highly recommended for browser tools and developer shells){}", EMERALD_GREEN, COLOR_RESET);
    }
    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
    
    // Give the user a moment to see the success message
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    
    Ok(())
}
