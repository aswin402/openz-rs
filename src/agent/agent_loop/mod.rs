use crate::config::schema::Config;
use crate::providers::{LLMProvider, GenerationSettings};
use crate::tools::ToolRegistry;
use crate::session::{Session, SessionManager, Message};
use crate::agent::style::*;
use anyhow::Result;
use std::sync::Arc;

pub mod restore;
pub mod compact;
pub mod command;
pub mod build;
pub mod run;
pub mod save;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    Restore,
    Compact,
    Command,
    Build,
    Run,
    Save,
    Done,
}

pub struct AgentLoop {
    pub config: Config,
    pub provider: Arc<dyn LLMProvider>,
    pub tools: ToolRegistry,
    pub session_manager: SessionManager,
}

pub struct RunResult {
    pub content: String,
    pub tools_used: Vec<String>,
    pub streamed: bool,
}

pub struct TurnContext<'a> {
    pub session_key: &'a str,
    pub user_content: &'a str,
    pub active_provider: Arc<dyn LLMProvider>,
    pub session: Session,
    pub messages: Vec<Message>,
    pub system_prompt: String,
    pub final_content: String,
    pub tools_used: Vec<String>,
    pub interaction_id: Option<String>,
    pub turn_errors: Vec<String>,
    pub session_file_lock: Option<std::fs::File>,
    pub streamed: bool,
    pub config: Config,
}

struct ActivityGuard<'a> {
    session_key: &'a str,
}

impl<'a> Drop for ActivityGuard<'a> {
    fn drop(&mut self) {
        crate::agent::activity::update_activity(self.session_key, "Idle", None);
    }
}

static SESSION_LOCKS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> = std::sync::OnceLock::new();

