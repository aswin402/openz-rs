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
        match map.get("model").and_then(|v| v.as_str()) {
            Some(model) => cfg.agents.defaults.model = model.to_string(),
            None => {}
        }
        match map.get("provider").and_then(|v| v.as_str()) {
            Some(provider) => cfg.agents.defaults.provider = provider.to_string(),
            None => {}
        }
        match map.get("temperature").and_then(|v| v.as_f64()) {
            Some(temp) => cfg.agents.defaults.temperature = temp as f32,
            None => {}
        }
        match map.get("max_tokens").and_then(|v| v.as_u64()) {
            Some(tokens) => cfg.agents.defaults.max_tokens = tokens as usize,
            None => {}
        }
        match map.get("max_messages").and_then(|v| v.as_u64()) {
            Some(max_msg) => cfg.agents.defaults.max_messages = max_msg as usize,
            None => {}
        }
        match map.get("max_tool_iterations").and_then(|v| v.as_u64()) {
            Some(max_iter) => cfg.agents.defaults.max_tool_iterations = max_iter as usize,
            None => {}
        }
        match map.get("caveman_mode").and_then(|v| v.as_bool()) {
            Some(caveman) => cfg.agents.defaults.caveman_mode = caveman,
            None => {}
        }
        match map.get("tool_timeout_secs").and_then(|v| v.as_u64()) {
            Some(timeout) => cfg.agents.defaults.tool_timeout_secs = timeout,
            None => {}
        }
        match map.get("streaming").and_then(|v| v.as_bool()) {
            Some(streaming) => cfg.agents.defaults.streaming = streaming,
            None => {}
        }
    };

    match metadata.get("config_override").and_then(|v| v.as_object()) {
        Some(nested) => apply_map(config, nested),
        None => match metadata.get("config").and_then(|v| v.as_object()) {
            Some(nested) => apply_map(config, nested),
            None => {}
        },
    }
    apply_map(config, metadata);
}

pub async fn save_session_overrides(
    session_manager: &SessionManager,
    session_key: &str,
    overrides: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    match overrides.is_empty() {
        true => Ok(()),
        false => {
            let mut session = session_manager.get_or_create_async(session_key).await;
            for (k, v) in overrides {
                session.metadata.insert(k.clone(), v.clone());
            }
            session_manager.save(&session).await
        }
    }
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
    latest_config.agents.defaults.max_tool_iterations =
        runtime_config.agents.defaults.max_tool_iterations;

    match session_metadata {
        Some(metadata) => apply_session_overrides(&mut latest_config, metadata),
        None => {}
    }
    latest_config
}

