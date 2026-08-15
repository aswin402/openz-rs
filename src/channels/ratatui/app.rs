use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::theme::Theme;

pub static IS_RATATUI_ACTIVE: AtomicBool = AtomicBool::new(false);

static BRANCH_CACHE: Mutex<Option<(Instant, Option<String>)>> = Mutex::new(None);
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
    /// Reasoning/thinking content from the LLM
    pub reasoning: Option<String>,
    /// Time spent thinking (in seconds)
    pub thinking_time: Option<f64>,
    /// Tool execution success status
    pub tool_success: Option<bool>,
    /// Tool execution duration in milliseconds
    pub tool_duration_ms: Option<u64>,
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
            tool_success: None,
            tool_duration_ms: None,
        }
    }

    pub fn tool_start(name: String, details: String) -> Self {
        Self {
            role: "tool".to_string(),
            content: String::new(),
            is_tool: true,
            tool_name: Some(name),
            tool_details: Some(details),
            reasoning: None,
            thinking_time: None,
            tool_success: None,
            tool_duration_ms: None,
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
            reasoning: None,
            thinking_time: None,
            tool_success: Some(success),
            tool_duration_ms: Some(duration_ms),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ModalState {
    None,
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
}

impl ModalState {
    pub fn is_active(&self) -> bool {
        !matches!(self, ModalState::None)
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
    pub scroll_offset: u16,
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
}

pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/model", "Switch active LLM provider and model interactively"),
    ("/clear", "Clear conversation timeline"),
    ("/history", "Restore or switch chat sessions"),
    ("/new-session", "Start a clean conversation session"),
    ("/help", "Show help, keybindings and commands"),
    ("/mcps", "List configured MCP servers and active status"),
    ("/memory", "Inspect cognitive facts and knowledge graph"),
    ("/skills", "List active learned skills"),
    ("/workflows", "Search and execute reusable SOP workflows"),
    ("/servers", "List OpenZ background server instances"),
    ("/logs", "Stream real-time color-coded structured logs"),
    ("/settings", "View and adjust active configuration"),
    ("/streaming", "Toggle response streaming preference"),
    ("/device", "Manage local application and device inventory"),
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
        }
    }

    /// Retrieve current git branch with background cache refresh
    pub fn get_git_branch(workspace: &Path) -> Option<String> {
        let now = Instant::now();
        let mut cached_branch = None;
        let mut needs_refresh = true;

        if let Ok(guard) = BRANCH_CACHE.lock() {
            if let Some((last_check, ref branch)) = *guard {
                cached_branch = branch.clone();
                if now.duration_since(last_check) < Duration::from_secs(3) {
                    needs_refresh = false;
                }
            }
        }

        if needs_refresh && !IS_FETCHING_GIT.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let ws = workspace.to_path_buf();
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
                    *guard = Some((Instant::now(), branch));
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
}
