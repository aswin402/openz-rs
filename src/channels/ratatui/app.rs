use std::sync::atomic::AtomicBool;

pub static IS_RATATUI_ACTIVE: AtomicBool = AtomicBool::new(false);

pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub is_tool: bool,
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
    pub should_exit: bool,
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
            should_exit: false,
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

