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

pub const ALL_THOUGHT_TAGS: &[&str] = &[
    "<think>",
    "</think>",
    "<thought>",
    "</thought>",
    "<thinking>",
    "</thinking>",
    "<reasoning>",
    "</reasoning>",
    "<antThinking>",
    "</antThinking>",
    "<Think>",
    "</Think>",
    "<Thought>",
    "</Thought>",
    "<Thinking>",
    "</Thinking>",
    "<Reasoning>",
    "</Reasoning>",
    "<THINK>",
    "</THINK>",
];

pub const OPEN_THOUGHT_TAGS: &[&str] = &[
    "<think>",
    "<thought>",
    "<thinking>",
    "<reasoning>",
    "<antThinking>",
    "<Think>",
    "<Thought>",
    "<Thinking>",
    "<Reasoning>",
    "<THINK>",
];

pub const CLOSE_THOUGHT_TAGS: &[&str] = &[
    "</think>",
    "</thought>",
    "</thinking>",
    "</reasoning>",
    "</antThinking>",
    "</Think>",
    "</Thought>",
    "</Thinking>",
    "</Reasoning>",
    "</THINK>",
];

/// Strips all XML thinking tags
pub fn sanitize_thought_text(raw: &str) -> String {
    let mut s = raw.to_string();
    for tag in ALL_THOUGHT_TAGS {
        s = s.replace(tag, "");
    }
    s.trim().to_string()
}

/// Strips any stray XML thinking tags that might leak into assistant text
pub fn sanitize_assistant_text(raw: &str) -> String {
    let mut s = raw.to_string();
    for tag in ALL_THOUGHT_TAGS {
        s = s.replace(tag, "");
    }
    s
}

/// Extracts any thoughts enclosed in <think>...</think>, <thought>...</thought>, etc.
/// Returns (Option<extracted_thought>, cleaned_content).
pub fn extract_thoughts_from_content(raw: &str) -> (Option<String>, String) {
    let mut thoughts = Vec::new();
    let mut cleaned_content = String::new();
    let mut remaining = raw;

    while !remaining.is_empty() {
        let earliest_open = OPEN_THOUGHT_TAGS
            .iter()
            .filter_map(|&tag| remaining.find(tag).map(|idx| (idx, tag.len())))
            .min_by_key(|&(idx, _)| idx);

        match earliest_open {
            Some((start_idx, open_len)) => {
                cleaned_content.push_str(&remaining[..start_idx]);
                let after_open = &remaining[start_idx + open_len..];

                let earliest_close = CLOSE_THOUGHT_TAGS
                    .iter()
                    .filter_map(|&tag| after_open.find(tag).map(|idx| (idx, tag.len())))
                    .min_by_key(|&(idx, _)| idx);

                match earliest_close {
                    Some((close_idx, close_len)) => {
                        let thought_text = &after_open[..close_idx];
                        let sanitized = sanitize_thought_text(thought_text);
                        match sanitized.is_empty() {
                            false => thoughts.push(sanitized),
                            true => {}
                        }
                        let rest = &after_open[close_idx + close_len..];
                        remaining = rest
                            .strip_prefix("\r\n")
                            .or_else(|| rest.strip_prefix('\n'))
                            .unwrap_or(rest);
                    }
                    None => {
                        let sanitized = sanitize_thought_text(after_open);
                        match sanitized.is_empty() {
                            false => thoughts.push(sanitized),
                            true => {}
                        }
                        remaining = "";
                    }
                }
            }
            None => {
                cleaned_content.push_str(remaining);
                break;
            }
        }
    }

    let final_cleaned = sanitize_assistant_text(&cleaned_content);
    let final_thought = match thoughts.is_empty() {
        true => None,
        false => Some(thoughts.join("\n\n")),
    };

    (final_thought, final_cleaned)
}

