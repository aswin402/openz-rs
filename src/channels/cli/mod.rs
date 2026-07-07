use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::config::schema::AgentDefaults;
use anyhow::Result;
use std::io::{self, Write};

pub mod input;
pub mod mcp;
pub mod render;

// Re-export mcp progress bar functions/states
pub use mcp::{
    increment_mcp_failed, increment_mcp_loaded, init_mcp_progress, queue_notification,
    send_notification, set_mcp_done, set_mcp_status,
};

// Re-export render custom limit
pub use render::CUSTOM_CONTEXT_LIMIT;

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

#[allow(unused_macros)]
macro_rules! eprint {
    () => {
        crate::tui_eprint!()
    };
    ($($arg:tt)*) => {
        crate::tui_eprint!($($arg)*)
    };
}

pub struct RawModeGuard;

impl RawModeGuard {
    pub fn new() -> Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(RawModeGuard)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableBracketedPaste);
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub struct CliChannel {
    agent_loop: tokio::sync::Mutex<AgentLoop>,
    defaults: tokio::sync::Mutex<AgentDefaults>,
}

impl CliChannel {
    pub fn new(agent_loop: AgentLoop, defaults: AgentDefaults) -> Self {
        static PANIC_HOOK: std::sync::Once = std::sync::Once::new();
        PANIC_HOOK.call_once(|| {
            let default_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic_info| {
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    crossterm::terminal::LeaveAlternateScreen
                );
                default_hook(panic_info);
            }));
        });

        if let Ok(mut guard) = CUSTOM_CONTEXT_LIMIT.lock() {
            *guard = defaults.context_limit;
        }
        CliChannel {
            agent_loop: tokio::sync::Mutex::new(agent_loop),
            defaults: tokio::sync::Mutex::new(defaults),
        }
    }
}

#[async_trait::async_trait]
impl crate::channels::Channel for CliChannel {
    fn name(&self) -> &'static str {
        "cli"
    }

    async fn start(&self) -> anyhow::Result<()> {
        crate::agent::style::spinner::IS_SILENT
            .scope(false, async move { self.start_inner().await })
            .await
    }
}

struct CliActiveGuard;
impl Drop for CliActiveGuard {
    fn drop(&mut self) {
        crate::shutdown::set_cli_active(false);
    }
}

impl CliChannel {
    async fn start_inner(&self) -> anyhow::Result<()> {
        // Derive a unique session key per workspace directory so multiple
        // `openz agent` instances can run in different directories.
        let session_key = crate::config::loader::get_cli_session_key();
        crate::shutdown::set_cli_active(true);
        let _guard = CliActiveGuard;

        let white = "\x1b[38;2;240;240;240m";
        let slate = "\x1b[38;2;107;122;153m";

        println!(
            "{}     ██████╗ ██████╗ ███████╗███╗   ██╗{}███████╗",
            white, RED_ORANGE
        );
        println!(
            "{}    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║{}╚══███╔╝",
            white, RED_ORANGE
        );
        println!(
            "{}    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║{}  ███╔╝",
            white, RED_ORANGE
        );
        println!(
            "{}    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║{} ███╔╝",
            white, RED_ORANGE
        );
        println!(
            "{}    ╚██████╔╝██║     ███████╗██║ ╚████║{}███████╗",
            white, RED_ORANGE
        );
        println!(
            "{}     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝{}╚══════╝\r",
            white, RED_ORANGE
        );

        println!(
            "{}openz v{}{}",
            COLOR_BOLD,
            env!("CARGO_PKG_VERSION"),
            COLOR_RESET
        );
        {
            let defaults = self.defaults.lock().await;
            println!(
                "{}{}{}",
                slate,
                format!("{} | {}", defaults.provider, defaults.model),
                COLOR_RESET
            );
        }

        if let Ok(current_dir) = std::env::current_dir() {
            let path_str = if let Some(home) = dirs::home_dir() {
                if current_dir == home {
                    "~".to_string()
                } else if let Ok(stripped) = current_dir.strip_prefix(&home) {
                    format!("~/{}", stripped.display())
                } else {
                    current_dir.display().to_string()
                }
            } else {
                current_dir.display().to_string()
            };
            println!("{}{}{}", slate, path_str, COLOR_RESET);
        }

        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );

