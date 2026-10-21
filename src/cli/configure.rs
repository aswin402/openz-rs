use crate::agent::style::*;
use crate::channels::provider_model_catalog;
use crate::config::loader::{load_config, save_config};
use crate::config::schema::{Config, ProviderConfig};
use crate::println;
use anyhow::{anyhow, Result};
use inquire::{Confirm, Password, PasswordDisplayMode, Text};

pub(crate) fn non_empty_trimmed_or(input: &str, default: &str) -> String {
    let trimmed = input.trim();
    match trimmed.is_empty() {
        true => default.to_string(),
        false => trimmed.to_string(),
    }
}

fn apply_non_empty_input(target: &mut String, input: &str) {
    let trimmed = input.trim();
    match trimmed.is_empty() {
        true => {}
        false => *target = trimmed.to_string(),
    }
}

fn apply_parsed_input<T: std::str::FromStr>(target: &mut T, input: &str) {
    match input.trim().parse::<T>() {
        Ok(parsed) => *target = parsed,
        Err(_) => {}
    }
}

fn status_label(base: &str, active: bool, suffix: &str) -> String {
    match active {
        true => format!("{base} ({suffix})"),
        false => base.to_string(),
    }
}

pub fn update_provider_key(config: &mut Config, provider_name: &str, api_key: String) {
    let mut p_config = config
        .get_provider_config(provider_name)
        .cloned()
        .unwrap_or_else(|| ProviderConfig {
            api_key: None,
            api_key_env: None,
            api_key_file: None,
            api_base: None,
            default_model: None,
            extra: std::collections::HashMap::new(),
        });
    p_config.api_key = Some(api_key);

    match p_config.api_base.is_none() {
        true => {
            p_config.api_base = Some(
                crate::config::provider_catalog::default_api_base_for_provider(provider_name)
                    .to_string(),
            );
        }
        false => {}
    }

    config.set_provider_config(provider_name, p_config);
}

fn is_local_base(api_base: &str) -> bool {
    let base = api_base.trim().to_lowercase();
    base.starts_with("http://localhost")
        || base.starts_with("http://127.0.0.1")
        || base.starts_with("http://0.0.0.0")
        || base.starts_with("http://[::1]")
}

async fn handle_custom_provider_form(
    config: &mut Config,
    existing_name: Option<String>,
) -> Result<()> {
    let provider_name = match existing_name {
        Some(name) => name,
        None => {
            let input = Text::new("Custom provider name (letters, numbers, _, -, .):").prompt()?;
            let name = input.trim().to_string();
            match Config::is_custom_provider_name_valid(&name) {
                true => name,
                false => {
                    return Err(anyhow!(
                        "invalid custom provider name `{}`; use a unique name with letters, numbers, _, -, or .",
                        name
                    ));
                }
            }
        }
    };

    let existing = config.get_provider_config(&provider_name).cloned();
    match existing.is_some() {
        true => {
            let reconfigure = Confirm::new(&format!(
                "Custom provider `{}` already exists. Reconfigure?",
                provider_name
            ))
            .with_default(false)
            .prompt()?;
            match reconfigure {
                true => {}
                false => return Ok(()),
            }
        }
        false => {}
    }

    let existing_base = existing
        .as_ref()
        .and_then(|provider| provider.api_base.as_deref())
        .unwrap_or("https://api.openai.com/v1");
    let base_input = Text::new(&format!(
        "OpenAI-compatible API base URL [default: {}]:",
        existing_base
    ))
    .prompt()?;
    let api_base = non_empty_trimmed_or(&base_input, existing_base);

    let key = Password::new(&format!(
        "API key for `{}` (leave empty only for local/no-key providers):",
        provider_name
    ))
    .without_confirmation()
    .with_display_mode(PasswordDisplayMode::Masked)
    .prompt()?;
    let api_key = match key.trim().is_empty() {
        true => existing
            .as_ref()
            .and_then(|provider| provider.api_key.clone()),
        false => Some(key.trim().to_string()),
    };

    match api_key.as_deref().unwrap_or_default().trim().is_empty() && !is_local_base(&api_base) {
        true => {
            println!(
                "{}⚠️ No API key set. You can also set {}.{}",
                AURA_GOLD,
                Config::custom_provider_env_var(&provider_name),
                COLOR_RESET
            );
        }
        false => {}
    }

    let existing_model = existing
        .as_ref()
        .and_then(|provider| provider.default_model.as_deref())
        .unwrap_or("gpt-4o");
    let model_input = Text::new(&format!(
        "Default model for `{}` [default: {}]:",
        provider_name, existing_model
    ))
    .prompt()?;
    let default_model = non_empty_trimmed_or(&model_input, existing_model);

    config.set_provider_config(
        &provider_name,
        ProviderConfig {
            api_key,
            api_key_env: None,
            api_key_file: None,
            api_base: Some(api_base),
            default_model: Some(default_model.clone()),
            extra: existing.map(|provider| provider.extra).unwrap_or_default(),
        },
    );

    let use_now = Confirm::new("Use this provider/model as the active default now?")
        .with_default(true)
        .prompt()?;
    match use_now {
        true => {
            config.agents.defaults.provider = provider_name.clone();
            config.agents.defaults.model = default_model.clone();
        }
        false => {}
    }

    save_config(config)?;
    println!(
        "{}✓ Custom provider `{}` saved with default model `{}`.{}",
        EMERALD_GREEN, provider_name, default_model, COLOR_RESET
    );
    Ok(())
}