impl ChatMessage {
    /// Quick constructor — sets role + content, extracting embedded thoughts if assistant
    pub fn simple(role: &str, content: String) -> Self {
        let (extracted_reasoning, cleaned_content) = match role {
            "assistant" => extract_thoughts_from_content(&content),
            _ => (None, content),
        };

        Self {
            role: role.to_string(),
            content: cleaned_content,
            is_tool: role == "tool",
            tool_name: None,
            tool_details: None,
            tool_summary: None,
            reasoning: extracted_reasoning,
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

        let (extracted_thought, cleaned_content) = match msg.role.as_str() {
            "assistant" => extract_thoughts_from_content(&msg.content),
            _ => (None, msg.content.clone()),
        };
        let reasoning = reasoning.or(extracted_thought);

        Self {
            role: msg.role.clone(),
            content: cleaned_content,
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
            match msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
                Some(tool_calls) => {
                    for tc in tool_calls {
                        let id_opt = tc.get("id").and_then(|v| v.as_str());
                        let name_opt = tc
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str());
                        match (id_opt, name_opt) {
                            (Some(id), Some(name)) => {
                                let args = tc
                                    .get("function")
                                    .and_then(|f| f.get("arguments"))
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Null);
                                let parsed_args = match args.as_str() {
                                    Some(s) => {
                                        serde_json::from_str(s).unwrap_or(serde_json::Value::Null)
                                    }
                                    None => args,
                                };
                                tool_calls_map
                                    .insert(id.to_string(), (name.to_string(), parsed_args));
                            }
                            _ => {}
                        }
                    }
                }
                None => {}
            }
        }

        let mut result = Vec::with_capacity(messages.len());
        for msg in messages {
            match msg.role.as_str() {
                "tool" => {
                    let tool_call_id = msg
                        .extra
                        .get("tool_call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    match tool_calls_map.get(tool_call_id) {
                        Some((raw_name, args)) => {
                            let formatted_args =
                                crate::agent::agent_loop::tool_execution::format_tool_args(
                                    raw_name,
                                    args,
                                );
                            let clean_name = crate::agent::style::get_tool_clean_name(raw_name);
                            let details = crate::agent::style::clean_tool_args_msg(
                                raw_name,
                                &formatted_args,
                            );

                            let outcome_val: serde_json::Value =
                                serde_json::from_str(&msg.content).unwrap_or_else(|_| {
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

                            let opt_details = match details.is_empty() {
                                true => None,
                                false => Some(details),
                            };

                            result.push(ChatMessage {
                                role: "tool".to_string(),
                                content: msg.content.clone(),
                                is_tool: true,
                                tool_name: Some(clean_name),
                                tool_details: opt_details,
                                tool_summary: Some(summary),
                                reasoning: None,
                                thinking_time: None,
                                tool_success: Some(success),
                                tool_duration_ms: None,
                                ephemeral: false,
                            });
                        }
                        None => result.push(ChatMessage::from_session_message(msg)),
                    }
                }
                _ => result.push(ChatMessage::from_session_message(msg)),
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

    pub fn thought(reasoning: String, thinking_time: Option<f64>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: String::new(),
            is_tool: false,
            tool_name: None,
            tool_details: None,
            tool_summary: None,
            reasoning: Some(reasoning),
            thinking_time,
            tool_success: None,
            tool_duration_ms: None,
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
        match self {
            ModalState::CommandCatalog {
                filtered_indices,
                selected_index,
                filter,
            } => {
                let query = filter.trim().to_lowercase();
                *filtered_indices = PALETTE_COMMANDS
                    .iter()
                    .enumerate()
                    .filter(|(_, cmd)| match query.is_empty() {
                        true => true,
                        false => {
                            cmd.slash_name.to_lowercase().contains(&query)
                                || cmd.title.to_lowercase().contains(&query)
                                || cmd.description.to_lowercase().contains(&query)
                                || cmd.category.label().to_lowercase().contains(&query)
                        }
                    })
                    .map(|(i, _)| i)
                    .collect();

                *selected_index = (*selected_index).min(filtered_indices.len().saturating_sub(1));
            }
            _ => {}
        }
    }

    pub fn update_model_filter(&mut self) {
        match self {
            ModalState::ModelSelect {
                models,
                filtered_indices,
                selected_idx,
                filter,
                ..
            } => {
                let query = filter.to_lowercase();
                *filtered_indices = models
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| match query.is_empty() {
                        true => true,
                        false => m.to_lowercase().contains(&query),
                    })
                    .map(|(i, _)| i)
                    .collect();

                *selected_idx = (*selected_idx).min(filtered_indices.len().saturating_sub(1));
            }
            _ => {}
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
    /// Current specific agent activity for live shimmering status
    pub current_activity: Option<super::animation::AgentActivity>,
    /// Selected spinner animation style
    pub spinner_style: super::animation::SpinnerStyle,
    /// Clock reference for smooth millisecond animations
    pub start_time: Instant,
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
        let cwd_display = match home {
            Some(ref h) => match cwd_str.starts_with(h) {
                true => cwd_str.replacen(h, "~", 1),
                false => cwd_str,
            },
            None => cwd_str,
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
            current_activity: None,
            spinner_style: super::animation::SpinnerStyle::BrailleWave,
            start_time: Instant::now(),
        }
    }

    /// Milliseconds elapsed since the application started (for smooth animations)
    pub fn elapsed_millis(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    /// Sets the current specific agent activity (e.g. EditingFile, WebResearch, etc.)
    pub fn set_activity(&mut self, activity: super::animation::AgentActivity) {
        self.current_activity = Some(activity);
    }

    /// Clears the active activity back to idle
    pub fn clear_activity(&mut self) {
        self.current_activity = None;
    }

    /// Handles live start of a tool call
    pub fn handle_tool_start(&mut self, name: String, details: String) {
        let activity = super::animation::AgentActivity::from_tool_call(&name, &details);
        self.set_activity(activity);
        self.messages.push(ChatMessage::tool_start(name, details));
        self.scroll_to_bottom();
    }

    /// Handles live completion of a tool call
    pub fn handle_tool_end(
        &mut self,
        name: &str,
        output: String,
        summary: Option<String>,
        success: bool,
        duration_ms: Option<u64>,
    ) {
        let found = self.messages.iter_mut().rev().find(|m| {
            m.is_tool && m.tool_name.as_deref() == Some(name) && m.tool_success.is_none()
        });

        match found {
            Some(msg) => {
                msg.content = output;
                msg.tool_summary = summary;
                msg.tool_success = Some(success);
                msg.tool_duration_ms = duration_ms;
            }
            None => {
                self.messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: output,
                    is_tool: true,
                    tool_name: Some(name.to_string()),
                    tool_details: None,
                    tool_summary: summary,
                    reasoning: None,
                    thinking_time: None,
                    tool_success: Some(success),
                    tool_duration_ms: duration_ms,
                    ephemeral: false,
                });
            }
        }

        let n = name.to_ascii_lowercase();
        const POST_TOOL_RULES: &[(&[&str], &str)] = &[
            (&["check", "test", "clippy", "lint", "exec", "cmd"], "execution results"),
            (&["search", "web", "fetch", "crawl", "research"], "research findings"),
            (&["read", "doc", "pdf", "xlsx"], "document data"),
            (&["write", "patch", "edit", "create"], "code changes"),
        ];
        let next_activity = POST_TOOL_RULES
            .iter()
            .find_map(|(patterns, task_label)| {
                match patterns.iter().any(|p| n.contains(p)) {
                    true => Some(super::animation::AgentActivity::Analyzing {
                        task: (*task_label).to_string(),
                    }),
                    false => None,
                }
            })
            .unwrap_or(super::animation::AgentActivity::Working);

        self.set_activity(next_activity);
        self.update_approx_tokens();
        self.scroll_to_bottom();
    }

    pub fn queue_prompt(&mut self, prompt: String) {
        let trimmed = prompt.trim().to_string();
        match trimmed.is_empty() {
            false => self.queued_prompts.push_back(trimmed),
            true => {}
        }
    }

    pub fn pop_next_prompt(&mut self) -> Option<String> {
        self.queued_prompts.pop_front()
    }

    pub fn scroll_up(&mut self, lines: u32) {
        match self.auto_scroll {
            true => {
                self.auto_scroll = false;
                self.scroll_offset = self.max_scroll.saturating_sub(lines);
            }
            false => {
                self.scroll_offset = self.scroll_offset.saturating_sub(lines);
            }
        }
    }

    pub fn scroll_down(&mut self, lines: u32) {
        match self.auto_scroll {
            true => {}
            false => {
                let next = self.scroll_offset.saturating_add(lines);
                match next >= self.max_scroll {
                    true => {
                        self.scroll_offset = self.max_scroll;
                        self.auto_scroll = true;
                    }
                    false => {
                        self.scroll_offset = next;
                    }
                }
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

        match BRANCH_CACHE.lock() {
            Ok(guard) => match guard.get(&workspace_key) {
                Some((last_check, branch)) => {
                    cached_branch = branch.clone();
                    match now.duration_since(*last_check) < Duration::from_secs(3) {
                        true => needs_refresh = false,
                        false => {}
                    }
                }
                None => {}
            },
            Err(_) => {}
        }

        let should_spawn =
            needs_refresh && !IS_FETCHING_GIT.swap(true, std::sync::atomic::Ordering::SeqCst);
        match should_spawn {
            true => {
                let cache_key = workspace_key.clone();
                let ws = workspace_key;
                let fetcher = move || {
                    let output = std::process::Command::new("git")
                        .arg("rev-parse")
                        .arg("--abbrev-ref")
                        .arg("HEAD")
                        .current_dir(&ws)
                        .output();

                    let branch = output.ok().and_then(|out| match out.status.success() {
                        true => {
                            let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
                            match b.is_empty() {
                                false => Some(b),
                                true => None,
                            }
                        }
                        false => None,
                    });

                    match BRANCH_CACHE.lock() {
                        Ok(mut guard) => {
                            guard.insert(cache_key, (Instant::now(), branch));
                        }
                        Err(_) => {}
                    }
                    IS_FETCHING_GIT.store(false, std::sync::atomic::Ordering::SeqCst);
                };

                match tokio::runtime::Handle::try_current() {
                    Ok(handle) => {
                        handle.spawn_blocking(fetcher);
                    }
                    Err(_) => {
                        std::thread::spawn(fetcher);
                    }
                }
            }
            false => {}
        }

        cached_branch
    }

    pub fn sanitize_paste_text(raw: &str) -> String {
        let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
        let trimmed_trailing_newlines = normalized.trim_end_matches('\n');
        let mut out = String::with_capacity(trimmed_trailing_newlines.len());
        for c in trimmed_trailing_newlines.chars() {
            match c {
                '\n' => out.push('\n'),
                '\t' => out.push_str("    "),
                other => match other.is_control() {
                    true => {}
                    false => out.push(other),
                },
            }
        }
        out
    }

    pub fn insert_pasted_text(&mut self, raw: &str) {
        let sanitized = Self::sanitize_paste_text(raw);
        let chars: Vec<char> = sanitized.chars().collect();
        match chars.is_empty() {
            true => {}
            false => {
                let idx = self.cursor_idx.min(self.typed_input.len());
                let count = chars.len();
                self.typed_input.splice(idx..idx, chars);
                self.cursor_idx = idx + count;
                self.selected_index = None;
                self.slash_selected_idx = 0;
                self.history_idx = None;
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        match c == '\n' || !c.is_control() {
            false => {}
            true => {
                let idx = self.cursor_idx.min(self.typed_input.len());
                self.typed_input.insert(idx, c);
                self.cursor_idx = idx + 1;
                self.selected_index = None;
                self.slash_selected_idx = 0;
                self.history_idx = None;
            }
        }
    }

    pub fn delete_char_before_cursor(&mut self) {
        let idx = self.cursor_idx.min(self.typed_input.len());
        match idx > 0 {
            true => {
                self.typed_input.remove(idx - 1);
                self.cursor_idx = idx - 1;
            }
            false => {}
        }
        self.selected_index = None;
        self.slash_selected_idx = 0;
        self.history_idx = None;
    }

    pub fn delete_char_at_cursor(&mut self) {
        let idx = self.cursor_idx.min(self.typed_input.len());
        match idx < self.typed_input.len() {
            true => {
                self.typed_input.remove(idx);
                self.selected_index = None;
                self.slash_selected_idx = 0;
                self.history_idx = None;
            }
            false => {}
        }
    }

    pub fn is_slash_command_input(trimmed: &str) -> bool {
        match trimmed.contains('\n') {
            true => false,
            false => match trimmed == "exit" || trimmed == "quit" {
                true => true,
                false => match trimmed.strip_prefix('/') {
                    Some(rest) => {
                        let cmd_token = rest.split_whitespace().next().unwrap_or("");
                        !cmd_token.is_empty() && !cmd_token.contains('/')
                    }
                    None => false,
                },
            },
        }
    }

    pub fn matching_slash_commands(&self) -> Vec<(String, String)> {
        match self.has_active_slash_query() {
            false => Vec::new(),
            true => {
                let input_str: String = self.typed_input.iter().collect();
                SLASH_COMMANDS
                    .iter()
                    .copied()
                    .filter(|(cmd, _)| cmd.starts_with(&input_str))
                    .map(|(cmd, desc)| (cmd.to_string(), desc.to_string()))
                    .collect()
            }
        }
    }

    /// Checks whether the user is typing a slash command (starts with '/' and hasn't started typing arguments or paths)
    pub fn has_active_slash_query(&self) -> bool {
        self.typed_input.first() == Some(&'/')
            && !self
                .typed_input
                .iter()
                .skip(1)
                .any(|&c| c.is_whitespace() || c == '/')
    }

    /// Extract query text without leading slash
    pub fn slash_search_text(&self) -> String {
        let input: String = self.typed_input.iter().collect();
        match input.strip_prefix('/') {
            Some(stripped) => stripped.trim().to_string(),
            None => String::new(),
        }
    }

    /// Matching palette commands based on active category and query
    pub fn matching_palette_commands(&self) -> Vec<usize> {
        let categories = CommandCategory::all();
        let current_cat = categories[self.slash_category_idx % categories.len()];
        let query = self.slash_search_text().to_lowercase();

        let mut matches: Vec<usize> = PALETTE_COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, cmd)| {
                let category_ok =
                    current_cat == CommandCategory::All || cmd.category == current_cat;
                let query_ok = match query.is_empty() {
                    true => true,
                    false => {
                        let name = cmd.slash_name.trim_start_matches('/').to_lowercase();
                        let title = cmd.title.to_lowercase();
                        let desc = cmd.description.to_lowercase();
                        name.starts_with(&query)
                            || title.starts_with(&query)
                            || name.contains(&query)
                            || title.contains(&query)
                            || desc.contains(&query)
                    }
                };
                category_ok && query_ok
            })
            .map(|(i, _)| i)
            .collect();

        match query.is_empty() {
            false => {
                matches.sort_by_key(|&idx| {
                    let cmd = &PALETTE_COMMANDS[idx];
                    let name = cmd.slash_name.trim_start_matches('/').to_lowercase();
                    let title = cmd.title.to_lowercase();
                    match (
                        name == query,
                        name.starts_with(&query),
                        title.starts_with(&query),
                        name.contains(&query),
                        title.contains(&query),
                    ) {
                        (true, _, _, _, _) => 0,
                        (false, true, _, _, _) => 1,
                        (false, false, true, _, _) => 2,
                        (false, false, false, true, _) => 3,
                        (false, false, false, false, true) => 4,
                        (false, false, false, false, false) => 5,
                    }
                });
            }
            true => {}
        }

        matches
    }

    /// Returns the currently selected palette command
    pub fn selected_palette_command(&self) -> Option<&'static PaletteCommand> {
        let matches = self.matching_palette_commands();
        match matches.is_empty() {
            true => None,
            false => {
                let idx = self.slash_selected_idx.min(matches.len().saturating_sub(1));
                Some(&PALETTE_COMMANDS[matches[idx]])
            }
        }
    }

    /// Cycle category tabs in spotlight palette (Tab / BackTab)
    pub fn cycle_slash_category(&mut self, forward: bool) {
        let total = CommandCategory::all().len();
        match (total, forward, self.slash_category_idx) {
            (0, _, _) => return,
            (_, true, idx) => self.slash_category_idx = (idx + 1) % total,
            (_, false, 0) => self.slash_category_idx = total.saturating_sub(1),
            (_, false, idx) => self.slash_category_idx = idx.saturating_sub(1),
        }
        self.slash_selected_idx = 0;
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