        let session_manager = {
            let agent_loop = self.agent_loop.lock().await;
            agent_loop.session_manager.clone()
        };
        if let Ok(session) = session_manager.load(&session_key) {
            render::print_session_history(&session);
        }

        loop {
            let (model, provider, session_manager) = {
                let defaults = self.defaults.lock().await;
                let agent_loop = self.agent_loop.lock().await;
                (
                    defaults.model.clone(),
                    defaults.provider.clone(),
                    agent_loop.session_manager.clone(),
                )
            };

            let (input, remote_sender) =
                match input::read_line_raw(&model, &provider, &session_manager, &session_key) {
                    Ok(inp) => inp,
                    Err(e) => {
                        eprintln!("Error reading input: {}", e);
                        continue;
                    }
                };
            let trimmed = input.trim();

            if trimmed.is_empty() {
                continue;
            }

            if trimmed == "/exit" || trimmed == "exit" || trimmed == "quit" {
                println!("Goodbye!");
                break;
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
                continue;
            }

            if trimmed == "/help" {
                println!("{}Available commands:{}", COLOR_BOLD, COLOR_RESET);
                for &(cmd, desc) in render::SLASH_COMMANDS {
                    println!("  {}{:<12}{} - {}", RED_ORANGE, cmd, COLOR_RESET, desc);
                }
                println!(
                    "{}────────────────────────────────────────────────────────────{}",
                    LIGHT_WHITE, COLOR_RESET
                );
                continue;
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
                            continue;
                        }
                    };

