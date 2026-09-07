use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::channels::ProviderModels;
use crate::config::schema::AgentDefaults;
use std::io::{self, Write};
use std::path::Path;

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
        crate::tui_eprintln!()
    };
    ($($arg:tt)*) => {
        crate::tui_eprintln!($($arg)*)
    };
}

pub enum SlashCommandOutcome {
    Handled,
    Exit,
    ExecuteTurn(String),
    Unhandled,
}

pub async fn handle_slash_command(
    trimmed: &str,
    agent_loop: &tokio::sync::Mutex<AgentLoop>,
    defaults: &tokio::sync::Mutex<AgentDefaults>,
    session_key: &str,
    _workspace: &Path,
) -> SlashCommandOutcome {
    if trimmed == "/exit" || trimmed == "exit" || trimmed == "quit" {
        println!("Goodbye!");
        return SlashCommandOutcome::Exit;
    }

    if trimmed == "/clear" {
        use crossterm::ExecutableCommand;
        let mut stdout = io::stdout();
        let _ = stdout.execute(crossterm::terminal::Clear(
            crossterm::terminal::ClearType::All,
        ));
        let _ = stdout.execute(crossterm::terminal::Clear(
            crossterm::terminal::ClearType::Purge,
        ));
        let _ = stdout.execute(crossterm::cursor::MoveTo(0, 0));
        let _ = stdout.flush();
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/help" {
        println!("{}Available commands:{}", COLOR_BOLD, COLOR_RESET);
        for &(cmd, desc) in super::render::SLASH_COMMANDS {
            println!("  {}{:<12}{} - {}", RED_ORANGE, cmd, COLOR_RESET, desc);
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/settings" {
        let defaults = defaults.lock().await;
        println!("{}Active Settings:{}", COLOR_BOLD, COLOR_RESET);
        println!(
            "  {}Model:{}          {}",
            RED_ORANGE, COLOR_RESET, defaults.model
        );
        println!(
            "  {}Provider:{}       {}",
            RED_ORANGE, COLOR_RESET, defaults.provider
        );
        println!(
            "  {}Security Mode:{}  {}",
            RED_ORANGE, COLOR_RESET, defaults.security_mode
        );
        println!(
            "  {}TUI Trace:{}      {}",
            RED_ORANGE, COLOR_RESET, defaults.tui_thought_display
        );
        println!(
            "  {}Streaming:{}      {}",
            RED_ORANGE,
            COLOR_RESET,
            if defaults.streaming {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!(
            "  {}Sandbox:{}        {}",
            RED_ORANGE,
            COLOR_RESET,
            if defaults.enable_sandbox {
                "Enabled"
            } else {
                "Disabled"
            }
        );

        println!(
            "  {}Whitelisted Command Prefixes:{}",
            RED_ORANGE, COLOR_RESET
        );
        if defaults.whitelisted_command_prefixes.is_empty() {
            println!("    (None)");
        } else {
            for prefix in &defaults.whitelisted_command_prefixes {
                println!("    • {}", prefix);
            }
        }

        println!("  {}Whitelisted Paths:{}", RED_ORANGE, COLOR_RESET);
        if defaults.whitelisted_paths.is_empty() {
            println!("    (None)");
        } else {
            for path in &defaults.whitelisted_paths {
                println!("    • {}", path);
            }
        }

        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/streaming" {
        let current_streaming = defaults.lock().await.streaming;
        let options = vec![
            format!(
                "Enable streaming{}",
                if current_streaming { " (current)" } else { "" }
            ),
            format!(
                "Disable streaming{}",
                if !current_streaming { " (current)" } else { "" }
            ),
            "Back".to_string(),
        ];
        let header = format!(
            "Current streaming mode: {}",
            if current_streaming {
                "enabled"
            } else {
                "disabled"
            }
        );
        let active_mdl = defaults.lock().await.model.clone();
        match select_menu_custom(
            "Choose response streaming mode:",
            &options,
            &active_mdl,
            Some(&header),
            true,
        ) {
            Ok(Some(choice @ 0..=1)) => {
                let enable = choice == 0;
                match crate::config::loader::load_config() {
                    Ok(mut config) => {
                        config.agents.defaults.streaming = enable;
                        match crate::config::loader::save_config(&config) {
                            Ok(()) => {
                                *defaults.lock().await = config.agents.defaults.clone();
                                println!(
                                    "{}✓ Response streaming {}.{}",
                                    EMERALD_GREEN,
                                    if enable { "enabled" } else { "disabled" },
                                    COLOR_RESET
                                );
                            }
                            Err(e) => eprintln!(
                                "{}✕ Error: Failed to save config: {}{}",
                                ERROR_RED, e, COLOR_RESET
                            ),
                        }
                    }
                    Err(e) => eprintln!(
                        "{}✕ Error: Failed to load config: {}{}",
                        ERROR_RED, e, COLOR_RESET
                    ),
                }
            }
            Ok(Some(_)) | Ok(None) => println!("No changes made."),
            Err(e) => eprintln!(
                "{}✕ Error: Failed to open streaming menu: {}{}",
                ERROR_RED, e, COLOR_RESET
            ),
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/tui" || trimmed == "/tui settings" {
        let defaults = defaults.lock().await;
        println!("{}TUI Settings:{}", COLOR_BOLD, COLOR_RESET);
        println!(
            "  {}Trace visibility:{} {}",
            RED_ORANGE, COLOR_RESET, defaults.tui_thought_display
        );
        println!(
            "Use {} /tui trace <full|compact|off>{} or legacy /tui thoughts.",
            RED_ORANGE, COLOR_RESET
        );
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if let Some(stripped) = trimmed
        .strip_prefix("/tui trace")
        .or_else(|| trimmed.strip_prefix("/tui thoughts"))
        .or_else(|| trimmed.strip_prefix("/thoughts"))
    {
        let mode = match stripped.trim().to_lowercase().as_str() {
            "full" | "on" | "default" => "full",
            "compact" | "summary" => "compact",
            "off" | "none" | "hide" => "off",
            _ => {
                println!("Usage: /tui trace <full|compact|off>");
                println!(
                    "{}────────────────────────────────────────────────────────────{}",
                    LIGHT_WHITE, COLOR_RESET
                );
                return SlashCommandOutcome::Handled;
            }
        };

        match crate::config::loader::load_config() {
            Ok(mut config) => {
                config.agents.defaults.tui_thought_display = mode.to_string();
                match crate::config::loader::save_config(&config) {
                    Ok(()) => {
                        *defaults.lock().await = config.agents.defaults.clone();
                        println!(
                            "{}✓ TUI trace visibility set to {}.{}",
                            EMERALD_GREEN, mode, COLOR_RESET
                        );
                    }
                    Err(e) => eprintln!(
                        "{}✕ Error: Failed to save config: {}{}",
                        ERROR_RED, e, COLOR_RESET
                    ),
                }
            }
            Err(e) => eprintln!(
                "{}✕ Error: Failed to load config: {}{}",
                ERROR_RED, e, COLOR_RESET
            ),
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if let Some(stripped) = trimmed.strip_prefix("/device") {
        if let Err(e) = super::device::handle_device_command(stripped.trim()).await {
            eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET);
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/servers" {
        let servers = crate::shutdown::list_registered_children();
        if servers.is_empty() {
            println!("No OpenZ-launched background servers running.");
        } else {
            println!("{}OpenZ background servers:{}", COLOR_BOLD, COLOR_RESET);
            for server in servers {
                println!(
                    "  {}#{}{} pid={} {} - {}",
                    RED_ORANGE,
                    server.id,
                    COLOR_RESET,
                    server.pid,
                    server.kind,
                    server.command
                );
            }
            println!(
                "Use {} /stop-server <id>{} or {} /stop-server all{}.",
                RED_ORANGE, COLOR_RESET, RED_ORANGE, COLOR_RESET
            );
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if let Some(stripped) = trimmed.strip_prefix("/stop-server") {
        let target = stripped.trim();
        if target.is_empty() {
            println!("Usage: /stop-server <id|all>");
        } else {
            match crate::shutdown::stop_registered_child(target) {
                Ok(0) => println!("No matching background server found."),
                Ok(count) => println!(
                    "{}✓ Stopped {} background server(s).{}",
                    EMERALD_GREEN, count, COLOR_RESET
                ),
                Err(e) => {
                    println!("{}✕ Failed to stop server: {}{}", ERROR_RED, e, COLOR_RESET)
                }
            }
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if let Some(stripped) = trimmed.strip_prefix("/model") {
        let arg = stripped.trim();
        if arg.is_empty() {
            use crate::config::loader::load_config;
            let config = match load_config() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "{}✕ Error: Failed to load config: {}{}",
                        ERROR_RED, e, COLOR_RESET
                    );
                    return SlashCommandOutcome::Handled;
                }
            };

            let provider_list = crate::channels::provider_model_catalog();

            let filtered_providers: Vec<&ProviderModels> = provider_list
                .iter()
                .filter(|p| config.is_provider_configured(p.name))
                .collect();

            let custom_provider_options = config
                .custom_provider_names()
                .into_iter()
                .filter(|name| config.is_provider_available(name))
                .map(|name| {
                    let default_model = config.custom_provider_default_model(&name);
                    let display = format!(
                        "Custom: {} ({})",
                        name,
                        default_model.as_deref().unwrap_or("custom model")
                    );
                    let models = default_model.into_iter().collect::<Vec<_>>();
                    (name, display, models)
                })
                .collect::<Vec<_>>();

            if filtered_providers.is_empty() && custom_provider_options.is_empty() {
                println!(
                    "{}⚠️ No LLM providers configured! Please run 'openz configure' first.{}",
                    crate::agent::style::colors::AURA_GOLD,
                    crate::agent::style::colors::COLOR_RESET
                );
                println!(
                    "{}────────────────────────────────────────────────────────────{}",
                    LIGHT_WHITE, COLOR_RESET
                );
                return SlashCommandOutcome::Handled;
            }

            let mut provider_options: Vec<String> = filtered_providers
                .iter()
                .map(|p| format!("{} ({})", p.display, p.models.len()))
                .collect();
            provider_options.extend(
                custom_provider_options
                    .iter()
                    .map(|(_, display, _)| display.clone()),
            );
            provider_options.push("Exit".to_string());
            let (active_mdl, current_active_header) = {
                let defaults = defaults.lock().await;
                (
                    defaults.model.clone(),
                    format!(
                        "Current active model: {} | Provider: {}",
                        defaults.model, defaults.provider
                    ),
                )
            };
            match select_menu_custom(
                "Choose an LLM provider:",
                &provider_options,
                &active_mdl,
                Some(&current_active_header),
                false,
            ) {
                Ok(Some(selected_idx)) => {
                    if selected_idx == filtered_providers.len() + custom_provider_options.len() {
                        println!("Model selection cancelled.");
                        println!(
                            "{}────────────────────────────────────────────────────────────{}",
                            LIGHT_WHITE, COLOR_RESET
                        );
                        return SlashCommandOutcome::Handled;
                    }
                    let (prov_name, prov_display, curated_models) =
                        if selected_idx < filtered_providers.len() {
                            let prov_info = filtered_providers[selected_idx];
                            let mut models = prov_info
                                .models
                                .iter()
                                .map(|model| model.to_string())
                                .collect::<Vec<_>>();
                            if let Some(default_model) = config
                                .get_provider_config(prov_info.name)
                                .and_then(|provider| provider.default_model.clone())
                                .filter(|model| !model.trim().is_empty())
                            {
                                if !models.contains(&default_model) {
                                    models.push(default_model);
                                }
                            }
                            (
                                prov_info.name.to_string(),
                                format!("{} ({})", prov_info.display, prov_info.models.len()),
                                models,
                            )
                        } else {
                            let custom_idx = selected_idx - filtered_providers.len();
                            custom_provider_options[custom_idx].clone()
                        };
                    if prov_name == "ollama_local" {
                        crate::providers::ollama_manager::ensure_local_ollama(&config);
                    }

                    print!(
                        "{}◇ Fetching models for {}...{}",
                        AURA_SLATE, prov_display, COLOR_RESET
                    );
                    let _ = std::io::stdout().flush();

                    let mut model_options = match crate::channels::fetch_provider_models(
                        &prov_name, &config,
                    )
                    .await
                    {
                        Some(mut models) => {
                            print!("\r\x1b[2K");
                            let _ = std::io::stdout().flush();
                            for m in &curated_models {
                                if !models.contains(m) {
                                    models.push(m.clone());
                                }
                            }
                            models.sort();
                            models
                        }
                        None => {
                            print!("\r\x1b[2K");
                            let _ = std::io::stdout().flush();
                            curated_models.clone()
                        }
                    };
                    model_options.push("Type manually (Custom Model)".to_string());
                    model_options.push("Exit".to_string());

                    match select_menu_custom(
                        &format!("Choose a model from {}:", prov_display),
                        &model_options,
                        &active_mdl,
                        None,
                        false,
                    ) {
                        Ok(Some(selected_model_idx)) => {
                            if selected_model_idx == model_options.len() - 1 {
                                println!("Model selection cancelled.");
                                println!(
                                    "{}────────────────────────────────────────────────────────────{}",
                                    LIGHT_WHITE, COLOR_RESET
                                );
                                return SlashCommandOutcome::Handled;
                            }
                            let prov = prov_name.as_str();
                            let mdl = if selected_model_idx == model_options.len() - 2 {
                                match inquire::Text::new("Enter custom model name:").prompt() {
                                    Ok(custom) => {
                                        if custom.trim().is_empty() {
                                            println!("Model selection cancelled.");
                                            println!(
                                                "{}────────────────────────────────────────────────────────────{}",
                                                LIGHT_WHITE, COLOR_RESET
                                            );
                                            return SlashCommandOutcome::Handled;
                                        }
                                        custom.trim().to_string()
                                    }
                                    Err(_) => {
                                        println!("Model selection cancelled.");
                                        println!(
                                            "{}────────────────────────────────────────────────────────────{}",
                                            LIGHT_WHITE, COLOR_RESET
                                        );
                                        return SlashCommandOutcome::Handled;
                                    }
                                }
                            } else {
                                model_options[selected_model_idx].clone()
                            };

                            use crate::config::loader::{load_config, save_config};
                            match load_config() {
                                Ok(mut config) => {
                                    config.agents.defaults.provider = prov.to_string();
                                    config.agents.defaults.model = mdl.clone();
                                    if let Err(e) = save_config(&config) {
                                        eprintln!(
                                            "{}✕ Error: Failed to save config: {}{}",
                                            ERROR_RED, e, COLOR_RESET
                                        );
                                    } else {
                                        match crate::providers::resolver::resolve_provider_full(
                                            &config, &mdl,
                                        ) {
                                            Ok(resolved) => {
                                                let mut loop_lock = agent_loop.lock().await;
                                                loop_lock.update_model_and_provider(
                                                    config.clone(),
                                                    resolved.instance,
                                                );
                                                let new_defaults = config.agents.defaults.clone();
                                                if let Ok(mut guard) =
                                                    super::render::CUSTOM_CONTEXT_LIMIT.lock()
                                                {
                                                    *guard = new_defaults.context_limit;
                                                }
                                                *defaults.lock().await = new_defaults;
                                                println!(
                                                    "{}✓ Model updated to {} (provider: {}){}",
                                                    EMERALD_GREEN, mdl, prov, COLOR_RESET
                                                );
                                                let warning =
                                                    crate::channels::render_model_risk_warning(
                                                        prov, &mdl,
                                                    );
                                                if !warning.is_empty() {
                                                    println!(
                                                        "{}{}{}",
                                                        AURA_GOLD, warning, COLOR_RESET
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "{}✕ Error: Failed to initialize new model: {}{}",
                                                    ERROR_RED, e, COLOR_RESET
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "{}✕ Error: Failed to load config: {}{}",
                                        ERROR_RED, e, COLOR_RESET
                                    );
                                }
                            }
                        }
                        Ok(None) => {
                            println!("Model selection cancelled.");
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    println!("Provider selection cancelled.");
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            }
        } else {
            let (prov, mdl) = if let Some(idx) = arg.find('/') {
                (&arg[..idx], &arg[idx + 1..])
            } else {
                ("auto", arg)
            };

            use crate::config::loader::{load_config, save_config};
            match load_config() {
                Ok(mut config) => {
                    config.agents.defaults.provider = prov.to_string();
                    config.agents.defaults.model = mdl.to_string();
                    if let Err(e) = save_config(&config) {
                        eprintln!(
                            "{}✕ Error: Failed to save config: {}{}",
                            ERROR_RED, e, COLOR_RESET
                        );
                    } else {
                        match crate::providers::resolver::resolve_provider_full(&config, mdl) {
                            Ok(resolved) => {
                                let mut loop_lock = agent_loop.lock().await;
                                loop_lock.update_model_and_provider(
                                    config.clone(),
                                    resolved.instance,
                                );
                                let new_defaults = config.agents.defaults.clone();
                                if let Ok(mut guard) = super::render::CUSTOM_CONTEXT_LIMIT.lock() {
                                    *guard = new_defaults.context_limit;
                                }
                                *defaults.lock().await = new_defaults;
                                println!(
                                    "{}✓ Model updated to {} (provider: {}){}",
                                    EMERALD_GREEN, mdl, prov, COLOR_RESET
                                );
                                let warning = crate::channels::render_model_risk_warning(prov, mdl);
                                if !warning.is_empty() {
                                    println!("{}{}{}", AURA_GOLD, warning, COLOR_RESET);
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "{}✕ Error: Failed to resolve provider/model: {}{}",
                                    ERROR_RED, e, COLOR_RESET
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{}✕ Error: Failed to load config: {}{}",
                        ERROR_RED, e, COLOR_RESET
                    );
                }
            }
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/new-session" {
        let session_manager = {
            let loop_lock = agent_loop.lock().await;
            loop_lock.session_manager.clone()
        };
        if let Ok(mut current_session) = session_manager.load(session_key) {
            if !current_session.messages.is_empty() {
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let archive_key = format!("cli:history_{}", timestamp);
                current_session.key = archive_key;
                let _ = session_manager.save(&current_session).await;

                let empty_session = crate::session::Session::new(session_key);
                let _ = session_manager.save(&empty_session).await;
            }
        }
        println!(
            "{}✓ Session reset. Starting a new session.{}",
            EMERALD_GREEN, COLOR_RESET
        );
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/skill" {
        match crate::agent::skills::load_skills() {
            Ok(skills) => {
                if skills.is_empty() {
                    println!("No active skills found in ~/.openz/skills");
                } else {
                    println!("{}Active skills:{}", COLOR_BOLD, COLOR_RESET);
                    for skill in skills {
                        println!("  • {}", skill.name);
                    }
                }
            }
            Err(e) => {
                eprintln!("{}✕ Error loading skills: {}{}", ERROR_RED, e, COLOR_RESET);
            }
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed.starts_with("/sources") {
        let query = trimmed.strip_prefix("/sources").unwrap_or("").trim();
        match crate::tools::shared_memory::search_source_bookmarks(query, 10).await {
            Ok(items) if items.is_empty() => println!("No saved sources matched."),
            Ok(items) => {
                println!("{}Saved sources:{}", COLOR_BOLD, COLOR_RESET);
                for item in items {
                    println!("  • {} [{}] {}", item.label, item.kind, item.uri);
                    if !item.summary.trim().is_empty() {
                        println!("    {}", item.summary.trim());
                    }
                }
            }
            Err(e) => eprintln!(
                "{}✕ Error searching sources: {}{}",
                ERROR_RED, e, COLOR_RESET
            ),
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed.starts_with("/workflows") {
        let query = trimmed.strip_prefix("/workflows").unwrap_or("").trim();
        match crate::tools::shared_memory::search_workflow_cards(query, 10, false).await {
            Ok(items) if items.is_empty() => println!("No reusable workflows matched."),
            Ok(items) => {
                println!("{}Reusable workflows:{}", COLOR_BOLD, COLOR_RESET);
                for item in items {
                    println!(
                        "  • {} [{}] success={} failure={}",
                        item.name, item.status, item.success_count, item.failure_count
                    );
                    println!("    {}", item.summary.trim());
                }
            }
            Err(e) => eprintln!(
                "{}✕ Error searching workflows: {}{}",
                ERROR_RED, e, COLOR_RESET
            ),
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/mcps" {
        let loop_lock = agent_loop.lock().await;
        println!("{}Configured MCP Servers:{}", COLOR_BOLD, COLOR_RESET);
        if loop_lock.config.mcp_servers.is_empty() {
            println!("  No MCP servers configured.");
        } else {
            for (name, mcp_cfg) in &loop_lock.config.mcp_servers {
                let status = if mcp_cfg.enabled {
                    format!("{}enabled{}", EMERALD_GREEN, COLOR_RESET)
                } else {
                    format!("{}disabled{}", AURA_SLATE, COLOR_RESET)
                };
                println!("  • {} ({}) - {}", name, status, mcp_cfg.command);
            }
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/history" {
        let session_manager = {
            let loop_lock = agent_loop.lock().await;
            loop_lock.session_manager.clone()
        };
        match crate::cli::load_session_history() {
            Ok(history) => {
                if history.is_empty() {
                    println!("No session history found.");
                } else {
                    match select_menu_with_history_first_option(
                        "Select a previous session or continue current:",
                        &history,
                        "Continue Current Session",
                    ) {
                        Ok(selected) => {
                            if selected == 0 {
                                println!(
                                    "{}✓ Continuing current session.{}",
                                    EMERALD_GREEN, COLOR_RESET
                                );
                            } else {
                                let selected_item = &history[selected - 1];
                                if selected_item.key != session_key {
                                    let _ = crate::cli::archive_current_session(
                                        &session_manager,
                                        session_key,
                                    )
                                    .await;
                                    if let Ok(mut session) =
                                        session_manager.load(&selected_item.key)
                                    {
                                        session.key = session_key.to_string();
                                        let _ = session_manager.save(&session).await;
                                        println!(
                                            "{}✓ Loaded session: {}{}",
                                            EMERALD_GREEN,
                                            selected_item.display_title,
                                            COLOR_RESET
                                        );
                                        super::render::print_session_history(&session);
                                    }
                                } else {
                                    println!(
                                        "{}✓ You are already in this session.{}",
                                        EMERALD_GREEN, COLOR_RESET
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "{}✕ Error running selection menu: {}{}",
                                ERROR_RED, e, COLOR_RESET
                            );
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "{}✕ Error loading session history: {}{}",
                    ERROR_RED, e, COLOR_RESET
                );
            }
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/memory" {
        let session_manager = {
            let loop_lock = agent_loop.lock().await;
            loop_lock.session_manager.clone()
        };
        if let Ok(session) = session_manager.load(session_key) {
            println!("{}Session Metadata & Memory:{}", COLOR_BOLD, COLOR_RESET);
            if session.metadata.is_empty() {
                println!("  No memory or metadata recorded for this session.");
            } else {
                for (k, v) in &session.metadata {
                    println!("  • {}: {}", k, v);
                }
            }
        } else {
            println!("No active session found.");
        }
        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );
        return SlashCommandOutcome::Handled;
    }

    if trimmed == "/paste" || trimmed == "/clip" {
        match super::input::handle_clipboard_paste(0) {
            Ok(img_path) => {
                println!(
                    "{}✓ Image captured from clipboard and saved to: {}{}",
                    EMERALD_GREEN,
                    img_path.display(),
                    COLOR_RESET
                );
                print!("Enter query/instructions for this image: ");
                let _ = io::stdout().flush();
                let mut query = String::new();
                let _ = io::stdin().read_line(&mut query);
                let combined_query = format!(
                    "{} ![](file://{})",
                    query.trim(),
                    img_path.to_string_lossy()
                );
                return SlashCommandOutcome::ExecuteTurn(combined_query);
            }
            Err(e) => {
                eprintln!(
                    "{}✕ Error: Failed to retrieve image from clipboard: {}{}",
                    ERROR_RED, e, COLOR_RESET
                );
                return SlashCommandOutcome::Handled;
            }
        }
    }

    SlashCommandOutcome::Unhandled
}
