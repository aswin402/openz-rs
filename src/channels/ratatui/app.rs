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
    ChoosingModel {
        provider_name: String,
        provider_display: String,
        models: Vec<String>,
        selected_idx: usize,
    },
}

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
            
            // Standard provider lists with their display names and models
            let provider_list = &[
                ("mivi", "Mivi Local", vec!["mivi"]),
                ("openai", "OpenAI", vec!["gpt-4.5", "gpt-4o", "gpt-4o-mini", "o1", "o1-mini"]),
                ("anthropic", "Anthropic", vec!["claude-3-5-sonnet", "claude-3-5-haiku", "claude-3-opus"]),
                ("deepseek", "DeepSeek", vec!["deepseek-chat", "deepseek-reasoner"]),
                ("google_ai_studio", "Google AI Studio", vec!["gemini-3.5-flash", "gemini-2.5-pro", "gemini-2.5-flash"]),
                ("opencode_zen", "OpenCode Zen", vec!["deepseek-v4-flash-free", "mimo-v2.5-free", "north-mini-code-free"]),
                ("groq", "Groq", vec!["deepseek-r1-distill-llama-70b", "llama-3.3-70b-versatile"]),
                ("ollama", "Ollama Local", vec!["llama3", "mistral", "qwen2.5", "deepseek-r1"]),
            ];

            // If input is exactly "/model" or starts with "/model " (no provider specified yet)
            if arg.is_empty() {
                // List configured/available providers
                if let Ok(config) = crate::config::loader::load_config() {
                    for (name, display, _) in provider_list {
                        if config.is_provider_configured(name) {
                            results.push((
                                format!("/model {}", name),
                                format!("Select provider: {}", display),
                            ));
                        }
                    }
                }
                // If config load failed or list is empty, return static list of popular ones
                if results.is_empty() {
                    for (name, display, _) in provider_list {
                        results.push((
                            format!("/model {}", name),
                            format!("Select provider: {}", display),
                        ));
                    }
                }
            } else {
                // Provider is specified. Check if there's a space after provider
                // e.g. "/model openai" (no space at end) vs "/model openai " (space at end)
                let parts: Vec<&str> = arg.split_whitespace().collect();
                if parts.len() == 1 {
                    let prov_query = parts[0];
                    // Filter matching providers
                    for (name, display, _) in provider_list {
                        if name.starts_with(prov_query) {
                            results.push((
                                format!("/model {}", name),
                                format!("Select provider: {}", display),
                            ));
                        }
                    }
                    
                    // If the user typed a complete provider name but no trailing space,
                    // also show option to press space to list models
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
                    
                    // Find models for this provider
                    if let Some((_, _, models)) = provider_list.iter().find(|(name, _, _)| name == &provider) {
                        for model in models {
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

