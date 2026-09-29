use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use super::theme::Theme;

pub static IS_RATATUI_ACTIVE: AtomicBool = AtomicBool::new(false);

type BranchCache = HashMap<PathBuf, (Instant, Option<String>)>;
static BRANCH_CACHE: LazyLock<Mutex<BranchCache>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static IS_FETCHING_GIT: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub is_tool: bool,
    /// Name of the tool if this is a tool call message
    pub tool_name: Option<String>,
    /// Tool arguments/details string
    pub tool_details: Option<String>,
    /// Tool outcome summary (e.g., "45 lines", "✓ all tests passing")
    pub tool_summary: Option<String>,
    /// Reasoning/thinking content from the LLM
    pub reasoning: Option<String>,
    /// Time spent thinking (in seconds)
    pub thinking_time: Option<f64>,
    /// Tool execution success status
    pub tool_success: Option<bool>,
    /// Tool execution duration in milliseconds
    pub tool_duration_ms: Option<u64>,
    /// UI-only message (cancellations, notices) — survives session re-syncs
    pub ephemeral: bool,
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
            tool_summary: None,
            reasoning: None,
            thinking_time: None,
            tool_success: None,
            tool_duration_ms: None,
            ephemeral: false,
        }
    }

    /// Ephemeral UI-only notice — kept across session re-syncs, never persisted
    pub fn notice(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
            is_tool: false,
            tool_name: None,
            tool_details: None,
            tool_summary: None,
            reasoning: None,
            thinking_time: None,
            tool_success: None,
            tool_duration_ms: None,
            ephemeral: true,
        }
    }

    /// Construct ChatMessage from a persisted session message
    pub fn from_session_message(msg: &crate::session::Message) -> Self {
        let reasoning = msg
            .extra
            .get("reasoning_content")
            .or_else(|| msg.extra.get("thought"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let thinking_time = msg
            .extra
            .get("thinking_time_secs")
            .or_else(|| msg.extra.get("thinking_time"))
            .and_then(|v| v.as_f64());
        let tool_name = msg
            .extra
            .get("tool_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let tool_details = msg
            .extra
            .get("tool_details")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let tool_success = msg.extra.get("tool_success").and_then(|v| v.as_bool());

        Self {
            role: msg.role.clone(),
            content: msg.content.clone(),
            is_tool: msg.role == "tool",
            tool_name,
            tool_details,
            tool_summary: None,
            reasoning,
            thinking_time,
            tool_success,
            tool_duration_ms: None,
            ephemeral: false,
        }
    }

    /// Converts a slice of persisted session messages, resolving tool calls and formatting summaries.
    pub fn from_session_messages(messages: &[crate::session::Message]) -> Vec<Self> {
        let mut tool_calls_map = std::collections::HashMap::new();
        for msg in messages {
            if let Some(tool_calls) = msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tool_calls {
                    if let (Some(id), Some(name)) = (
                        tc.get("id").and_then(|v| v.as_str()),
                        tc.get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str()),
                    ) {
                        let args = tc
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        let parsed_args = if let Some(s) = args.as_str() {
                            serde_json::from_str(s).unwrap_or(serde_json::Value::Null)
                        } else {
                            args
                        };
                        tool_calls_map.insert(id.to_string(), (name.to_string(), parsed_args));
                    }
                }
            }
        }

        let mut result = Vec::with_capacity(messages.len());
        for msg in messages {
            if msg.role == "tool" {
                let tool_call_id = msg
                    .extra
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if let Some((raw_name, args)) = tool_calls_map.get(tool_call_id) {
                    let formatted_args = crate::agent::agent_loop::tool_execution::format_tool_args(
                        raw_name,
                        args,
                    );
                    let clean_name = crate::agent::style::get_tool_clean_name(raw_name);
                    let details = crate::agent::style::clean_tool_args_msg(raw_name, &formatted_args);

                    let outcome_val: serde_json::Value = serde_json::from_str(&msg.content)
                        .unwrap_or_else(|_| {
                            serde_json::json!({
                                "status": "success",
                                "output": &msg.content
                            })
                        });

                    let summary = crate::agent::style::format_tool_outcome_summary(
                        raw_name,
                        args,
                        &outcome_val,
                    );

                    let success = outcome_val.get("error").is_none()
                        && outcome_val.get("status_code").is_none_or(|c| c == 0)
                        && !summary.contains("Failed")
                        && !summary.contains('\u{2715}');

                    result.push(ChatMessage {
                        role: "tool".to_string(),
                        content: msg.content.clone(),
                        is_tool: true,
                        tool_name: Some(clean_name),
                        tool_details: if details.is_empty() { None } else { Some(details) },
                        tool_summary: Some(summary),
                        reasoning: None,
                        thinking_time: None,
                        tool_success: Some(success),
                        tool_duration_ms: None,
                        ephemeral: false,
                    });
                } else {
                    result.push(ChatMessage::from_session_message(msg));
                }
            } else {
                result.push(ChatMessage::from_session_message(msg));
            }
        }
        result
    }

    pub fn tool_start(name: String, details: String) -> Self {
        Self {
            role: "tool".to_string(),
            content: String::new(),
            is_tool: true,
            tool_name: Some(name),
            tool_details: Some(details),
            tool_summary: None,
            reasoning: None,
            thinking_time: None,
            tool_success: None,
            tool_duration_ms: None,
            ephemeral: false,
        }
    }

    pub fn tool_finished(
        name: String,
        details: String,
        output: String,
        success: bool,
        duration_ms: u64,
    ) -> Self {
        Self {
            role: "tool".to_string(),
            content: output,
            is_tool: true,
            tool_name: Some(name),
            tool_details: Some(details),
            tool_summary: None,
            reasoning: None,
            thinking_time: None,
            tool_success: Some(success),
            tool_duration_ms: Some(duration_ms),
            ephemeral: false,
        }
    }
}

