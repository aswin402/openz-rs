use crate::agent::style::*;
use crate::config::schema::Config;
use crate::providers::{GenerationSettings, LLMProvider};
use crate::session::{Message, Session, SessionManager};
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::sync::Arc;

const DEFAULT_PROVIDER_ATTEMPT_TIMEOUT_SECS: u64 = 45;
const MAX_PROVIDER_ATTEMPT_TIMEOUT_SECS: u64 = 120;
const DEFAULT_MAX_FALLBACK_ATTEMPTS: usize = 2;
const HARD_MAX_FALLBACK_ATTEMPTS: usize = 8;

pub mod build;
pub mod command;
pub mod compact;
pub mod intent;
pub mod loop_control;
pub mod research_policy;
pub mod restore;
pub mod run;
pub mod save;
pub mod security_approval;
pub mod streaming;
pub mod tool_execution;
pub mod transcript;

#[cfg(test)]
mod integration_tests;

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

#[derive(Clone, Debug)]
pub struct TurnCancellationContext {
    pub turn_id: String,
    pub token: crate::tools::subagent::CancellationToken,
}

tokio::task_local! {
    static TURN_CANCELLATION_CONTEXT: TurnCancellationContext;
}

pub fn current_turn_cancellation_context() -> Option<TurnCancellationContext> {
    TURN_CANCELLATION_CONTEXT.try_with(Clone::clone).ok()
}

pub fn current_turn_id() -> Option<String> {
    current_turn_cancellation_context().map(|context| context.turn_id)
}

pub async fn with_turn_cancellation_context<F, T>(
    context: TurnCancellationContext,
    future: F,
) -> T
where
    F: std::future::Future<Output = T>,
{
    TURN_CANCELLATION_CONTEXT.scope(context, future).await
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
    pub workflow_notice_names: std::collections::HashSet<String>,
}

struct ActivityGuard<'a> {
    session_key: &'a str,
}

impl<'a> Drop for ActivityGuard<'a> {
    fn drop(&mut self) {
        crate::agent::activity::update_activity(self.session_key, "Idle", None);
    }
}

static SESSION_LOCKS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>,
> = std::sync::OnceLock::new();

pub fn apply_session_overrides(
    config: &mut Config,
    metadata: &serde_json::Map<String, serde_json::Value>,
) {
    let apply_map = |cfg: &mut Config, map: &serde_json::Map<String, serde_json::Value>| {
        if let Some(model) = map.get("model").and_then(|v| v.as_str()) {
            cfg.agents.defaults.model = model.to_string();
        }
        if let Some(provider) = map.get("provider").and_then(|v| v.as_str()) {
            cfg.agents.defaults.provider = provider.to_string();
        }
        if let Some(temp) = map.get("temperature").and_then(|v| v.as_f64()) {
            cfg.agents.defaults.temperature = temp as f32;
        }
        if let Some(tokens) = map.get("max_tokens").and_then(|v| v.as_u64()) {
            cfg.agents.defaults.max_tokens = tokens as usize;
        }
        if let Some(max_msg) = map.get("max_messages").and_then(|v| v.as_u64()) {
            cfg.agents.defaults.max_messages = max_msg as usize;
        }
        if let Some(max_iter) = map.get("max_tool_iterations").and_then(|v| v.as_u64()) {
            cfg.agents.defaults.max_tool_iterations = max_iter as usize;
        }
        if let Some(caveman) = map.get("caveman_mode").and_then(|v| v.as_bool()) {
            cfg.agents.defaults.caveman_mode = caveman;
        }
        if let Some(timeout) = map.get("tool_timeout_secs").and_then(|v| v.as_u64()) {
            cfg.agents.defaults.tool_timeout_secs = timeout;
        }
        if let Some(streaming) = map.get("streaming").and_then(|v| v.as_bool()) {
            cfg.agents.defaults.streaming = streaming;
        }
    };

    if let Some(nested) = metadata.get("config_override").and_then(|v| v.as_object()) {
        apply_map(config, nested);
    } else if let Some(nested) = metadata.get("config").and_then(|v| v.as_object()) {
        apply_map(config, nested);
    }
    apply_map(config, metadata);
}

