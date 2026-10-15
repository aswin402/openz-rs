use crate::agent::style::markdown::horizontal_divider;
use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::channels::ProviderModels;
use crate::config::loader::{load_config, save_config};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedSlashCommand<'a> {
    Exit,
    Clear,
    Help,
    Settings,
    Streaming,
    TuiSettings,
    TuiTrace(&'a str),
    Device(&'a str),
    Servers,
    StopServer(&'a str),
    Model(&'a str),
    NewSession,
    Skill,
    Sources(&'a str),
    Workflows(&'a str),
    Mcps,
    History,
    Memory,
    Paste,
    NotCommand,
}

impl<'a> ParsedSlashCommand<'a> {
    pub fn parse(trimmed: &'a str) -> Self {
        match trimmed {
            "/exit" | "exit" | "quit" => return Self::Exit,
            "/clear" => return Self::Clear,
            "/help" | "/commands" => return Self::Help,
            "/settings" => return Self::Settings,
            "/streaming" => return Self::Streaming,
            "/tui" | "/tui settings" => return Self::TuiSettings,
            "/servers" => return Self::Servers,
            "/new-session" => return Self::NewSession,
            "/skill" => return Self::Skill,
            "/mcps" => return Self::Mcps,
            "/history" => return Self::History,
            "/memory" => return Self::Memory,
            "/paste" | "/clip" => return Self::Paste,
            _ => {}
        }
        match trimmed
            .strip_prefix("/tui trace")
            .or_else(|| trimmed.strip_prefix("/tui thoughts"))
            .or_else(|| trimmed.strip_prefix("/thoughts"))
        {
            Some(rest) => return Self::TuiTrace(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/device") {
            Some(rest) => return Self::Device(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/stop-server") {
            Some(rest) => return Self::StopServer(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/model") {
            Some(rest) => return Self::Model(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/sources") {
            Some(rest) => return Self::Sources(rest.trim()),
            None => {}
        }
        match trimmed.strip_prefix("/workflows") {
            Some(rest) => return Self::Workflows(rest.trim()),
            None => {}
        }
        Self::NotCommand
    }
}

fn print_command_divider() {
    println!("{}{}{}", LIGHT_WHITE, horizontal_divider(60), COLOR_RESET);
}

fn enabled_label(flag: bool) -> &'static str {
    match flag {
        true => "Enabled",
        false => "Disabled",
    }
}

fn enabled_lower_label(flag: bool) -> &'static str {
    match flag {
        true => "enabled",
        false => "disabled",
    }
}

fn current_suffix(flag: bool) -> &'static str {
    match flag {
        true => " (current)",
        false => "",
    }
}

fn print_bullet_list_or_none(items: &[String]) {
    match items.is_empty() {
        true => println!("    (None)"),
        false => {
            for item in items {
                println!("    • {}", item);
            }
        }
    }
}

async fn apply_cli_model_switch(
    prov: &str,
    mdl: &str,
    agent_loop: &tokio::sync::Mutex<AgentLoop>,
    defaults: &tokio::sync::Mutex<AgentDefaults>,
) {
    match load_config() {
        Ok(mut config) => {
            config.agents.defaults.provider = prov.to_string();
            config.agents.defaults.model = mdl.to_string();
            match save_config(&config) {
                Err(e) => eprintln!(
                    "{}✕ Error: Failed to save config: {}{}",
                    ERROR_RED, e, COLOR_RESET
                ),
                Ok(()) => match crate::providers::resolver::resolve_provider_full(&config, mdl) {
                    Ok(resolved) => {
                        let mut loop_lock = agent_loop.lock().await;
                        loop_lock.update_model_and_provider(config.clone(), resolved.instance);
                        let new_defaults = config.agents.defaults.clone();
                        match super::render::CUSTOM_CONTEXT_LIMIT.lock() {
                            Ok(mut guard) => *guard = new_defaults.context_limit,
                            Err(_) => {}
                        }
                        *defaults.lock().await = new_defaults;
                        println!(
                            "{}✓ Model updated to {} (provider: {}){}",
                            EMERALD_GREEN, mdl, prov, COLOR_RESET
                        );
                        let warning = crate::channels::render_model_risk_warning(prov, mdl);
                        match warning.is_empty() {
                            true => {}
                            false => println!("{}{}{}", AURA_GOLD, warning, COLOR_RESET),
                        }
                    }
                    Err(e) => eprintln!(
                        "{}✕ Error: Failed to initialize new model: {}{}",
                        ERROR_RED, e, COLOR_RESET
                    ),
                },
            }
        }
        Err(e) => eprintln!(
            "{}✕ Error: Failed to load config: {}{}",
            ERROR_RED, e, COLOR_RESET
        ),
    }
}

async fn run_interactive_model_selector(
    agent_loop: &tokio::sync::Mutex<AgentLoop>,
    defaults: &tokio::sync::Mutex<AgentDefaults>,
) {
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "{}✕ Error: Failed to load config: {}{}",
                ERROR_RED, e, COLOR_RESET
            );
            return;
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

    match filtered_providers.is_empty() && custom_provider_options.is_empty() {
        true => {
            println!(
                "{}⚠️ No LLM providers configured! Please run 'openz configure' first.{}",
                AURA_GOLD, COLOR_RESET
            );
            return;
        }
        false => {}
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
        let guard = defaults.lock().await;
        (
            guard.model.clone(),
            format!(
                "Current active model: {} | Provider: {}",
                guard.model, guard.provider
            ),
        )
    };

    let selected_idx = match select_menu_custom(
        "Choose an LLM provider:",
        &provider_options,
        &active_mdl,
        Some(&current_active_header),
        false,
    ) {
        Ok(Some(idx)) => idx,
        Ok(None) => {
            println!("Provider selection cancelled.");
            return;
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };

    let total_providers = filtered_providers.len() + custom_provider_options.len();
    match selected_idx >= total_providers {
        true => {
            println!("Model selection cancelled.");
            return;
        }
        false => {}
    }

    let (prov_name, prov_display, curated_models) = match selected_idx < filtered_providers.len() {
        true => {
            let prov_info = filtered_providers[selected_idx];
            let mut models = prov_info
                .models
                .iter()
                .map(|model| model.to_string())
                .collect::<Vec<_>>();
            match config
                .get_provider_config(prov_info.name)
                .and_then(|provider| provider.default_model.clone())
                .filter(|model| !model.trim().is_empty())
            {
                Some(default_model) => match models.contains(&default_model) {
                    true => {}
                    false => models.push(default_model),
                },
                None => {}
            }
            (
                prov_info.name.to_string(),
                format!("{} ({})", prov_info.display, prov_info.models.len()),
                models,
            )
        }
        false => {
            let custom_idx = selected_idx - filtered_providers.len();
            custom_provider_options[custom_idx].clone()
        }
    };

    match prov_name.as_str() {
        "ollama_local" => crate::providers::ollama_manager::ensure_local_ollama(&config),
        _ => {}
    }

    print!(
        "{}◇ Fetching models for {}...{}",
        AURA_SLATE, prov_display, COLOR_RESET
    );
    let _ = std::io::stdout().flush();

    let mut model_options = match crate::channels::fetch_provider_models(&prov_name, &config).await
    {
        Some(mut models) => {
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
            for m in &curated_models {
                match models.contains(m) {
                    true => {}
                    false => models.push(m.clone()),
                }
            }
            models.sort();
            models
        }
        None => {
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
            curated_models
        }
    };
    model_options.push("Type manually (Custom Model)".to_string());
    model_options.push("Exit".to_string());

    let selected_model_idx = match select_menu_custom(
        &format!("Choose a model from {}:", prov_display),
        &model_options,
        &active_mdl,
        None,
        false,
    ) {
        Ok(Some(idx)) => idx,
        Ok(None) => {
            println!("Model selection cancelled.");
            return;
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };

    match selected_model_idx == model_options.len() - 1 {
        true => {
            println!("Model selection cancelled.");
            return;
        }
        false => {}
    }

    let mdl = match selected_model_idx == model_options.len() - 2 {
        true => match inquire::Text::new("Enter custom model name:").prompt() {
            Ok(custom) => {
                let trimmed_custom = custom.trim().to_string();
                match trimmed_custom.is_empty() {
                    true => {
                        println!("Model selection cancelled.");
                        return;
                    }
                    false => trimmed_custom,
                }
            }
            Err(_) => {
                println!("Model selection cancelled.");
                return;
            }
        },
        false => model_options[selected_model_idx].clone(),
    };

    apply_cli_model_switch(&prov_name, &mdl, agent_loop, defaults).await;
}

pub async fn handle_slash_command(
    trimmed: &str,
    agent_loop: &tokio::sync::Mutex<AgentLoop>,
    defaults: &tokio::sync::Mutex<AgentDefaults>,
    session_key: &str,
    _workspace: &Path,
) -> SlashCommandOutcome {
    match ParsedSlashCommand::parse(trimmed) {
        ParsedSlashCommand::Exit => {
            println!("Goodbye!");
            SlashCommandOutcome::Exit
        }
        ParsedSlashCommand::Clear => {
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
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Help => {
            println!("{}Available commands:{}", COLOR_BOLD, COLOR_RESET);
            for &(cmd, desc) in super::render::SLASH_COMMANDS {
                println!("  {}{:<12}{} - {}", RED_ORANGE, cmd, COLOR_RESET, desc);
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Settings => {
            let guard = defaults.lock().await;
            println!("{}Active Settings:{}", COLOR_BOLD, COLOR_RESET);
            println!("  {}Model:{}          {}", RED_ORANGE, COLOR_RESET, guard.model);
            println!(
                "  {}Provider:{}       {}",
                RED_ORANGE, COLOR_RESET, guard.provider
            );
            println!(
                "  {}Security Mode:{}  {}",
                RED_ORANGE, COLOR_RESET, guard.security_mode
            );
            println!(
                "  {}TUI Trace:{}      {}",
                RED_ORANGE, COLOR_RESET, guard.tui_thought_display
            );
            println!(
                "  {}Streaming:{}      {}",
                RED_ORANGE,
                COLOR_RESET,
                enabled_label(guard.streaming)
            );
            println!(
                "  {}Sandbox:{}        {}",
                RED_ORANGE,
                COLOR_RESET,
                enabled_label(guard.enable_sandbox)
            );
            println!(
                "  {}Whitelisted Command Prefixes:{}",
                RED_ORANGE, COLOR_RESET
            );
            print_bullet_list_or_none(&guard.whitelisted_command_prefixes);
            println!("  {}Whitelisted Paths:{}", RED_ORANGE, COLOR_RESET);
            print_bullet_list_or_none(&guard.whitelisted_paths);
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Streaming => {
            let (current_streaming, active_mdl) = {
                let guard = defaults.lock().await;
                (guard.streaming, guard.model.clone())
            };
            let options = vec![
                format!("Enable streaming{}", current_suffix(current_streaming)),
                format!("Disable streaming{}", current_suffix(!current_streaming)),
                "Back".to_string(),
            ];
            let header = format!(
                "Current streaming mode: {}",
                enabled_lower_label(current_streaming)
            );
            match select_menu_custom(
                "Choose response streaming mode:",
                &options,
                &active_mdl,
                Some(&header),
                true,
            ) {
                Ok(Some(choice @ 0..=1)) => {
                    let enable = choice == 0;
                    match load_config() {
                        Ok(mut config) => {
                            config.agents.defaults.streaming = enable;
                            match save_config(&config) {
                                Ok(()) => {
                                    *defaults.lock().await = config.agents.defaults.clone();
                                    println!(
                                        "{}✓ Response streaming {}.{}",
                                        EMERALD_GREEN,
                                        enabled_lower_label(enable),
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
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::TuiSettings => {
            let guard = defaults.lock().await;
            println!("{}TUI Settings:{}", COLOR_BOLD, COLOR_RESET);
            println!(
                "  {}Trace visibility:{} {}",
                RED_ORANGE, COLOR_RESET, guard.tui_thought_display
            );
            println!(
                "Use {} /tui trace <full|compact|off>{} or legacy /tui thoughts.",
                RED_ORANGE, COLOR_RESET
            );
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::TuiTrace(arg) => {
            let mode = match arg.to_lowercase().as_str() {
                "full" | "on" | "default" => "full",
                "compact" | "summary" => "compact",
                "off" | "none" | "hide" => "off",
                _ => {
                    println!("Usage: /tui trace <full|compact|off>");
                    print_command_divider();
                    return SlashCommandOutcome::Handled;
                }
            };
            match load_config() {
                Ok(mut config) => {
                    config.agents.defaults.tui_thought_display = mode.to_string();
                    match save_config(&config) {
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
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Device(arg) => {
            match super::device::handle_device_command(arg).await {
                Ok(()) => {}
                Err(e) => eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET),
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Servers => {
            let servers = crate::shutdown::list_registered_children();
            match servers.is_empty() {
                true => println!("No OpenZ-launched background servers running."),
                false => {
                    println!("{}OpenZ background servers:{}", COLOR_BOLD, COLOR_RESET);
                    for server in servers {
                        println!(
                            "  {}#{}{} pid={} {} - {}",
                            RED_ORANGE, server.id, COLOR_RESET, server.pid, server.kind, server.command
                        );
                    }
                    println!(
                        "Use {} /stop-server <id>{} or {} /stop-server all{}.",
                        RED_ORANGE, COLOR_RESET, RED_ORANGE, COLOR_RESET
                    );
                }
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::StopServer(target) => {
            match target.is_empty() {
                true => println!("Usage: /stop-server <id|all>"),
                false => match crate::shutdown::stop_registered_child(target) {
                    Ok(0) => println!("No matching background server found."),
                    Ok(count) => println!(
                        "{}✓ Stopped {} background server(s).{}",
                        EMERALD_GREEN, count, COLOR_RESET
                    ),
                    Err(e) => println!("{}✕ Failed to stop server: {}{}", ERROR_RED, e, COLOR_RESET),
                },
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Model(arg) => {
            match arg.is_empty() {
                true => run_interactive_model_selector(agent_loop, defaults).await,
                false => {
                    let (prov, mdl) = match arg.find('/') {
                        Some(idx) => (&arg[..idx], &arg[idx + 1..]),
                        None => ("auto", arg),
                    };
                    apply_cli_model_switch(prov, mdl, agent_loop, defaults).await;
                }
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::NewSession => {
            let session_manager = {
                let loop_lock = agent_loop.lock().await;
                loop_lock.session_manager.clone()
            };
            match session_manager.load(session_key) {
                Ok(mut current_session) => match current_session.messages.is_empty() {
                    true => {}
                    false => {
                        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                        current_session.key = format!("cli:history_{}", timestamp);
                        let _ = session_manager.save(&current_session).await;

                        let empty_session = crate::session::Session::new(session_key);
                        let _ = session_manager.save(&empty_session).await;
                    }
                },
                Err(_) => {}
            }
            println!(
                "{}✓ Session reset. Starting a new session.{}",
                EMERALD_GREEN, COLOR_RESET
            );
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Skill => {
            match crate::agent::skills::load_skills() {
                Ok(skills) => match skills.is_empty() {
                    true => println!("No active skills found in ~/.openz/skills"),
                    false => {
                        println!("{}Active skills:{}", COLOR_BOLD, COLOR_RESET);
                        for skill in skills {
                            println!("  • {}", skill.name);
                        }
                    }
                },
                Err(e) => eprintln!("{}✕ Error loading skills: {}{}", ERROR_RED, e, COLOR_RESET),
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Sources(query) => {
            match crate::tools::shared_memory::search_source_bookmarks(query, 10).await {
                Ok(items) => match items.is_empty() {
                    true => println!("No saved sources matched."),
                    false => {
                        println!("{}Saved sources:{}", COLOR_BOLD, COLOR_RESET);
                        for item in items {
                            println!("  • {} [{}] {}", item.label, item.kind, item.uri);
                            match item.summary.trim().is_empty() {
                                true => {}
                                false => println!("    {}", item.summary.trim()),
                            }
                        }
                    }
                },
                Err(e) => eprintln!(
                    "{}✕ Error searching sources: {}{}",
                    ERROR_RED, e, COLOR_RESET
                ),
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Workflows(query) => {
            match crate::tools::shared_memory::search_workflow_cards(query, 10, false).await {
                Ok(items) => match items.is_empty() {
                    true => println!("No reusable workflows matched."),
                    false => {
                        println!("{}Reusable workflows:{}", COLOR_BOLD, COLOR_RESET);
                        for item in items {
                            println!(
                                "  • {} [{}] success={} failure={}",
                                item.name, item.status, item.success_count, item.failure_count
                            );
                            println!("    {}", item.summary.trim());
                        }
                    }
                },
                Err(e) => eprintln!(
                    "{}✕ Error searching workflows: {}{}",
                    ERROR_RED, e, COLOR_RESET
                ),
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Mcps => {
            let loop_lock = agent_loop.lock().await;
            println!("{}Configured MCP Servers:{}", COLOR_BOLD, COLOR_RESET);
            match loop_lock.config.mcp_servers.is_empty() {
                true => println!("  No MCP servers configured."),
                false => {
                    for (name, mcp_cfg) in &loop_lock.config.mcp_servers {
                        let status = match mcp_cfg.enabled {
                            true => format!("{}enabled{}", EMERALD_GREEN, COLOR_RESET),
                            false => format!("{}disabled{}", AURA_SLATE, COLOR_RESET),
                        };
                        println!("  • {} ({}) - {}", name, status, mcp_cfg.command);
                    }
                }
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::History => {
            let session_manager = {
                let loop_lock = agent_loop.lock().await;
                loop_lock.session_manager.clone()
            };
            match crate::cli::load_session_history() {
                Ok(history) => match history.is_empty() {
                    true => println!("No session history found."),
                    false => match select_menu_with_history_first_option(
                        "Select a previous session or continue current:",
                        &history,
                        "Continue Current Session",
                    ) {
                        Ok(0) => println!(
                            "{}✓ Continuing current session.{}",
                            EMERALD_GREEN, COLOR_RESET
                        ),
                        Ok(selected) => {
                            let selected_item = &history[selected - 1];
                            match selected_item.key == session_key {
                                true => println!(
                                    "{}✓ You are already in this session.{}",
                                    EMERALD_GREEN, COLOR_RESET
                                ),
                                false => {
                                    let _ = crate::cli::archive_current_session(
                                        &session_manager,
                                        session_key,
                                    )
                                    .await;
                                    match session_manager.load(&selected_item.key) {
                                        Ok(mut session) => {
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
                                        Err(_) => {}
                                    }
                                }
                            }
                        }
                        Err(e) => eprintln!(
                            "{}✕ Error running selection menu: {}{}",
                            ERROR_RED, e, COLOR_RESET
                        ),
                    },
                },
                Err(e) => eprintln!(
                    "{}✕ Error loading session history: {}{}",
                    ERROR_RED, e, COLOR_RESET
                ),
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Memory => {
            let session_manager = {
                let loop_lock = agent_loop.lock().await;
                loop_lock.session_manager.clone()
            };
            match session_manager.load(session_key) {
                Ok(session) => {
                    println!("{}Session Metadata & Memory:{}", COLOR_BOLD, COLOR_RESET);
                    match session.metadata.is_empty() {
                        true => println!("  No memory or metadata recorded for this session."),
                        false => {
                            for (k, v) in &session.metadata {
                                println!("  • {}: {}", k, v);
                            }
                        }
                    }
                }
                Err(_) => println!("No active session found."),
            }
            print_command_divider();
            SlashCommandOutcome::Handled
        }
        ParsedSlashCommand::Paste => match super::input::handle_clipboard_paste(0) {
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
                SlashCommandOutcome::ExecuteTurn(combined_query)
            }
            Err(e) => {
                eprintln!(
                    "{}✕ Error: Failed to retrieve image from clipboard: {}{}",
                    ERROR_RED, e, COLOR_RESET
                );
                SlashCommandOutcome::Handled
            }
        },
        ParsedSlashCommand::NotCommand => SlashCommandOutcome::Unhandled,
    }
}