fn get_session_lock(session_key: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let map = SESSION_LOCKS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    if guard.len() > 100 {
        guard.retain(|_, arc| std::sync::Arc::strong_count(arc) > 1);
    }
    guard.entry(session_key.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

impl AgentLoop {
    pub fn new(
        config: Config,
        provider: Arc<dyn LLMProvider>,
        tools: ToolRegistry,
        session_manager: SessionManager,
    ) -> Self {
        AgentLoop {
            config,
            provider,
            tools,
            session_manager,
        }
    }

    pub fn update_model_and_provider(&mut self, config: Config, provider: Arc<dyn LLMProvider>) {
        self.config = config.clone();
        self.provider = provider.clone();
        if let Some(ref mut ctx) = self.tools.context {
            ctx.0 = config;
            ctx.1 = provider;
        }
    }

    fn cleanup_old_files() {
        use std::time::{SystemTime, Duration};

        let max_age = Duration::from_secs(7 * 24 * 60 * 60); // 7 days
        let now = SystemTime::now();

        for dir_name in &["traces", "tool_outputs"] {
            let dir = crate::config::resolve_path(&format!("~/.openz/{}", dir_name));
            if !dir.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age > max_age {
                                    let _ = std::fs::remove_file(entry.path());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn chat_with_fallback(
        &self,
        active_provider: &mut Arc<dyn LLMProvider>,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
        activity_msg: &str,
    ) -> Result<crate::providers::LLMResponse> {
        let chat_fut = active_provider.chat(system_prompt, messages, tools, settings);
        let mut chat_result = with_spinner(activity_msg, chat_fut).await;

        if chat_result.is_err() {
            let mut fallbacks = Vec::new();
            for fallback in &self.config.agents.defaults.fallback_models {
                if let Some(s) = fallback.as_str() {
                    if !s.trim().is_empty() {
                        fallbacks.push(s.trim().to_string());
                    }
                }
            }

            let mut resolved_fallback = false;
            for fallback_model in fallbacks {
                let silent = crate::agent::style::spinner::is_silent();
                if !silent {
                    crate::tui_println!(
                        "{}▲ Primary provider failed. Attempting fallback model: {}{}",
                        AURA_GOLD, fallback_model, COLOR_RESET
                    );
                }
                if let Ok(fallback_provider) = crate::tools::subagent::build_provider_for_model(&self.config, &fallback_model) {
                    let chat_fut = fallback_provider.chat(system_prompt, messages, tools, settings);
                    chat_result = with_spinner(activity_msg, chat_fut).await;
                    if chat_result.is_ok() {
                        resolved_fallback = true;
                        *active_provider = fallback_provider;
                        break;
                    }
                }
            }

            if !resolved_fallback {
                let chat_fut = active_provider.chat(system_prompt, messages, tools, settings);
                chat_result = with_spinner(activity_msg, chat_fut).await;
            }
        }

        chat_result
    }

    pub async fn chat_stream_with_fallback(
        &self,
        active_provider: &mut Arc<dyn LLMProvider>,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
        settings: &GenerationSettings,
        activity_msg: &str,
    ) -> Result<std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<crate::providers::ChatStreamChunk>> + Send>>> {
        let chat_fut = active_provider.chat_stream(system_prompt, messages, tools, settings);
        let mut chat_result = with_spinner(activity_msg, chat_fut).await;

        if chat_result.is_err() {
            let mut fallbacks = Vec::new();
            for fallback in &self.config.agents.defaults.fallback_models {
                if let Some(s) = fallback.as_str() {
                    if !s.trim().is_empty() {
                        fallbacks.push(s.trim().to_string());
                    }
                }
            }

            let mut resolved_fallback = false;
            for fallback_model in fallbacks {
                let silent = crate::agent::style::spinner::is_silent();
                if !silent {
                    crate::tui_println!(
                        "{}▲ Primary provider failed. Attempting fallback model: {}{}",
                        AURA_GOLD, fallback_model, COLOR_RESET
                    );
                }
                if let Ok(fallback_provider) = crate::tools::subagent::build_provider_for_model(&self.config, &fallback_model) {
                    let chat_fut = fallback_provider.chat_stream(system_prompt, messages, tools, settings);
                    chat_result = with_spinner(activity_msg, chat_fut).await;
                    if chat_result.is_ok() {
                        resolved_fallback = true;
                        *active_provider = fallback_provider;
                        break;
                    }
                }
            }

            if !resolved_fallback {
                let chat_fut = active_provider.chat_stream(system_prompt, messages, tools, settings);
                chat_result = with_spinner(activity_msg, chat_fut).await;
            }
        }

        chat_result
    }

    pub async fn run(&self, user_content: &str, session_key: &str) -> Result<RunResult> {
        let lock = get_session_lock(session_key);
        let _guard = lock.lock().await;

        let parent_key = crate::agent::style::spinner::get_current_session_key();
        let target_key = match parent_key {
            Some(ref pk) if !pk.starts_with("subagent:") => pk.clone(),
            _ => session_key.to_string(),
        };

        let is_cli = target_key.starts_with("cli:") && (!session_key.starts_with("subagent:") || crate::shutdown::is_cli_active());
        let silent = !is_cli;

        crate::agent::style::spinner::IS_SILENT.scope(silent, async move {
            crate::agent::style::spinner::CURRENT_SESSION_KEY.scope(target_key, async move {
                let span = tracing::info_span!("turn", session = %session_key);
                let _enter = span.enter();
                self.run_inner(user_content, session_key).await
            }).await
        }).await
    }

    async fn run_inner(&self, user_content: &str, session_key: &str) -> Result<RunResult> {
        static CLEANUP_ONCE: std::sync::Once = std::sync::Once::new();
        CLEANUP_ONCE.call_once(|| {
            Self::cleanup_old_files();
        });

        crate::agent::activity::update_activity(session_key, "Processing user prompt", None);
        let _guard = ActivityGuard { session_key };

        let mut ctx = TurnContext {
            session_key,
            user_content,
            active_provider: self.provider.clone(),
            session: Session::new(session_key),
            messages: Vec::new(),
            system_prompt: String::new(),
            final_content: String::new(),
            tools_used: Vec::new(),
            interaction_id: None,
            turn_errors: Vec::new(),
            session_file_lock: None,
            streamed: false,
            config: self.config.clone(),
        };

        let mut state = TurnState::Restore;
        while state != TurnState::Done {
            // Reload configuration dynamically from disk at the start of each turn iteration
            if let Ok(latest_config) = crate::config::loader::load_config() {
                ctx.config = latest_config;
            }

            state = match state {
                TurnState::Restore => restore::handle(self, &mut ctx).await?,
                TurnState::Compact => compact::handle(self, &mut ctx).await?,
                TurnState::Command => command::handle(self, &mut ctx).await?,
                TurnState::Build => build::handle(self, &mut ctx).await?,
                TurnState::Run => run::handle(self, &mut ctx).await?,
                TurnState::Save => save::handle(self, &mut ctx).await?,
                TurnState::Done => TurnState::Done,
            };
        }

        Ok(RunResult {
            content: ctx.final_content,
            tools_used: ctx.tools_used,
            streamed: ctx.streamed,
        })
    }
}