fn merge_latest_config_for_runtime(
    runtime_config: &Config,
    mut latest_config: Config,
    _session_key: &str,
    session_metadata: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Config {
    // Preserve the runtime overridden model/provider for any active loop execution.
    // This allows message-level overrides or dynamic updates to stick during the turn.
    latest_config.agents.defaults.model = runtime_config.agents.defaults.model.clone();
    latest_config.agents.defaults.provider = runtime_config.agents.defaults.provider.clone();
    latest_config.agents.defaults.fallback_models =
        runtime_config.agents.defaults.fallback_models.clone();

    if let Some(metadata) = session_metadata {
        apply_session_overrides(&mut latest_config, metadata);
    }
    latest_config
}

fn get_session_lock(session_key: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let map = SESSION_LOCKS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    if guard.len() > 100 {
        guard.retain(|_, arc| std::sync::Arc::strong_count(arc) > 1);
    }
    guard
        .entry(session_key.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

fn max_fallback_attempts() -> usize {
    std::env::var("OPENZ_MAX_FALLBACK_ATTEMPTS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_FALLBACK_ATTEMPTS)
        .min(HARD_MAX_FALLBACK_ATTEMPTS)
}

fn fallback_models_for_turn(config: &Config) -> Vec<String> {
    let max_attempts = max_fallback_attempts();
    if max_attempts == 0 {
        return Vec::new();
    }

    let active_model = config.agents.defaults.model.trim();
    let mut fallbacks = Vec::new();
    for fallback in &config.agents.defaults.fallback_models {
        let Some(model) = fallback
            .as_str()
            .map(str::trim)
            .filter(|model| !model.is_empty())
        else {
            continue;
        };
        if model.eq_ignore_ascii_case(active_model)
            || fallbacks
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(model))
        {
            continue;
        }
        fallbacks.push(model.to_string());
        if fallbacks.len() >= max_attempts {
            break;
        }
    }
    fallbacks
}

async fn wait_for_turn_cancellation() {
    let Some(context) = current_turn_cancellation_context() else {
        std::future::pending::<()>().await;
        return;
    };
    context.token.wait_for_cancellation().await;
}

async fn wait_for_cli_cancel_since(initial: u64) {
    let mut rx = crate::shutdown::cli_cancel_tx().subscribe();
    while *rx.borrow() == initial {
        if rx.changed().await.is_err() {
            break;
        }
    }
}

async fn cancel_aware_with_spinner<F, T>(
    activity_msg: &str,
    initial_cancel: u64,
    future: F,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    tokio::select! {
        biased;
        _ = wait_for_turn_cancellation() => Err(anyhow::anyhow!("Cancelled by user")),
        _ = wait_for_cli_cancel_since(initial_cancel) => Err(anyhow::anyhow!("Cancelled by user")),
        result = with_spinner(activity_msg, future) => result,
    }
}

async fn cancel_aware_provider_call<F, T>(
    activity_msg: &str,
    initial_cancel: u64,
    timeout_secs: u64,
    future: F,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    cancel_aware_with_spinner(activity_msg, initial_cancel, async move {
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), future).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!(
                "Provider response timed out after {}s",
                timeout_secs
            )),
        }
    })
    .await
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

    pub(crate) fn provider_attempt_timeout_secs(&self) -> u64 {
        let configured = std::env::var("OPENZ_PROVIDER_ATTEMPT_TIMEOUT_SECS")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or(DEFAULT_PROVIDER_ATTEMPT_TIMEOUT_SECS);
        configured
            .min(self.config.agents.defaults.tool_timeout_secs.max(1))
            .clamp(1, MAX_PROVIDER_ATTEMPT_TIMEOUT_SECS)
    }

    pub(crate) fn provider_attempt_timeout_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.provider_attempt_timeout_secs())
    }

    fn cleanup_old_files() {
        use std::time::{Duration, SystemTime};

        let max_age = Duration::from_secs(7 * 24 * 60 * 60); // 7 days
        let now = SystemTime::now();

        for dir_name in &["traces", "tool_outputs"] {
            let dir = crate::config::runtime_data_dir().join(dir_name);
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
        // Helper: check if CLI cancel was fired since we started
        let cwf_cancel_tx = crate::shutdown::cli_cancel_tx();
        let cwf_cancel_rx = cwf_cancel_tx.subscribe();
        let cwf_cancel_initial = *cwf_cancel_rx.borrow();
        let is_cancelled = || -> bool { *cwf_cancel_rx.borrow() != cwf_cancel_initial };
        let provider_timeout_secs = self.provider_attempt_timeout_secs();

        let chat_fut = active_provider.chat(system_prompt, messages, tools, settings);
        let mut chat_result = cancel_aware_provider_call(
            activity_msg,
            cwf_cancel_initial,
            provider_timeout_secs,
            chat_fut,
        )
        .await;

        if chat_result.is_err() {
            if let Err(ref err) = chat_result {
                let provider_name = self.config.agents.defaults.provider.as_str();
                let model_name = self.config.agents.defaults.model.as_str();
                let risk = crate::channels::classify_model_risk(provider_name, model_name);
                let _ = crate::model_registry::record_model_failure(
                    provider_name,
                    model_name,
                    risk.tier,
                    risk.risky,
                    risk.reasons
                        .iter()
                        .map(|reason| reason.to_string())
                        .collect(),
                    &err.to_string(),
                );
            }
            if is_cancelled() {
                return Err(anyhow::anyhow!("Cancelled by user"));
            }

            let fallbacks = fallback_models_for_turn(&self.config);

            let mut resolved_fallback = false;
            for fallback_model in fallbacks {
                // Check cancel before each fallback attempt
                if is_cancelled() {
                    return Err(anyhow::anyhow!("Cancelled by user"));
                }
                let silent = crate::agent::style::spinner::is_silent();
                if !silent {
                    crate::tui_println!(
                        "{}▲ Primary provider failed. Attempting fallback model: {}{}",
                        AURA_GOLD,
                        fallback_model,
                        COLOR_RESET
                    );
                }
                if let Ok(fallback_provider) =
                    crate::tools::subagent::build_provider_for_model(&self.config, &fallback_model)
                {
                    let chat_fut = fallback_provider.chat(system_prompt, messages, tools, settings);
                    chat_result = cancel_aware_provider_call(
                        activity_msg,
                        cwf_cancel_initial,
                        provider_timeout_secs,
                        chat_fut,
                    )
                    .await;
                    if chat_result.is_ok() {
                        resolved_fallback = true;
                        let fallback_provider_name = fallback_model
                            .split_once('/')
                            .map(|(provider, _)| provider)
                            .unwrap_or("auto");
                        let risk = crate::channels::classify_model_risk(
                            fallback_provider_name,
                            &fallback_model,
                        );
                        let _ = crate::model_registry::record_model_success(
                            fallback_provider_name,
                            &fallback_model,
                            risk.tier,
                            risk.risky,
                            risk.reasons
                                .iter()
                                .map(|reason| reason.to_string())
                                .collect(),
                            false,
                            false,
                            true,
                        );
                        *active_provider = fallback_provider;
                        break;
                    }
                }
            }

            if !resolved_fallback && is_cancelled() {
                return Err(anyhow::anyhow!("Cancelled by user"));
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
    ) -> Result<
        std::pin::Pin<
            Box<dyn futures_util::Stream<Item = Result<crate::providers::ChatStreamChunk>> + Send>,
        >,
    > {
        let csf_cancel_tx = crate::shutdown::cli_cancel_tx();
        let csf_cancel_rx = csf_cancel_tx.subscribe();
        let csf_cancel_initial = *csf_cancel_rx.borrow();
        let is_cancelled = || -> bool { *csf_cancel_rx.borrow() != csf_cancel_initial };
        let provider_timeout_secs = self.provider_attempt_timeout_secs();

        let chat_fut = active_provider.chat_stream(system_prompt, messages, tools, settings);
        let mut chat_result = cancel_aware_provider_call(
            activity_msg,
            csf_cancel_initial,
            provider_timeout_secs,
            chat_fut,
        )
        .await;

        if chat_result.is_err() {
            if let Err(ref err) = chat_result {
                let provider_name = self.config.agents.defaults.provider.as_str();
                let model_name = self.config.agents.defaults.model.as_str();
                let risk = crate::channels::classify_model_risk(provider_name, model_name);
                let _ = crate::model_registry::record_model_failure(
                    provider_name,
                    model_name,
                    risk.tier,
                    risk.risky,
                    risk.reasons
                        .iter()
                        .map(|reason| reason.to_string())
                        .collect(),
                    &err.to_string(),
                );
            }
            let fallbacks = fallback_models_for_turn(&self.config);

            let mut resolved_fallback = false;
            for fallback_model in fallbacks {
                if is_cancelled() {
                    return Err(anyhow::anyhow!("Cancelled by user"));
                }
                let silent = crate::agent::style::spinner::is_silent();
                if !silent {
                    crate::tui_println!(
                        "{}▲ Primary provider failed. Attempting fallback model: {}{}",
                        AURA_GOLD,
                        fallback_model,
                        COLOR_RESET
                    );
                }
                if let Ok(fallback_provider) =
                    crate::tools::subagent::build_provider_for_model(&self.config, &fallback_model)
                {
                    let chat_fut =
                        fallback_provider.chat_stream(system_prompt, messages, tools, settings);
                    chat_result = cancel_aware_provider_call(
                        activity_msg,
                        csf_cancel_initial,
                        provider_timeout_secs,
                        chat_fut,
                    )
                    .await;
                    if chat_result.is_ok() {
                        resolved_fallback = true;
                        let fallback_provider_name = fallback_model
                            .split_once('/')
                            .map(|(provider, _)| provider)
                            .unwrap_or("auto");
                        let risk = crate::channels::classify_model_risk(
                            fallback_provider_name,
                            &fallback_model,
                        );
                        let _ = crate::model_registry::record_model_success(
                            fallback_provider_name,
                            &fallback_model,
                            risk.tier,
                            risk.risky,
                            risk.reasons
                                .iter()
                                .map(|reason| reason.to_string())
                                .collect(),
                            false,
                            false,
                            true,
                        );
                        *active_provider = fallback_provider;
                        break;
                    }
                }
            }

            if !resolved_fallback && is_cancelled() {
                return Err(anyhow::anyhow!("Cancelled by user"));
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

        let is_cli = target_key.starts_with("cli:")
            && (!session_key.starts_with("subagent:") || crate::shutdown::is_cli_active());
        let is_ratatui = crate::channels::ratatui::app::IS_RATATUI_ACTIVE
            .load(std::sync::atomic::Ordering::Relaxed);
        let silent = !is_cli || is_ratatui;

        crate::agent::style::spinner::IS_SILENT
            .scope(silent, async move {
                crate::agent::style::spinner::CURRENT_SESSION_KEY
                    .scope(target_key, async move {
                        let span = tracing::info_span!("turn", session = %session_key);
                        let _enter = span.enter();
                        self.run_inner(user_content, session_key).await
                    })
                    .await
            })
            .await
    }

    async fn run_inner(&self, user_content: &str, session_key: &str) -> Result<RunResult> {
        static CLEANUP_ONCE: std::sync::Once = std::sync::Once::new();
        CLEANUP_ONCE.call_once(|| {
            Self::cleanup_old_files();
        });

        self.tools.begin_turn();
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
            workflow_notice_names: std::collections::HashSet::new(),
        };

        let mut state = TurnState::Restore;
        while state != TurnState::Done {
            // Reload configuration dynamically from disk at the start of each turn iteration
            if let Ok(latest_config) = crate::config::loader::load_config() {
                let old_provider = ctx.config.agents.defaults.provider.clone();
                let old_model = ctx.config.agents.defaults.model.clone();
                let session_metadata = if state == TurnState::Restore {
                    None
                } else {
                    Some(&ctx.session.metadata)
                };
                let merged = merge_latest_config_for_runtime(
                    &ctx.config,
                    latest_config,
                    session_key,
                    session_metadata,
                );
                let provider_changed = merged.agents.defaults.provider != old_provider
                    || merged.agents.defaults.model != old_model;
                if provider_changed {
                    match crate::providers::resolver::resolve_provider_full(
                        &merged,
                        &merged.agents.defaults.model,
                    ) {
                        Ok(resolved) => {
                            ctx.active_provider = resolved.instance;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to resolve updated provider/model from config: {}",
                                e
                            );
                        }
                    }
                }
                ctx.config = merged;
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

        let cleaned_tasks = crate::tools::task_manager::cleanup_turn_end_tasks();
        if cleaned_tasks > 0 {
            tracing::debug!(cleaned_tasks, "Cleaned OpenZ turn-scoped tasks");
        }

        Ok(RunResult {
            content: ctx.final_content,
            tools_used: ctx.tools_used,
            streamed: ctx.streamed,
        })
    }
}

#[cfg(test)]
mod tests;
