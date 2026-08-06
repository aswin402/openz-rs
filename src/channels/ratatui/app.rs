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
        }
    }

    pub fn matching_slash_commands(&self) -> Vec<(&'static str, &'static str)> {
        let input_str: String = self.typed_input.iter().collect();
        if input_str.starts_with('/') {
            SLASH_COMMANDS
                .iter()
                .copied()
                .filter(|(cmd, _)| cmd.starts_with(&input_str))
                .collect()
        } else {
            Vec::new()
        }
    }
}