#[derive(Clone)]
pub enum ModalState {
    None,
    ExitConfirm {
        selected_yes: bool,
    },
    CommandCatalog {
        filtered_indices: Vec<usize>,
        selected_index: usize,
        filter: String,
    },
    ProviderSelect {
        providers: Vec<(String, String)>, // (name, display_name)
        selected_idx: usize,
    },
    ModelSelect {
        provider_name: String,
        provider_display: String,
        models: Vec<String>,
        filtered_indices: Vec<usize>,
        selected_idx: usize,
        filter: String,
        loading: bool,
    },
    Help,
    History {
        sessions: Vec<(String, String, String)>, // (key, title, timestamp)
        selected_idx: usize,
    },
    SecurityApproval {
        tool_name: String,
        description: String,
        options: Vec<String>,
        selected_idx: usize,
        session_key: String,
        tx: std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<bool>>>>,
    },
}

impl std::fmt::Debug for ModalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModalState::None => write!(f, "ModalState::None"),
            ModalState::ExitConfirm { selected_yes } => {
                write!(f, "ModalState::ExitConfirm({selected_yes})")
            }
            ModalState::CommandCatalog { selected_index, .. } => {
                write!(f, "ModalState::CommandCatalog({selected_index})")
            }
            ModalState::ProviderSelect { selected_idx, .. } => {
                write!(f, "ModalState::ProviderSelect({selected_idx})")
            }
            ModalState::ModelSelect {
                provider_name,
                selected_idx,
                ..
            } => write!(f, "ModalState::ModelSelect({provider_name}, {selected_idx})"),
            ModalState::Help => write!(f, "ModalState::Help"),
            ModalState::History { selected_idx, .. } => {
                write!(f, "ModalState::History({selected_idx})")
            }
            ModalState::SecurityApproval {
                tool_name,
                selected_idx,
                ..
            } => write!(f, "ModalState::SecurityApproval({tool_name}, {selected_idx})"),
        }
    }
}

impl ModalState {
    pub fn is_active(&self) -> bool {
        !matches!(self, ModalState::None)
    }

    pub fn new_command_catalog() -> Self {
        let indices = (0..PALETTE_COMMANDS.len()).collect();
        Self::CommandCatalog {
            filtered_indices: indices,
            selected_index: 0,
            filter: String::new(),
        }
    }

    pub fn update_command_catalog_filter(&mut self) {
        if let ModalState::CommandCatalog {
            filtered_indices,
            selected_index,
            filter,
        } = self
        {
            let query = filter.trim().to_lowercase();
            *filtered_indices = PALETTE_COMMANDS
                .iter()
                .enumerate()
                .filter(|(_, cmd)| {
                    if query.is_empty() {
                        true
                    } else {
                        cmd.slash_name.to_lowercase().contains(&query)
                            || cmd.title.to_lowercase().contains(&query)
                            || cmd.description.to_lowercase().contains(&query)
                            || cmd.category.label().to_lowercase().contains(&query)
                    }
                })
                .map(|(i, _)| i)
                .collect();

            if *selected_index >= filtered_indices.len() {
                *selected_index = filtered_indices.len().saturating_sub(1);
            }
        }
    }