fn get_session_lock(session_key: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let map = SESSION_LOCKS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    match guard.len() > 100 {
        true => guard.retain(|_, arc| std::sync::Arc::strong_count(arc) > 1),
        false => {}
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

fn fallback_models_for_turn(config: &Config, active_model_override: Option<&str>) -> Vec<String> {
    let max_attempts = max_fallback_attempts();
    match max_attempts {
        0 => return Vec::new(),
        _ => {}
    }

    let active_model = active_model_override
        .unwrap_or(&config.agents.defaults.model)
        .trim();
    let mut fallbacks = Vec::new();
    for fallback in &config.agents.defaults.fallback_models {
        let model = match fallback
            .as_str()
            .map(str::trim)
            .filter(|model| !model.is_empty())
        {
            Some(m) => m,
            None => continue,
        };
        let duplicate = model.eq_ignore_ascii_case(active_model)
            || fallbacks
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(model));
        match duplicate {
            true => continue,
            false => {}
        }
        fallbacks.push(model.to_string());
        match fallbacks.len() >= max_attempts {
            true => break,
            false => {}
        }
    }
    fallbacks
}

async fn wait_for_turn_cancellation() {
    match current_turn_cancellation_context() {
        Some(context) => context.token.wait_for_cancellation().await,
        None => std::future::pending::<()>().await,
    }
}

async fn wait_for_cli_cancel_since(initial: u64) {
    let mut rx = crate::shutdown::cli_cancel_tx().subscribe();
    while *rx.borrow() == initial {
        match rx.changed().await.is_err() {
            true => break,
            false => {}
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
        match self.tools.context {
            Some(ref mut ctx) => {
                ctx.0 = config;
                ctx.1 = provider;
            }
            None => {}
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
            match dir.exists() {
                false => continue,
                true => {}
            }
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let remove_expired = entry
                            .metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|modified| now.duration_since(modified).ok())
                            .map(|age| age > max_age)
                            .unwrap_or(false);
                        match remove_expired {
                            true => {
                                let _ = std::fs::remove_file(entry.path());
                            }
                            false => {}
                        }
                    }
                }
                Err(_) => {}
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

        let active_model_str = self.config.agents.defaults.model.as_str();

        match chat_result {
            Ok(_) => {}
            Err(ref err) => {
                let provider_name = self.config.agents.defaults.provider.as_str();
                let risk = crate::channels::classify_model_risk(provider_name, active_model_str);
                let _ = crate::model_registry::record_model_failure(
                    provider_name,
                    active_model_str,
                    risk.tier,
                    risk.risky,
                    risk.reasons
                        .iter()
                        .map(|reason| reason.to_string())
                        .collect(),
                    &err.to_string(),
                );
                match is_cancelled() {
                    true => return Err(anyhow::anyhow!("Cancelled by user")),
                    false => {}
                }

                let fallbacks = fallback_models_for_turn(&self.config, Some(active_model_str));
                let mut resolved_fallback = false;
                for fallback_model in fallbacks {
                    match is_cancelled() {
                        true => return Err(anyhow::anyhow!("Cancelled by user")),
                        false => {}
                    }
                    let silent = crate::agent::style::spinner::is_silent();
                    match silent {
                        false => {
                            crate::tui_println!(
                                "{}▲ Primary provider failed. Attempting fallback model: {}{}",
                                AURA_GOLD,
                                fallback_model,
                                COLOR_RESET
                            );
                        }
                        true => {}
                    }
                    match crate::tools::subagent::build_provider_for_model(
                        &self.config,
                        &fallback_model,
                    ) {
                        Ok(fallback_provider) => {
                            let chat_fut =
                                fallback_provider.chat(system_prompt, messages, tools, settings);
                            chat_result = cancel_aware_provider_call(
                                activity_msg,
                                cwf_cancel_initial,
                                provider_timeout_secs,
                                chat_fut,
                            )
                            .await;
                            match chat_result.is_ok() {
                                true => {
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
                                false => {}
                            }
                        }
                        Err(_) => {}
                    }
                }

                match !resolved_fallback && is_cancelled() {
                    true => return Err(anyhow::anyhow!("Cancelled by user")),
                    false => {}
                }
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

        let active_model_str = self.config.agents.defaults.model.as_str();

        match chat_result {
            Ok(_) => {}
            Err(ref err) => {
                let provider_name = self.config.agents.defaults.provider.as_str();
                let risk = crate::channels::classify_model_risk(provider_name, active_model_str);
                let _ = crate::model_registry::record_model_failure(
                    provider_name,
                    active_model_str,
                    risk.tier,
                    risk.risky,
                    risk.reasons
                        .iter()
                        .map(|reason| reason.to_string())
                        .collect(),
                    &err.to_string(),
                );
                let fallbacks = fallback_models_for_turn(&self.config, Some(active_model_str));

                let mut resolved_fallback = false;
                for fallback_model in fallbacks {
                    match is_cancelled() {
                        true => return Err(anyhow::anyhow!("Cancelled by user")),
                        false => {}
                    }
                    let silent = crate::agent::style::spinner::is_silent();
                    match silent {
                        false => {
                            crate::tui_println!(
                                "{}▲ Primary provider failed. Attempting fallback model: {}{}",
                                AURA_GOLD,
                                fallback_model,
                                COLOR_RESET
                            );
                        }
                        true => {}
                    }
                    match crate::tools::subagent::build_provider_for_model(
                        &self.config,
                        &fallback_model,
                    ) {
                        Ok(fallback_provider) => {
                            let chat_fut = fallback_provider
                                .chat_stream(system_prompt, messages, tools, settings);
                            chat_result = cancel_aware_provider_call(
                                activity_msg,
                                csf_cancel_initial,
                                provider_timeout_secs,
                                chat_fut,
                            )
                            .await;
                            match chat_result.is_ok() {
                                true => {
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
                                false => {}
                            }
                        }
                        Err(_) => {}
                    }
                }

                match !resolved_fallback && is_cancelled() {
                    true => return Err(anyhow::anyhow!("Cancelled by user")),
                    false => {}
                }
            }
        }

        match chat_result {
            Ok(stream) => Ok(Box::pin(crate::providers::openai::filter_think_stream(stream))),
            Err(e) => Err(e),
        }
    }

    pub async fn run(&self, user_content: &str, session_key: &str) -> Result<RunResult> {
        let lock = get_session_lock(session_key);
        let _guard = lock.lock().await;

        let parent_key = crate::agent::style::spinner::get_current_session_key();
        let target_key = match parent_key {
            Some(ref pk) => match pk.starts_with("subagent:") {
                false => pk.clone(),
                true => session_key.to_string(),
            },
            None => session_key.to_string(),
        };

        let is_cli = target_key.starts_with("cli:")
            && (!session_key.starts_with("subagent:") || crate::shutdown::is_cli_active());
        let is_ratatui = crate::channels::ratatui::is_ratatui_session_active(Some(&target_key));
        let is_headless = target_key.starts_with("cli:headless")
            || crate::cli::headless::current_headless_policy().is_some();
        let silent =
            !is_cli || is_ratatui || is_headless || crate::agent::style::spinner::is_silent();

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
            match crate::config::loader::load_config() {
                Ok(latest_config) => {
                    let old_provider = ctx.config.agents.defaults.provider.clone();
                    let old_model = ctx.config.agents.defaults.model.clone();
                    let session_metadata = match state == TurnState::Restore {
                        true => None,
                        false => Some(&ctx.session.metadata),
                    };
                    let merged = merge_latest_config_for_runtime(
                        &ctx.config,
                        latest_config,
                        session_key,
                        session_metadata,
                    );
                    let provider_changed = merged.agents.defaults.provider != old_provider
                        || merged.agents.defaults.model != old_model;
                    match provider_changed {
                        true => match crate::providers::resolver::resolve_provider_full(
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
                        },
                        false => {}
                    }
                    ctx.config = merged;
                }
                Err(_) => {}
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
        match cleaned_tasks > 0 {
            true => tracing::debug!(cleaned_tasks, "Cleaned OpenZ turn-scoped tasks"),
            false => {}
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