                    struct ProviderModels {
                        name: &'static str,
                        display: &'static str,
                        models: &'static [&'static str],
                    }

                    let provider_list = &[
                        ProviderModels {
                            name: "openai",
                            display: "OpenAI (8)",
                            models: &[
                                "gpt-4.5",
                                "gpt-4o",
                                "gpt-4o-mini",
                                "o1",
                                "o1-mini",
                                "o3",
                                "o3-mini",
                                "o4-mini",
                            ],
                        },
                        ProviderModels {
                            name: "anthropic",
                            display: "Anthropic (5)",
                            models: &[
                                "claude-3-5-sonnet-20241022",
                                "claude-3-5-sonnet",
                                "claude-3-5-haiku-20241022",
                                "claude-3-5-haiku",
                                "claude-3-opus-20240229",
                                "claude-3-opus",
                            ],
                        },
                        ProviderModels {
                            name: "openrouter",
                            display: "OpenRouter (5)",
                            models: &[
                                "google/gemini-2.5-pro",
                                "google/gemini-2.5-flash",
                                "anthropic/claude-3.5-sonnet",
                                "meta-llama/llama-3.3-70b-instruct",
                                "deepseek/deepseek-r1",
                            ],
                        },
                        ProviderModels {
                            name: "deepseek",
                            display: "DeepSeek (2)",
                            models: &["deepseek-chat", "deepseek-reasoner"],
                        },
                        ProviderModels {
                            name: "groq",
                            display: "Groq (5)",
                            models: &[
                                "deepseek-r1-distill-llama-70b",
                                "llama-3.3-70b-versatile",
                                "llama-3.1-8b-instant",
                                "mixtral-8x7b-32768",
                                "gemma2-9b-it",
                            ],
                        },
                        ProviderModels {
                            name: "ollama_local",
                            display: "Ollama Local (Auto-Start)",
                            models: &["llama3", "mistral", "phi3", "qwen2.5", "deepseek-r1"],
                        },
                        ProviderModels {
                            name: "ollama",
                            display: "Ollama (5)",
                            models: &["llama3", "mistral", "phi3", "qwen2.5", "deepseek-r1"],
                        },
                        ProviderModels {
                            name: "minimax",
                            display: "minimax.io (6)",
                            models: &[
                                "MiniMax-M3",
                                "MiniMax-M2.7",
                                "MiniMax-M2.5",
                                "MiniMax-M2.1",
                                "MiniMax-M2",
                                "MiniMax-M1",
                            ],
                        },
                        ProviderModels {
                            name: "mistral",
                            display: "Mistral AI (7)",
                            models: &[
                                "mistral-large-latest",
                                "pixtral-large-latest",
                                "mistral-moderation-latest",
                                "codestral-latest",
                                "mistral-small-latest",
                                "ministral-8b-latest",
                                "ministral-14b-latest",
                            ],
                        },
                        ProviderModels {
                            name: "z.ai",
                            display: "z.ai (Zhipu GLM) (6)",
                            models: &[
                                "glm-5.1",
                                "glm-5",
                                "glm-5v-turbo",
                                "glm-4.7",
                                "glm-4.7-flash",
                                "glm-4-flash",
                            ],
                        },
                        ProviderModels {
                            name: "nvidia",
                            display: "NVIDIA NIM (5)",
                            models: &[
                                "meta/llama3-70b-instruct",
                                "nvidia/llama-3.1-nemotron-70b-instruct",
                                "meta/llama-3.1-70b-instruct",
                                "mistralai/mixtral-8x22b-instruct-v0.1",
                                "google/gemma-2-27b-it",
                            ],
                        },
                        ProviderModels {
                            name: "opencode_zen",
                            display: "OpenCode Zen (4)",
                            models: &[
                                "deepseek-v4-flash-free",
                                "mimo-v2.5-free",
                                "north-mini-code-free",
                                "nemotron-3-ultra-free",
                            ],
                        },
                        ProviderModels {
                            name: "cerebras",
                            display: "Cerebras (3)",
                            models: &["llama-3.3-70b", "llama3.1-8b", "llama3.1-70b"],
                        },
                        ProviderModels {
                            name: "google_ai_studio",
                            display: "Google AI Studio (Gemini) (7)",
                            models: &[
                                "gemini-3.5-flash",
                                "gemini-3.1-pro-preview",
                                "gemini-3.1-flash-lite",
                                "gemini-2.5-pro",
                                "gemini-2.5-flash",
                                "gemini-2.0-flash",
                                "gemini-1.5-pro",
                            ],
                        },
                        ProviderModels {
                            name: "cohere",
                            display: "Cohere (5)",
                            models: &[
                                "command-a-plus-05-2026",
                                "command-r7b-12-2024",
                                "command-r7-12-2025",
                                "command-r-plus-08-2024",
                                "command-r-08-2024",
                            ],
                        },
                        ProviderModels {
                            name: "llm7",
                            display: "LLM7 (3)",
                            models: &["gpt-4o", "gpt-4o-mini", "claude-3-5-sonnet"],
                        },
                        ProviderModels {
                            name: "sambanova",
                            display: "SambaNova (5)",
                            models: &[
                                "DeepSeek-V3.2",
                                "Meta-Llama-3.3-70B-Instruct",
                                "Qwen2.5-72B-Instruct",
                                "QwQ-32B",
                                "gemma-4-31B-it",
                            ],
                        },
                        ProviderModels {
                            name: "huggingface",
                            display: "Hugging Face Inference (3)",
                            models: &[
                                "meta-llama/Llama-3.3-70B-Instruct",
                                "Qwen/QwQ-32B",
                                "deepseek-ai/DeepSeek-R1",
                            ],
                        },
                    ];

                    let filtered_providers: Vec<&ProviderModels> = provider_list
                        .iter()
                        .filter(|p| config.is_provider_configured(p.name))
                        .collect();

                    if filtered_providers.is_empty() {
                        println!(
                            "{}⚠️ No LLM providers configured! Please run 'openz configure' first.{}",
                            crate::agent::style::colors::AURA_GOLD,
                            crate::agent::style::colors::COLOR_RESET
                        );
                        println!(
                            "{}────────────────────────────────────────────────────────────{}",
                            LIGHT_WHITE, COLOR_RESET
                        );
                        continue;
                    }

                    let mut provider_options: Vec<String> = filtered_providers
                        .iter()
                        .map(|p| p.display.to_string())
                        .collect();
                    provider_options.push("Exit".to_string());
                    let (active_mdl, current_active_header) = {
                        let defaults = self.defaults.lock().await;
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
                            if selected_idx == filtered_providers.len() {
                                println!("Model selection cancelled.");
                                println!(
                                    "{}────────────────────────────────────────────────────────────{}",
                                    LIGHT_WHITE, COLOR_RESET
                                );
                                continue;
                            }
                            let prov_info = filtered_providers[selected_idx];
                            if prov_info.name == "ollama_local" {
                                crate::providers::ollama_manager::ensure_local_ollama(&config);
                            }

                            print!(
                                "{}◇ Fetching models for {}...{}",
                                AURA_SLATE, prov_info.display, COLOR_RESET
                            );
                            let _ = std::io::stdout().flush();

                            let mut model_options = match crate::channels::fetch_provider_models(
                                prov_info.name,
                                &config,
                            )
                            .await
                            {
                                Some(mut models) => {
                                    print!("\r\x1b[2K");
                                    let _ = std::io::stdout().flush();
                                    for &m in prov_info.models {
                                        let ms = m.to_string();
                                        if !models.contains(&ms) {
                                            models.push(ms);
                                        }
                                    }
                                    models.sort();
                                    models
                                }
                                None => {
                                    print!("\r\x1b[2K");
                                    let _ = std::io::stdout().flush();
                                    prov_info.models.iter().map(|&m| m.to_string()).collect()
                                }
                            };
                            model_options.push("Type manually (Custom Model)".to_string());
                            model_options.push("Exit".to_string());

                            match select_menu_custom(
                                &format!("Choose a model from {}:", prov_info.display),
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
                                        continue;
                                    }
                                    let prov = prov_info.name;
                                    let mdl = if selected_model_idx == model_options.len() - 2 {
                                        match inquire::Text::new("Enter custom model name:")
                                            .prompt()
                                        {
                                            Ok(custom) => {
                                                if custom.trim().is_empty() {
                                                    println!("Model selection cancelled.");
                                                    println!(
                                                        "{}────────────────────────────────────────────────────────────{}",
                                                        LIGHT_WHITE, COLOR_RESET
                                                    );
                                                    continue;
                                                }
                                                custom.trim().to_string()
                                            }
                                            Err(_) => {
                                                println!("Model selection cancelled.");
                                                println!(
                                                    "{}────────────────────────────────────────────────────────────{}",
                                                    LIGHT_WHITE, COLOR_RESET
                                                );
                                                continue;
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
                                                match crate::providers::resolver::resolve_provider_full(&config, &mdl) {
                                                    Ok(resolved) => {
                                                        let mut loop_lock = self.agent_loop.lock().await;
                                                        loop_lock.update_model_and_provider(config.clone(), resolved.instance);
                                                        let new_defaults = config.agents.defaults.clone();
                                                        if let Ok(mut guard) = CUSTOM_CONTEXT_LIMIT.lock() {
                                                            *guard = new_defaults.context_limit;
                                                        }
                                                        *self.defaults.lock().await = new_defaults;
                                                        println!("{}✓ Model updated to {} (provider: {}){}", EMERALD_GREEN, mdl, prov, COLOR_RESET);
                                                    }
                                                    Err(e) => {
                                                        eprintln!("{}✕ Error: Failed to initialize new model: {}{}", ERROR_RED, e, COLOR_RESET);
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
                                match crate::providers::resolver::resolve_provider_full(
                                    &config, mdl,
                                ) {
                                    Ok(resolved) => {
                                        let mut loop_lock = self.agent_loop.lock().await;
                                        loop_lock.update_model_and_provider(
                                            config.clone(),
                                            resolved.instance,
                                        );
                                        let new_defaults = config.agents.defaults.clone();
                                        if let Ok(mut guard) = CUSTOM_CONTEXT_LIMIT.lock() {
                                            *guard = new_defaults.context_limit;
                                        }
                                        *self.defaults.lock().await = new_defaults;
                                        println!(
                                            "{}✓ Model updated to {} (provider: {}){}",
                                            EMERALD_GREEN, mdl, prov, COLOR_RESET
                                        );
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
                continue;
            }

            if trimmed == "/new" {
                let session_manager = {
                    let agent_loop = self.agent_loop.lock().await;
                    agent_loop.session_manager.clone()
                };
                if let Ok(mut current_session) = session_manager.load(&session_key) {
                    if !current_session.messages.is_empty() {
                        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                        let archive_key = format!("cli:history_{}", timestamp);
                        current_session.key = archive_key;
                        let _ = session_manager.save(&current_session).await;

                        let empty_session = crate::session::Session::new(&session_key);
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
                continue;
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
                continue;
            }

            if trimmed == "/mcps" {
                let agent_loop = self.agent_loop.lock().await;
                println!("{}Configured MCP Servers:{}", COLOR_BOLD, COLOR_RESET);
                if agent_loop.config.mcp_servers.is_empty() {
                    println!("  No MCP servers configured.");
                } else {
                    for (name, mcp_cfg) in &agent_loop.config.mcp_servers {
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
                continue;
            }

            if trimmed == "/history" {
                let session_manager = {
                    let agent_loop = self.agent_loop.lock().await;
                    agent_loop.session_manager.clone()
                };
                match crate::cli::load_session_history() {
                    Ok(history) => {
                        if history.is_empty() {
                            println!("No session history found.");
                        } else {
                            match select_menu_with_history("Select a session to load:", &history) {
                                Ok(selected) => {
                                    if selected == 0 {
                                        let _ = crate::cli::archive_current_session(
                                            &session_manager,
                                            &session_key,
                                        )
                                        .await;
                                        println!(
                                            "{}✓ Started new session.{}",
                                            EMERALD_GREEN, COLOR_RESET
                                        );
                                    } else {
                                        let selected_item = &history[selected - 1];
                                        if selected_item.key != session_key {
                                            let _ = crate::cli::archive_current_session(
                                                &session_manager,
                                                &session_key,
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
                                                render::print_session_history(&session);
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
                continue;
            }

            if trimmed == "/memory" {
                let session_manager = {
                    let agent_loop = self.agent_loop.lock().await;
                    agent_loop.session_manager.clone()
                };
                if let Ok(session) = session_manager.load(&session_key) {
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
                continue;
            }

            if trimmed == "/paste" || trimmed == "/clip" {
                match input::handle_clipboard_paste(0) {
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

                        let agent_loop = self.agent_loop.lock().await;
                        match agent_loop.run(&combined_query, &session_key).await {
                            Ok(res) => {
                                println!();
                                render::print_colored_markdown(&res.content);
                                println!();
                                println!(
                                    "{}────────────────────────────────────────────────────────────{}",
                                    LIGHT_WHITE, COLOR_RESET
                                );
                            }
                            Err(e) => {
                                eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET);
                                println!(
                                    "{}────────────────────────────────────────────────────────────{}",
                                    LIGHT_WHITE, COLOR_RESET
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "{}✕ Error: Failed to retrieve image from clipboard: {}{}",
                            ERROR_RED, e, COLOR_RESET
                        );
                    }
                }
                continue;
            }

            let runner = self.agent_loop.lock().await;

            let run_res = {
                let _raw_mode = RawModeGuard::new().ok();
                let run_fut: std::pin::Pin<
                    Box<
                        dyn std::future::Future<Output = anyhow::Result<crate::agent::RunResult>>
                            + Send,
                    >,
                > = if let Some(ref sender) = remote_sender {
                    Box::pin(
                        crate::agent::style::spinner::CURRENT_SESSION_KEY
                            .scope(sender.clone(), runner.run(trimmed, &session_key)),
                    )
                } else {
                    Box::pin(runner.run(trimmed, &session_key))
                };

                let tx = crate::shutdown::cli_cancel_tx();
                let mut rx = tx.subscribe();
                let initial_val = *rx.borrow();
                let cancel_fut = async move {
                    while *rx.borrow() == initial_val {
                        if rx.changed().await.is_err() {
                            break;
                        }
                    }
                };

                // Keep raw mode active while a turn is running so Esc/Ctrl+C are delivered
                // as key events to the cancellation watcher instead of being line-buffered.
                let _run_raw_guard = RawModeGuard::new().ok();

                // Use a dedicated OS thread (not a tokio task!) for keyboard polling.
                // crossterm::event::poll/read are blocking calls that would starve
                // the tokio runtime if run inside tokio::task::spawn.
                let keyboard_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let keyboard_done_for_thread = keyboard_done.clone();
                let _keyboard_thread = std::thread::spawn(move || {
                    loop {
                        // Check if the main task told us to stop. The thread is detached below,
                        // so cancellation never waits on a possibly-blocking terminal read.
                        if keyboard_done_for_thread.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }
                        if let Ok(true) =
                            crossterm::event::poll(std::time::Duration::from_millis(100))
                        {
                            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read()
                            {
                                if key.kind == crossterm::event::KeyEventKind::Press {
                                    if key.code == crossterm::event::KeyCode::Esc
                                        || (key.code == crossterm::event::KeyCode::Char('c')
                                            && key
                                                .modifiers
                                                .contains(crossterm::event::KeyModifiers::CONTROL))
                                    {
                                        crate::shutdown::trigger_cli_cancel();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });

                tokio::pin!(run_fut);
                tokio::pin!(cancel_fut);

                let result = tokio::select! {
                    res = &mut run_fut => {
                        Some(res)
                    }
                    _ = &mut cancel_fut => {
                        crate::shutdown::trigger_cli_cancel();
                        crate::tui_println!("\r\n{}▲ Execution cancelled by user (Ctrl+C / Esc).{}", AURA_GOLD, COLOR_RESET);
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            &mut run_fut,
                        )
                        .await;
                        None
                    }
                };

                // Signal the keyboard thread to stop. Do not join: crossterm::event::read()
                // can block after a poll/read race, and joining here makes Esc/Ctrl+C look hung.
                keyboard_done.store(true, std::sync::atomic::Ordering::SeqCst);

                result
            };

            if let Some(ref sender) = remote_sender {
                if sender.starts_with("telegram:") {
                    if let Some(chat_id_str) = sender.strip_prefix("telegram:") {
                        if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                            crate::channels::telegram::stop_typing_indicator(chat_id);
                        }
                    }
                }
            }

            match run_res {
                Some(Ok(res)) => {
                    if !res.streamed {
                        println!();
                        render::print_colored_markdown(&res.content);
                    }
                    println!();
                    println!(
                        "{}────────────────────────────────────────────────────────────{}",
                        LIGHT_WHITE, COLOR_RESET
                    );

                    if let Some(ref sender) = remote_sender {
                        if sender.starts_with("telegram:") {
                            if let Some(chat_id_str) = sender.strip_prefix("telegram:") {
                                if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                                    if let Some((bot_token, client)) =
                                        crate::channels::telegram::get_telegram_bot_info()
                                    {
                                        let send_url = format!(
                                            "https://api.telegram.org/bot{}/sendMessage",
                                            bot_token
                                        );
                                        let payload = serde_json::json!({
                                            "chat_id": chat_id,
                                            "text": format!("🔌 [Remote Control Output]\n{}", res.content)
                                        });
                                        let _ = client.post(&send_url).json(&payload).send().await;
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Err(e)) => {
                    eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET);
                    println!(
                        "{}────────────────────────────────────────────────────────────{}",
                        LIGHT_WHITE, COLOR_RESET
                    );
                }
                None => {
                    println!(
                        "\r\n{}✕ Conversation interrupted by user.{}",
                        ERROR_RED, COLOR_RESET
                    );
                    println!(
                        "{}────────────────────────────────────────────────────────────{}",
                        LIGHT_WHITE, COLOR_RESET
                    );
                }
            }
        }

        Ok(())
    }
}