    pub fn update_model_filter(&mut self) {
        if let ModalState::ModelSelect {
            models,
            filtered_indices,
            selected_idx,
            filter,
            ..
        } = self
        {
            let query = filter.to_lowercase();
            *filtered_indices = models
                .iter()
                .enumerate()
                .filter(|(_, m)| {
                    if query.is_empty() {
                        true
                    } else {
                        m.to_lowercase().contains(&query)
                    }
                })
                .map(|(i, _)| i)
                .collect();

            if *selected_idx >= filtered_indices.len() {
                *selected_idx = filtered_indices.len().saturating_sub(1);
            }
        }
    }
}

/// Categories for organizing spotlight palette commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    All,
    System,
    Agent,
    Tools,
}

impl CommandCategory {
    pub fn all() -> &'static [CommandCategory] {
        &[
            CommandCategory::All,
            CommandCategory::System,
            CommandCategory::Agent,
            CommandCategory::Tools,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            CommandCategory::All => "All",
            CommandCategory::System => "System",
            CommandCategory::Agent => "Agent",
            CommandCategory::Tools => "Tools",
        }
    }
}

/// A command item displayed in the floating spotlight palette and catalog.
#[derive(Debug, Clone, Copy)]
pub struct PaletteCommand {
    pub slash_name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub category: CommandCategory,
    pub shortcut: Option<&'static str>,
    pub example: &'static str,
}