use super::channels::{is_email_configured, is_telegram_configured};

fn is_discord_configured(config: &Config) -> bool {
    config
        .channels
        .discord
        .as_ref()
        .is_some_and(|dc| dc.enabled && !dc.bot_token.trim().is_empty())
}

fn is_whatsapp_configured(config: &Config) -> bool {
    config
        .channels
        .whatsapp
        .as_ref()
        .is_some_and(|wa| wa.enabled && !wa.api_key.trim().is_empty())
}

fn is_gateway_enabled(config: &Config) -> bool {
    config
        .channels
        .websocket
        .as_ref()
        .is_some_and(|ws| ws.enabled)
}

pub async fn handle_configure() -> Result<()> {
    let active_mdl = {
        let config = load_config()?;
        config.agents.defaults.model.clone()
    };

    loop {
        let mut config = load_config()?;

        let sandbox_status = match config.agents.defaults.enable_sandbox {
            true => "enabled",
            false => "disabled",
        };

        let configure_options = vec![
            "Providers".to_string(),
            status_label("Gateway (WebSocket)", is_gateway_enabled(&config), "enabled"),
            status_label("Telegram", is_telegram_configured(&config), "configured"),
            status_label("Discord", is_discord_configured(&config), "configured"),
            status_label("WhatsApp", is_whatsapp_configured(&config), "configured"),
            status_label("Email", is_email_configured(&config), "configured"),
            format!(
                "TUI (thoughts: {})",
                config.agents.defaults.tui_thought_display
            ),
            format!("Sandbox (seccomp) ({sandbox_status})"),
            "Browser (Firefox)".to_string(),
            "Exit".to_string(),
        ];

        let choice_idx = match select_menu_custom(
            "Choose configure category:",
            &configure_options,
            &active_mdl,
            Some("OpenZ Configuration"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break,
        };

        match choice_idx {
            0 => handle_providers_submenu(&mut config, &active_mdl).await?,
            1 => handle_gateway_submenu(&mut config).await?,
            2 => {
                let should_proceed = match is_telegram_configured(&config) {
                    true => Confirm::new("Telegram is already configured. Reconfigure?")
                        .with_default(false)
                        .prompt()?,
                    false => true,
                };
                match should_proceed {
                    true => handle_telegram_submenu(&mut config).await?,
                    false => continue,
                }
            }
            3 => handle_discord_submenu(&mut config).await?,
            4 => handle_whatsapp_submenu(&mut config).await?,
            5 => handle_email_submenu(&mut config).await?,
            6 => handle_tui_submenu(&mut config).await?,
            7 => handle_sandbox_submenu(&mut config).await?,
            8 => handle_browser_submenu(&mut config).await?,
            _ => break,
        }

        save_config(&config)?;
    }

    println!(
        "{}✓ Configuration saved successfully!{}",
        EMERALD_GREEN, COLOR_RESET
    );
    Ok(())
}

async fn handle_providers_submenu(config: &mut Config, active_mdl: &str) -> Result<()> {
    let provider_list = provider_model_catalog();

    loop {
        let custom_names = config.custom_provider_names();
        let mut prov_options: Vec<String> = provider_list
            .iter()
            .map(|p| status_label(p.display, config.is_provider_configured(p.name), "configured"))
            .collect();
        for name in &custom_names {
            let model = config
                .custom_provider_default_model(name)
                .unwrap_or_else(|| "custom model".to_string());
            prov_options.push(format!("Custom: {} ({})", name, model));
        }
        prov_options.push("Add Custom Provider".to_string());
        prov_options.push("Back".to_string());

        let choice_idx = match select_menu_custom(
            "Select provider to configure:",
            &prov_options,
            active_mdl,
            Some("Providers Configuration"),
            true,
        ) {
            Ok(Some(idx)) => idx,
            _ => break,
        };

        let builtin_len = provider_list.len();
        let add_custom_idx = builtin_len + custom_names.len();
        let back_idx = add_custom_idx + 1;

        match (
            choice_idx == back_idx,
            choice_idx == add_custom_idx,
            choice_idx >= builtin_len,
        ) {
            (true, _, _) => break,
            (false, true, _) => {
                handle_custom_provider_form(config, None).await?;
                continue;
            }
            (false, false, true) => {
                let custom_idx = choice_idx - builtin_len;
                handle_custom_provider_form(config, Some(custom_names[custom_idx].clone())).await?;
                continue;
            }
            (false, false, false) => {}
        }

        let prov_info = &provider_list[choice_idx];

        let should_configure = match config.is_provider_configured(prov_info.name) {
            true => Confirm::new(&format!(
                "{} is already configured. Reconfigure?",
                prov_info.display
            ))
            .with_default(false)
            .prompt()?,
            false => true,
        };
        match should_configure {
            true => {}
            false => continue,
        }

        let key = Password::new(&format!("Enter API Key for {}:", prov_info.display))
            .without_confirmation()
            .with_display_mode(PasswordDisplayMode::Masked)
            .prompt()?;
        let trimmed_key = key.trim();
        match trimmed_key.is_empty() {
            true => println!("{}⚠️ API Key unchanged.{}", AURA_GOLD, COLOR_RESET),
            false => {
                update_provider_key(config, prov_info.name, trimmed_key.to_string());
                save_config(config)?;
                println!(
                    "{}✓ API Key updated for {}!{}",
                    EMERALD_GREEN, prov_info.display, COLOR_RESET
                );
            }
        }
    }
    Ok(())
}

async fn handle_telegram_submenu(config: &mut Config) -> Result<()> {
    println!(
        "\n{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    println!(
        "{}--- Telegram Bot Setup Guide ---{}",
        COLOR_BOLD, COLOR_RESET
    );
    println!(
        "  1. Open Telegram and search for {}@BotFather{}.",
        HEADING_BLUE, COLOR_RESET
    );
    println!(
        "  2. Start a chat and send the command {}/newbot{}.",
        EMERALD_GREEN, COLOR_RESET
    );
    println!("  3. Choose a name and a username for your bot.");
    println!("  4. Copy the HTTP API token provided by BotFather.\n");

    let token = Password::new("Paste Telegram Bot Token: ")
        .without_confirmation()
        .with_display_mode(PasswordDisplayMode::Masked)
        .prompt()?;
    let trimmed = token.trim();
    match trimmed.is_empty() {
        true => println!("{}⚠️ Token unchanged.{}", AURA_GOLD, COLOR_RESET),
        false => {
            let mut tg = config.channels.telegram.clone().unwrap_or_default();
            tg.enabled = true;
            tg.bot_token = trimmed.to_string();
            config.channels.telegram = Some(tg);
            save_config(config)?;
            println!(
                "{}✓ Telegram bot configured successfully!{}",
                EMERALD_GREEN, COLOR_RESET
            );
        }
    }
    println!(
        "{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    Ok(())
}

fn setup_systemd_service() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not locate home directory"))?;
    let service_dir = home.join(".config").join("systemd").join("user");
    std::fs::create_dir_all(&service_dir)?;

    let exe_path =
        std::env::current_exe().unwrap_or_else(|_| home.join(".cargo").join("bin").join("openz"));

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
    println!(
        "\n{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    println!(
        "{}--- Gateway (WebSocket) Configuration ---{}",
        COLOR_BOLD, COLOR_RESET
    );

    let mut ws = config.channels.websocket.clone().unwrap_or_default();

    let enabled = Confirm::new("Enable WebSocket gateway?")
        .with_default(ws.enabled)
        .prompt()?;
    ws.enabled = enabled;

    match enabled {
        true => {
            let port_input =
                Text::new(&format!("WebSocket Port [default: {}]:", ws.port)).prompt()?;
            apply_parsed_input(&mut ws.port, &port_input);

            let host_input =
                Text::new(&format!("WebSocket Host [default: {}]:", ws.host)).prompt()?;
            apply_non_empty_input(&mut ws.host, &host_input);

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
                    match setup_systemd_service() {
                        Ok(()) => {
                            println!(
                                "{}✓ systemd service openz-gateway.service installed and enabled successfully!{}",
                                EMERALD_GREEN, COLOR_RESET
                            );
                        }
                        Err(e) => {
                            eprintln!(
                                "{}✕ Failed to setup systemd service: {}{}",
                                ERROR_RED, e, COLOR_RESET
                            );
                        }
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
        }
        false => {
            let _ = disable_systemd_service();
        }
    }

    config.channels.websocket = Some(ws);
    save_config(config)?;
    println!(
        "{}✓ Gateway configured successfully!{}",
        EMERALD_GREEN, COLOR_RESET
    );
    println!(
        "{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    Ok(())
}

async fn handle_discord_submenu(config: &mut Config) -> Result<()> {
    println!(
        "\n{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    println!(
        "{}--- Discord Bot Configuration ---{}",
        COLOR_BOLD, COLOR_RESET
    );

    let mut dc = config.channels.discord.clone().unwrap_or_default();

    let enabled = Confirm::new("Enable Discord Bot channel?")
        .with_default(dc.enabled)
        .prompt()?;
    dc.enabled = enabled;

    match enabled {
        true => {
            let token = Password::new("Paste Discord Bot Token: ")
                .without_confirmation()
                .with_display_mode(PasswordDisplayMode::Masked)
                .prompt()?;
            apply_non_empty_input(&mut dc.bot_token, &token);
        }
        false => {}
    }

    config.channels.discord = Some(dc);
    save_config(config)?;
    println!(
        "{}✓ Discord channel configured successfully!{}",
        EMERALD_GREEN, COLOR_RESET
    );
    println!(
        "{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    Ok(())
}

async fn handle_whatsapp_submenu(config: &mut Config) -> Result<()> {
    println!(
        "\n{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    println!(
        "{}--- WhatsApp Channel Configuration ---{}",
        COLOR_BOLD, COLOR_RESET
    );

    let mut wa = config.channels.whatsapp.clone().unwrap_or_default();

    let enabled = Confirm::new("Enable WhatsApp channel?")
        .with_default(wa.enabled)
        .prompt()?;
    wa.enabled = enabled;

    match enabled {
        true => {
            let key = Password::new("Paste WhatsApp API Key: ")
                .without_confirmation()
                .with_display_mode(PasswordDisplayMode::Masked)
                .prompt()?;
            apply_non_empty_input(&mut wa.api_key, &key);

            let phone_id = Text::new("Enter WhatsApp Phone Number ID: ").prompt()?;
            apply_non_empty_input(&mut wa.phone_number_id, &phone_id);

            let port_str = Text::new("Enter WhatsApp Webhook Port: ")
                .with_default(&wa.webhook_port.to_string())
                .prompt()?;
            apply_parsed_input(&mut wa.webhook_port, &port_str);

            let token = Text::new("Enter WhatsApp Webhook Verification Token: ")
                .with_default(&wa.verify_token)
                .prompt()?;
            apply_non_empty_input(&mut wa.verify_token, &token);
        }
        false => {}
    }

    config.channels.whatsapp = Some(wa);
    save_config(config)?;
    println!(
        "{}✓ WhatsApp channel configured successfully!{}",
        EMERALD_GREEN, COLOR_RESET
    );
    println!(
        "{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    Ok(())
}

async fn handle_email_submenu(config: &mut Config) -> Result<()> {
    println!(
        "\n{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    println!(
        "{}--- Email Channel Configuration ---{}",
        COLOR_BOLD, COLOR_RESET
    );

    let mut em = config.channels.email.clone().unwrap_or_default();

    let enabled = Confirm::new("Enable Email channel?")
        .with_default(em.enabled)
        .prompt()?;
    em.enabled = enabled;

    match enabled {
        true => {
            let imap_server = Text::new("Enter IMAP Server (e.g. imap.gmail.com): ")
                .with_default(&em.imap_server)
                .prompt()?;
            apply_non_empty_input(&mut em.imap_server, &imap_server);

            let imap_port_str = Text::new("Enter IMAP Port: ")
                .with_default(&em.imap_port.to_string())
                .prompt()?;
            apply_parsed_input(&mut em.imap_port, &imap_port_str);

            let smtp_server = Text::new("Enter SMTP Server (e.g. smtp.gmail.com): ")
                .with_default(&em.smtp_server)
                .prompt()?;
            apply_non_empty_input(&mut em.smtp_server, &smtp_server);

            let smtp_port_str = Text::new("Enter SMTP Port: ")
                .with_default(&em.smtp_port.to_string())
                .prompt()?;
            apply_parsed_input(&mut em.smtp_port, &smtp_port_str);

            let username = Text::new("Enter Username / Email: ")
                .with_default(&em.username)
                .prompt()?;
            apply_non_empty_input(&mut em.username, &username);

            let password = Password::new("Enter Password / App Password: ")
                .without_confirmation()
                .with_display_mode(PasswordDisplayMode::Masked)
                .prompt()?;
            apply_non_empty_input(&mut em.password, &password);

            let poll_str = Text::new("Enter Email Poll Interval (seconds): ")
                .with_default(&em.poll_interval_secs.to_string())
                .prompt()?;
            apply_parsed_input(&mut em.poll_interval_secs, &poll_str);
        }
        false => {}
    }

    config.channels.email = Some(em);
    save_config(config)?;
    println!(
        "{}✓ Email channel configured successfully!{}",
        EMERALD_GREEN, COLOR_RESET
    );
    println!(
        "{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    Ok(())
}

async fn handle_tui_submenu(config: &mut Config) -> Result<()> {
    println!(
        "\n{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    println!("{}--- TUI Configuration ---{}", COLOR_BOLD, COLOR_RESET);
    println!("Controls how private reasoning/trace summaries are displayed in the terminal UI.");
    println!();

    let options = vec![
        "Full debug trace with seconds".to_string(),
        "Compact trace summary".to_string(),
        "Off".to_string(),
        "Back".to_string(),
    ];
    let choice = match select_menu_custom(
        "Choose TUI trace visibility:",
        &options,
        &config.agents.defaults.model,
        Some("TUI Settings"),
        true,
    )? {
        Some(idx) => idx,
        None => return Ok(()),
    };

    let mode = match choice {
        0 => "full",
        1 => "compact",
        2 => "off",
        _ => return Ok(()),
    };

    config.agents.defaults.tui_thought_display = mode.to_string();
    save_config(config)?;
    println!(
        "{}✓ TUI trace visibility set to {}.{}",
        EMERALD_GREEN, mode, COLOR_RESET
    );
    println!(
        "{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    Ok(())
}

async fn handle_browser_submenu(config: &mut Config) -> Result<()> {
    println!(
        "\n{}--- Firefox Browser Configuration ---{}",
        COLOR_BOLD, COLOR_RESET
    );
    println!("Configure the WebDriver ports used by headless, visible, and attach modes.");

    let webdriver_default = config.browser.firefox_webdriver_port.to_string();
    let webdriver_input = Text::new("Firefox WebDriver port:")
        .with_default(&webdriver_default)
        .prompt()?;
    let attach_default = config.browser.firefox_attach_port.to_string();
    let attach_input = Text::new("Firefox attach port:")
        .with_default(&attach_default)
        .prompt()?;

    let parse_port = |value: &str, name: &str| -> Result<u16> {
        let port = value
            .trim()
            .parse::<u16>()
            .map_err(|_| anyhow!("{} must be a valid integer port", name))?;
        match (1024..=65535).contains(&port) {
            true => Ok(port),
            false => Err(anyhow!("{} must be between 1024 and 65535", name)),
        }
    };

    config.browser.firefox_webdriver_port = parse_port(&webdriver_input, "Firefox WebDriver port")?;
    config.browser.firefox_attach_port = parse_port(&attach_input, "Firefox attach port")?;
    save_config(config)?;
    println!(
        "{}✓ Firefox browser ports saved.{}",
        EMERALD_GREEN, COLOR_RESET
    );
    Ok(())
}

async fn handle_sandbox_submenu(config: &mut Config) -> Result<()> {
    println!(
        "\n{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );
    println!(
        "{}--- Sandbox (seccomp) Configuration ---{}",
        COLOR_BOLD, COLOR_RESET
    );
    println!(
        "The process sandbox (seccomp) restricts system calls (network, browser, tools like ps/which) inside the command execution sandbox."
    );
    println!(
        "Disabling it allows browser automation tools (gsd_browser, chromewright) and local compiler tools to run without seccomp blocking."
    );
    println!("Security is still enforced via openz's internal SecurityGuard prompt confirmations.");
    println!();

    let current = config.agents.defaults.enable_sandbox;
    let enable = Confirm::new("Enable process seccomp sandbox?")
        .with_default(current)
        .prompt()?;

    config.agents.defaults.enable_sandbox = enable;
    save_config(config)?;

    match enable {
        true => {
            println!(
                "{}✓ Sandbox enabled successfully!{}",
                EMERALD_GREEN, COLOR_RESET
            );
        }
        false => {
            println!(
                "{}✓ Sandbox disabled successfully! (Highly recommended for browser tools and developer shells){}",
                EMERALD_GREEN, COLOR_RESET
            );
        }
    }
    println!(
        "{}────────────────────────────────────────────────────────────{}",
        LIGHT_WHITE, COLOR_RESET
    );

    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    Ok(())
}

#[cfg(test)]
#[path = "configure_tests.rs"]
mod tests;

