use std::sync::atomic::AtomicBool;

pub static IS_RATATUI_ACTIVE: AtomicBool = AtomicBool::new(false);

pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub is_tool: bool,
    /// Name of the tool if this is a tool call message
    pub tool_name: Option<String>,
    /// Tool arguments/details string
    pub tool_details: Option<String>,
    /// Reasoning/thinking content from the LLM
    pub reasoning: Option<String>,
    /// Time spent thinking (in seconds)
    pub thinking_time: Option<f64>,
}

impl ChatMessage {
    /// Quick constructor — sets role + content, everything else defaults
    pub fn simple(role: &str, content: String) -> Self {
        Self {
            role: role.to_string(),
            content,
            is_tool: role == "tool",
            tool_name: None,
            tool_details: None,
            reasoning: None,
            thinking_time: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ModelSelectState {
    Closed,
    ChoosingProvider {
        providers: Vec<(String, String)>, // (name, display_name)
        selected_idx: usize,
    },
    FetchingModels {
        provider_name: String,
        provider_display: String,
    },
    ChoosingModel {
        provider_name: String,
        provider_display: String,
        models: Vec<String>,
        selected_idx: usize,
    },
}

// Re-export shared provider data from channels/mod.rs — single source of truth
pub use crate::channels::{build_configured_providers, curated_models_for, PROVIDER_REGISTRY};

pub struct RatatuiApp {
    pub model: String,
    pub provider: String,
    pub session_key: String,
    pub typed_input: Vec<char>,
    pub cursor_idx: usize,
    pub messages: Vec<ChatMessage>,
    pub scroll_offset: usize,
    pub selected_index: Option<usize>,
    pub approx_tokens: usize,
    pub limit_tokens: usize,
    pub cwd_display: String,
    pub prompt_history: Vec<String>,
    pub history_idx: Option<usize>,
    pub should_exit: bool,
    /// Whether the agent is currently processing (shows spinner)
    pub is_thinking: bool,
    /// Spinner frame index for animation
    pub spinner_idx: usize,
    /// Active interactive model selection state
    pub model_select: ModelSelectState,
}

pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/clear", "Clear screen"),
    ("/device", "Manage local app/device inventory"),
    ("/exit", "Exit OpenZ"),
    ("/help", "List slash commands"),
    ("/history", "Restore/switch sessions using selection menu"),
    ("/logs", "Show recent logs"),
    ("/mcps", "List configured MCP servers"),
    ("/memory", "View metadata memory"),
    ("/model", "Show or change active default model"),
    ("/new-session", "Start a new session"),
    ("/servers", "List OpenZ-launched background servers"),
    ("/settings", "Show active settings"),
    ("/skill", "List active skills"),
    ("/sources", "Search saved source bookmarks"),
    ("/stop-server", "Stop a background server by id, or all"),
    ("/streaming", "Manage response streaming mode"),
    ("/tui", "Manage TUI display settings"),
    ("/workflows", "Search reusable workflows"),
];

impl RatatuiApp {
    pub fn new(model: String, provider: String, session_key: String) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "~".to_string());
        let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
        let cwd_display = if let Some(ref h) = home {
            if cwd.starts_with(h) {
                cwd.replacen(h, "~", 1)
            } else {
                cwd
            }
        } else {
            cwd
        };

        Self {
            model,
            provider,
            session_key,
            typed_input: Vec::new(),
            cursor_idx: 0,
            messages: Vec::new(),
            scroll_offset: 0,
            selected_index: None,
            approx_tokens: 0,
            limit_tokens: 1_000_000,
            cwd_display,
            prompt_history: Vec::new(),
            history_idx: None,
            should_exit: false,
            is_thinking: false,
            spinner_idx: 0,
            model_select: ModelSelectState::Closed,
        }
    }

    pub fn matching_slash_commands(&self) -> Vec<(String, String)> {
        let input_str: String = self.typed_input.iter().collect();
        if !input_str.starts_with('/') {
            return Vec::new();
        }

        // Check if they typed /model ...
        if input_str.starts_with("/model") {
            let mut results = Vec::new();
            let arg = input_str.strip_prefix("/model").unwrap_or("").trim_start();
            // Remove leading 's' from "/models" prefix
            let arg = if input_str.starts_with("/models") {
                input_str.strip_prefix("/models").unwrap_or("").trim_start()
            } else {
                arg
            };

            if arg.is_empty() {
                // List configured/available providers from real config
                if let Ok(config) = crate::config::loader::load_config() {
                    for reg in PROVIDER_REGISTRY {
                        if config.is_provider_configured(reg.name) {
                            results.push((
                                format!("/model {}", reg.name),
                                format!("Select provider: {}", reg.display),
                            ));
                        }
                    }
                    // Also include custom providers
                    for name in config.custom_provider_names() {
                        if config.is_provider_available(&name) {
                            results.push((
                                format!("/model {}", name),
                                format!("Select custom provider: {}", name),
                            ));
                        }
                    }
                }
                if results.is_empty() {
                    for reg in PROVIDER_REGISTRY {
                        results.push((
                            format!("/model {}", reg.name),
                            format!("Select provider: {}", reg.display),
                        ));
                    }
                }
            } else {
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.len() == 1 {
                    let prov_query = parts[0];
                    // Filter matching providers from registry
                    for reg in PROVIDER_REGISTRY {
                        if reg.name.starts_with(prov_query) {
                            results.push((
                                format!("/model {}", reg.name),
                                format!("Select provider: {}", reg.display),
                            ));
                        }
                    }
                    // Also match custom providers
                    if let Ok(config) = crate::config::loader::load_config() {
                        for name in config.custom_provider_names() {
                            if name.starts_with(prov_query) && config.is_provider_available(&name) {
                                results.push((
                                    format!("/model {}", name),
                                    format!("Select custom provider: {}", name),
                                ));
                            }
                        }
                    }

                    if results.len() == 1 && results[0].0 == input_str {
                        let name = results[0].0.strip_prefix("/model ").unwrap_or("");
                        results.push((
                            format!("/model {} ", name),
                            "Press Space to list models".to_string(),
                        ));
                    }
                } else if parts.len() >= 2 {
                    let provider = parts[0];
                    let model_query = if arg.ends_with(' ') { "" } else { parts[1] };

                    // Find models for this provider from registry
                    if let Some(reg) = PROVIDER_REGISTRY.iter().find(|r| r.name == provider) {
                        for model in reg.models {
                            if model_query.is_empty() || model.starts_with(model_query) {
                                results.push((
                                    format!("/model {}/{}", provider, model),
                                    format!("Select model: {}", model),
                                ));
                            }
                        }
                    }
                }
            }

            return results;
        }

        // Default: match standard static list of slash commands
        SLASH_COMMANDS
            .iter()
            .copied()
            .filter(|(cmd, _)| cmd.starts_with(&input_str))
            .map(|(cmd, desc)| (cmd.to_string(), desc.to_string()))
            .collect()
    }
}