pub const PALETTE_COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        slash_name: "/model",
        title: "Switch Model & Provider",
        description: "Choose active LLM model or provider interactively",
        category: CommandCategory::Agent,
        shortcut: Some("Ctrl+L"),
        example: "/model | /model claude-3-5-sonnet | /model openai/gpt-4o",
    },
    PaletteCommand {
        slash_name: "/history",
        title: "Restore Chat Session",
        description: "Browse & reload past chat sessions",
        category: CommandCategory::System,
        shortcut: Some("Ctrl+H"),
        example: "/history | /history <session_key>",
    },
    PaletteCommand {
        slash_name: "/new-session",
        title: "New Conversation Session",
        description: "Start fresh session & reset conversation timeline",
        category: CommandCategory::System,
        shortcut: Some("Ctrl+N"),
        example: "/new-session",
    },
    PaletteCommand {
        slash_name: "/clear",
        title: "Clear Timeline",
        description: "Clear active conversation timeline messages",
        category: CommandCategory::System,
        shortcut: None,
        example: "/clear",
    },
    PaletteCommand {
        slash_name: "/commands",
        title: "Command & Capabilities Catalog",
        description: "Interactive catalog of all slash commands & keybindings",
        category: CommandCategory::System,
        shortcut: None,
        example: "/commands",
    },
    PaletteCommand {
        slash_name: "/help",
        title: "Help & Shortcuts Cheatsheet",
        description: "Interactive keyboard shortcuts cheatsheet",
        category: CommandCategory::System,
        shortcut: Some("F1"),
        example: "/help",
    },
    PaletteCommand {
        slash_name: "/settings",
        title: "Settings & Configuration",
        description: "View and adjust active configuration, security & sandbox",
        category: CommandCategory::System,
        shortcut: Some("F3"),
        example: "/settings",
    },
    PaletteCommand {
        slash_name: "/streaming",
        title: "Toggle Response Streaming",
        description: "Toggle live response token streaming on/off",
        category: CommandCategory::System,
        shortcut: None,
        example: "/streaming",
    },
    PaletteCommand {
        slash_name: "/memory",
        title: "Knowledge Graph & Memory",
        description: "Inspect cognitive knowledge graph, facts & entities",
        category: CommandCategory::Agent,
        shortcut: None,
        example: "/memory",
    },
    PaletteCommand {
        slash_name: "/skills",
        title: "Learned Autonomous Skills",
        description: "List and view active autonomous skills",
        category: CommandCategory::Agent,
        shortcut: None,
        example: "/skills",
    },
    PaletteCommand {
        slash_name: "/sop",
        title: "SOP Workflow Engine",
        description: "Inspect and trigger Standard Operating Procedure workflows",
        category: CommandCategory::Agent,
        shortcut: None,
        example: "/sop list | /sop simulate <name> | /sop resume <id>",
    },
    PaletteCommand {
        slash_name: "/workflows",
        title: "Multi-Agent Orchestrator",
        description: "Search and execute reusable multi-agent workflows",
        category: CommandCategory::Agent,
        shortcut: None,
        example: "/workflows",
    },
    PaletteCommand {
        slash_name: "/audit",
        title: "Session Merkle Ledger",
        description: "Cryptographically verify session SHA-256 Merkle chain",
        category: CommandCategory::Agent,
        shortcut: None,
        example: "/audit",
    },
    PaletteCommand {
        slash_name: "/mcps",
        title: "MCP Server Registry",
        description: "List configured MCP servers and active connection status",
        category: CommandCategory::Tools,
        shortcut: None,
        example: "/mcps",
    },
    PaletteCommand {
        slash_name: "/servers",
        title: "Background Dev Servers",
        description: "List OpenZ background dev servers & subprocesses",
        category: CommandCategory::Tools,
        shortcut: None,
        example: "/servers",
    },
    PaletteCommand {
        slash_name: "/stop-server",
        title: "Stop Server Process",
        description: "Stop background server instance by PID or all",
        category: CommandCategory::Tools,
        shortcut: None,
        example: "/stop-server <id|all>",
    },
    PaletteCommand {
        slash_name: "/logs",
        title: "Structured Color Logs",
        description: "Stream real-time color-coded structured logs",
        category: CommandCategory::Tools,
        shortcut: None,
        example: "/logs | /logs --tail 50 --level debug",
    },
    PaletteCommand {
        slash_name: "/device",
        title: "Device & App Inventory",
        description: "Manage local hardware and application inventory",
        category: CommandCategory::Tools,
        shortcut: None,
        example: "/device",
    },
    PaletteCommand {
        slash_name: "/sources",
        title: "Saved Research Sources",
        description: "Search and inspect saved knowledge source bookmarks",
        category: CommandCategory::Tools,
        shortcut: None,
        example: "/sources",
    },
    PaletteCommand {
        slash_name: "/exit",
        title: "Exit OpenZ Session",
        description: "Quit OpenZ interactive terminal session cleanly",
        category: CommandCategory::System,
        shortcut: Some("Ctrl+C"),
        example: "/exit | quit | Ctrl+C",
    },
];

// Re-export shared provider data from channels/mod.rs — single source of truth
pub use crate::channels::{build_configured_providers, curated_models_for, PROVIDER_REGISTRY};

pub struct RatatuiApp {
    pub model: String,
    pub provider: String,
    pub session_key: String,
    pub workspace_root: PathBuf,
    pub typed_input: Vec<char>,
    pub cursor_idx: usize,
    pub messages: Vec<ChatMessage>,
    pub scroll_offset: u32,
    pub max_scroll: u32,
    pub auto_scroll: bool,
    pub selected_index: Option<usize>,
    pub approx_tokens: usize,
    pub limit_tokens: usize,
    pub cwd_display: String,
    pub prompt_history: Vec<String>,
    pub history_idx: Option<usize>,
    pub should_exit: bool,
    /// Whether the agent is currently processing
    pub is_thinking: bool,
    /// Time when current turn started
    pub work_start: Option<Instant>,
    /// Spinner frame index for animation
    pub spinner_idx: usize,
    /// Active theme
    pub theme: Theme,
    /// Active interactive modal overlay
    pub modal: ModalState,
    /// Active category index for spotlight slash palette
    pub slash_category_idx: usize,
    /// Selected index inside matching palette commands
    pub slash_selected_idx: usize,
    /// Queue of user prompts submitted while agent is actively executing
    pub queued_prompts: std::collections::VecDeque<String>,
}

pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    (
        "/model",
        "Switch active LLM provider and model interactively",
    ),
    ("/clear", "Clear conversation timeline"),
    ("/history", "Restore or switch chat sessions"),
    ("/new-session", "Start a clean conversation session"),
    ("/help", "Show help, keybindings and commands"),
    ("/commands", "Open interactive command and capability catalog"),
    ("/mcps", "List configured MCP servers and active status"),
    ("/memory", "Inspect cognitive facts and knowledge graph"),
    ("/skills", "List active learned skills"),
    ("/sources", "Search saved knowledge source bookmarks"),
    ("/workflows", "Search and execute reusable SOP workflows"),
    ("/servers", "List OpenZ background server instances"),
    ("/stop-server", "Stop background server instance by ID or all"),
    ("/logs", "Stream real-time color-coded structured logs"),
    ("/settings", "View and adjust active configuration"),
    ("/streaming", "Toggle response streaming preference"),
    ("/device", "Manage local application and device inventory"),
    ("/audit", "Cryptographically verify session SHA-256 ledger"),
    ("/sop", "Inspect Standard Operating Procedure workflows"),
    ("/exit", "Quit OpenZ interactive terminal session"),
];

impl RatatuiApp {
    pub fn new(model: String, provider: String, session_key: String) -> Self {
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let cwd_str = workspace_root.to_string_lossy().to_string();
        let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
        let cwd_display = if let Some(ref h) = home {
            if cwd_str.starts_with(h) {
                cwd_str.replacen(h, "~", 1)
            } else {
                cwd_str
            }
        } else {
            cwd_str
        };

        Self {
            model,
            provider,
            session_key,
            workspace_root,
            typed_input: Vec::new(),
            cursor_idx: 0,
            messages: Vec::new(),
            scroll_offset: 0,
            max_scroll: 0,
            auto_scroll: true,
            selected_index: None,
            approx_tokens: 0,
            limit_tokens: 1_000_000,
            cwd_display,
            prompt_history: Vec::new(),
            history_idx: None,
            should_exit: false,
            is_thinking: false,
            work_start: None,
            spinner_idx: 0,
            theme: Theme::aura_dark(),
            modal: ModalState::None,
            slash_category_idx: 0,
            slash_selected_idx: 0,
            queued_prompts: std::collections::VecDeque::new(),
        }
    }

    pub fn queue_prompt(&mut self, prompt: String) {
        let trimmed = prompt.trim().to_string();
        if !trimmed.is_empty() {
            self.queued_prompts.push_back(trimmed);
        }
    }

    pub fn pop_next_prompt(&mut self) -> Option<String> {
        self.queued_prompts.pop_front()
    }

    pub fn scroll_up(&mut self, lines: u32) {
        if self.auto_scroll {
            self.auto_scroll = false;
            self.scroll_offset = self.max_scroll.saturating_sub(lines);
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        }
    }

