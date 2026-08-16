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
            reasoning,
            thinking_time,
            tool_success,
            tool_duration_ms: None,
            ephemeral: false,
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
            reasoning: None,
            thinking_time: None,
            tool_success: Some(success),
            tool_duration_ms: Some(duration_ms),
            ephemeral: false,
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
        }
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
        self.scroll_to_bottom();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_scroll(max_scroll: u32) -> RatatuiApp {
        let mut app = RatatuiApp::new("test-model".into(), "test-provider".into(), "cli:test".into());
        app.max_scroll = max_scroll;
        app
    }

    #[test]
    fn scroll_up_from_auto_scroll_lands_near_bottom() {
        let mut app = app_with_scroll(100);
        assert!(app.auto_scroll);
        app.scroll_up(3);
        assert!(!app.auto_scroll);
        assert_eq!(app.scroll_offset, 97);
    }

    #[test]
    fn scroll_up_clamps_at_zero() {
        let mut app = app_with_scroll(10);
        app.scroll_up(2); // leave auto mode
        app.scroll_up(50); // way past the top
        assert_eq!(app.scroll_offset, 0);
        assert!(!app.auto_scroll);
    }

    #[test]
    fn scroll_down_reengages_auto_scroll_at_bottom() {
        let mut app = app_with_scroll(100);
        app.scroll_up(10);
        assert_eq!(app.scroll_offset, 90);
        app.scroll_down(5);
        assert_eq!(app.scroll_offset, 95);
        assert!(!app.auto_scroll);
        app.scroll_down(5); // reaches the bottom edge
        assert_eq!(app.scroll_offset, 100);
        assert!(app.auto_scroll);
    }

    #[test]
    fn scroll_down_is_noop_when_auto_scrolling() {
        let mut app = app_with_scroll(100);
        app.scroll_to_bottom(); // anchor the field at max, auto mode on
        app.scroll_down(40); // auto mode absorbs it — no manual scroll starts
        assert_eq!(app.scroll_offset, 100);
        assert!(app.auto_scroll);
    }

    #[test]
    fn scroll_to_top_and_bottom_edges() {
        let mut app = app_with_scroll(100);
        app.scroll_to_top();
        assert_eq!(app.scroll_offset, 0);
        assert!(!app.auto_scroll);
        app.scroll_to_bottom();
        assert_eq!(app.scroll_offset, 100);
        assert!(app.auto_scroll);
    }

    #[test]
    fn apply_sync_session_preserves_recent_notices_only() {
        let mut app = app_with_scroll(50);
        for i in 0..7 {
            app.messages.push(ChatMessage::notice(format!("note {}", i)));
        }
        app.messages.push(ChatMessage::simple("user", "hello".into()));
        app.messages.push(ChatMessage::simple("assistant", "hi".into()));
        app.scroll_to_top();

        let disk = vec![
            ChatMessage::simple("user", "from disk".into()),
            ChatMessage::simple("assistant", "disk reply".into()),
        ];
        app.apply_sync_session(disk);

        assert_eq!(app.messages.len(), 7); // 2 disk + 5 kept notices (capped)
        assert_eq!(app.messages[0].content, "from disk");
        assert!(!app.messages[0].ephemeral);
        // Oldest two notices dropped, newest five kept in order
        let notices: Vec<&str> = app.messages[2..]
            .iter()
            .map(|m| m.content.as_str())
            .collect();
        assert_eq!(notices, vec!["note 2", "note 3", "note 4", "note 5", "note 6"]);
        assert!(app.auto_scroll); // re-anchored to bottom
    }

    #[test]
    fn from_session_message_extracts_extras() {
        let mut extra = serde_json::Map::new();
        extra.insert("tool_name".into(), serde_json::json!("read_file"));
        extra.insert("tool_details".into(), serde_json::json!("path=src/main.rs"));
        extra.insert("reasoning_content".into(), serde_json::json!("let me think"));
        extra.insert("thinking_time_secs".into(), serde_json::json!(1.25));
        let msg = crate::session::Message {
            role: "tool".into(),
            content: "file contents".into(),
            timestamp: None,
            extra,
        };

        let chat = ChatMessage::from_session_message(&msg);
        assert!(chat.is_tool);
        assert_eq!(chat.tool_name.as_deref(), Some("read_file"));
        assert_eq!(chat.tool_details.as_deref(), Some("path=src/main.rs"));
        assert_eq!(chat.reasoning.as_deref(), Some("let me think"));
        assert_eq!(chat.thinking_time, Some(1.25));
        assert!(!chat.ephemeral);
    }

    #[test]
    fn notice_is_ephemeral_and_defaults_are_not() {
        let notice = ChatMessage::notice("keep me".into());
        assert!(notice.ephemeral);
        assert!(!ChatMessage::simple("user", "x".into()).ephemeral);
        assert!(!ChatMessage::tool_start("t".into(), "d".into()).ephemeral);
    }
}