    pub fn scroll_down(&mut self, lines: u32) {
        if !self.auto_scroll {
            let next = self.scroll_offset.saturating_add(lines);
            if next >= self.max_scroll {
                self.scroll_offset = self.max_scroll;
                self.auto_scroll = true;
            } else {
                self.scroll_offset = next;
            }
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.auto_scroll = false;
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll;
        self.auto_scroll = true;
    }

    /// Replace the timeline with the session's disk content while keeping the
    /// most recent ephemeral UI notices, then re-anchor to the bottom.
    pub fn apply_sync_session(&mut self, disk_msgs: Vec<ChatMessage>) {
        let mut kept: Vec<ChatMessage> = self
            .messages
            .iter()
            .filter(|m| m.ephemeral)
            .rev()
            .take(5)
            .cloned()
            .collect();
        kept.reverse();
        self.messages = disk_msgs;
        self.messages.append(&mut kept);
        self.update_approx_tokens();
        self.scroll_to_bottom();
    }

    /// Recalculates approximate token usage from all timeline messages
    pub fn update_approx_tokens(&mut self) {
        let total_chars: usize = self.messages.iter().map(|m| m.content.len()).sum();
        self.approx_tokens = total_chars / 4;
    }

    /// Retrieve current git branch with background cache refresh
    pub fn get_git_branch(workspace: &Path) -> Option<String> {
        let workspace_key = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.to_path_buf());
        let now = Instant::now();
        let mut cached_branch = None;
        let mut needs_refresh = true;

        if let Ok(guard) = BRANCH_CACHE.lock() {
            if let Some((last_check, branch)) = guard.get(&workspace_key) {
                cached_branch = branch.clone();
                if now.duration_since(*last_check) < Duration::from_secs(3) {
                    needs_refresh = false;
                }
            }
        }

        if needs_refresh && !IS_FETCHING_GIT.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let cache_key = workspace_key.clone();
            let ws = workspace_key;
            let fetcher = move || {
                let output = std::process::Command::new("git")
                    .arg("rev-parse")
                    .arg("--abbrev-ref")
                    .arg("HEAD")
                    .current_dir(&ws)
                    .output();

                let branch = output.ok().and_then(|out| {
                    if out.status.success() {
                        let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if !b.is_empty() {
                            Some(b)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                if let Ok(mut guard) = BRANCH_CACHE.lock() {
                    guard.insert(cache_key, (Instant::now(), branch));
                }
                IS_FETCHING_GIT.store(false, std::sync::atomic::Ordering::SeqCst);
            };

            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn_blocking(fetcher);
            } else {
                std::thread::spawn(fetcher);
            }
        }

        cached_branch
    }

    pub fn matching_slash_commands(&self) -> Vec<(String, String)> {
        let input_str: String = self.typed_input.iter().collect();
        if !input_str.starts_with('/') {
            return Vec::new();
        }

        SLASH_COMMANDS
            .iter()
            .copied()
            .filter(|(cmd, _)| cmd.starts_with(&input_str))
            .map(|(cmd, desc)| (cmd.to_string(), desc.to_string()))
            .collect()
    }

    /// Whether user is currently typing a slash command
    /// Checks whether the user is typing a slash command (starts with '/' and hasn't started typing arguments)
    pub fn has_active_slash_query(&self) -> bool {
        self.typed_input.first() == Some(&'/') && !self.typed_input.contains(&' ')
    }

    /// Extract query text without leading slash
    pub fn slash_search_text(&self) -> String {
        let input: String = self.typed_input.iter().collect();
        if let Some(stripped) = input.strip_prefix('/') {
            stripped.trim().to_string()
        } else {
            String::new()
        }
    }

    /// Matching palette commands based on active category and query
    pub fn matching_palette_commands(&self) -> Vec<usize> {
        let categories = CommandCategory::all();
        let current_cat = categories[self.slash_category_idx % categories.len()];
        let query = self.slash_search_text().to_lowercase();

        let mut matches = Vec::new();
        for (i, cmd) in PALETTE_COMMANDS.iter().enumerate() {
            if current_cat != CommandCategory::All && cmd.category != current_cat {
                continue;
            }
            if query.is_empty() {
                matches.push(i);
            } else {
                let name = cmd.slash_name.trim_start_matches('/').to_lowercase();
                let title = cmd.title.to_lowercase();
                let desc = cmd.description.to_lowercase();
                if name.starts_with(&query)
                    || title.starts_with(&query)
                    || name.contains(&query)
                    || title.contains(&query)
                    || desc.contains(&query)
                {
                    matches.push(i);
                }
            }
        }

        // Sort by match relevance: exact match first, prefix next, then substring
        if !query.is_empty() {
            matches.sort_by_key(|&idx| {
                let cmd = &PALETTE_COMMANDS[idx];
                let name = cmd.slash_name.trim_start_matches('/').to_lowercase();
                let title = cmd.title.to_lowercase();
                if name == query {
                    0
                } else if name.starts_with(&query) {
                    1
                } else if title.starts_with(&query) {
                    2
                } else if name.contains(&query) {
                    3
                } else if title.contains(&query) {
                    4
                } else {
                    5
                }
            });
        }

        matches
    }

    /// Returns the currently selected palette command
    pub fn selected_palette_command(&self) -> Option<&'static PaletteCommand> {
        let matches = self.matching_palette_commands();
        if matches.is_empty() {
            None
        } else {
            let idx = self.slash_selected_idx.min(matches.len().saturating_sub(1));
            Some(&PALETTE_COMMANDS[matches[idx]])
        }
    }

    /// Cycle category tabs in spotlight palette (Tab / BackTab)
    pub fn cycle_slash_category(&mut self, forward: bool) {
        let total = CommandCategory::all().len();
        if total == 0 {
            return;
        }
        if forward {
            self.slash_category_idx = (self.slash_category_idx + 1) % total;
        } else if self.slash_category_idx == 0 {
            self.slash_category_idx = total.saturating_sub(1);
        } else {
            self.slash_category_idx = self.slash_category_idx.saturating_sub(1);
        }
        self.slash_selected_idx = 0;
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;

