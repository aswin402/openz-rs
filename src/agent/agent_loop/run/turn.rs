//! Turn-level provider coordination and cross-session locking.

use crate::agent::source_ledger::SourceLedger;
use crate::tools::shared_memory::AutoCaptureSummary;
use anyhow::Result;
use fs2::FileExt;
use std::collections::HashSet;

use super::super::TurnContext;

/// State that belongs to one Run phase and must survive across provider/tool
/// iterations without being mixed into the durable TurnContext.
pub(crate) struct RunContext {
    pub(crate) iterations: usize,
    pub(crate) loop_blocked_count: usize,
    pub(crate) max_iterations: usize,
    pub(crate) turn_capture_summaries: Vec<AutoCaptureSummary>,
    pub(crate) turn_source_ledger: SourceLedger,
    pub(crate) completed_direct_research_urls: HashSet<String>,
    pub(crate) auto_scoped_edit_paths: HashSet<String>,
    pub(crate) auto_opened_artifact_paths: HashSet<String>,
    pub(crate) auto_suggested_open_targets: HashSet<String>,
    pub(crate) direct_page_only: bool,
    pub(crate) direct_page_fetched: bool,
    pub(crate) turn_cancel: crate::tools::subagent::CancellationToken,
}

impl RunContext {
    pub(crate) fn new(ctx: &TurnContext<'_>) -> Self {
        Self {
            iterations: 0,
            loop_blocked_count: 0,
            max_iterations: ctx.config.agents.defaults.max_tool_iterations,
            turn_capture_summaries: Vec::new(),
            turn_source_ledger: SourceLedger::default(),
            completed_direct_research_urls: HashSet::new(),
            auto_scoped_edit_paths: HashSet::new(),
            auto_opened_artifact_paths: HashSet::new(),
            auto_suggested_open_targets: HashSet::new(),
            direct_page_only: super::research::direct_page_research_only(ctx.user_content),
            direct_page_fetched: false,
            turn_cancel: super::super::current_turn_cancellation_context()
                .map(|context| context.token)
                .unwrap_or_default(),
        }
    }
}

fn provider_turn_lock_key(provider: &str, model: &str) -> Option<String> {
    let mode = std::env::var("OPENZ_PROVIDER_TURN_LOCK").ok();
    provider_turn_lock_key_for_mode(provider, model, mode.as_deref())
}

pub(super) fn provider_turn_lock_key_for_mode(
    provider: &str,
    model: &str,
    mode: Option<&str>,
) -> Option<String> {
    let mode = mode.map(str::trim).filter(|value| !value.is_empty())?;
    let mode = mode.to_lowercase();
    (!matches!(mode.as_str(), "off" | "false" | "0")).then_some(())?;

    let provider_lower = provider.to_lowercase();
    let model_lower = model.to_lowercase();
    let fragile = provider_lower == "opencode_zen"
        || provider_lower == "opencode-zen"
        || model_lower.contains(":free")
        || model_lower.ends_with("-free")
        || model_lower.contains("flash-free");
    let should_lock = mode == "all"
        || (matches!(mode.as_str(), "fragile" | "free" | "on" | "true") && fragile);
    should_lock.then_some(())?;

    let raw = format!("{}__{}", provider_lower, model_lower);
    let slug = raw
        .chars()
        .map(|ch| ch.is_ascii_alphanumeric().then_some(ch).unwrap_or('_'))
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    Some(
        (!slug.is_empty())
            .then_some(slug)
            .unwrap_or_else(|| "provider".to_string()),
    )
}

pub(super) async fn acquire_provider_turn_lock(
    session_key: &str,
    provider: &str,
    model: &str,
) -> Result<Option<std::fs::File>> {
    let key = match provider_turn_lock_key(provider, model) {
        Some(key) => key,
        None => return Ok(None),
    };
    let dir = crate::config::loader::runtime_data_dir().join("provider_turn_locks");
    let path = dir.join(format!("{key}.lock"));
    let session_key = session_key.to_string();
    let provider = provider.to_string();
    let model = model.to_string();
    tokio::task::spawn_blocking(move || -> Result<Option<std::fs::File>> {
        std::fs::create_dir_all(&dir)?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        let mut delay = std::time::Duration::from_millis(100);
        let started = std::time::Instant::now();
        let mut announced_wait = false;
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Some(file)),
                Err(err) => match err.kind() == std::io::ErrorKind::WouldBlock {
                    false => return Err(err.into()),
                    true => {
                        let _ = (!announced_wait).then(|| {
                            crate::agent::activity::update_activity(
                                &session_key,
                                "Waiting for provider slot",
                                Some(&format!("{} / {}", provider, model)),
                            );
                            announced_wait = true;
                        });
                        match started.elapsed() > std::time::Duration::from_secs(120) {
                            true => anyhow::bail!(
                                "Provider '{}' model '{}' is still busy in another OpenZ session after 120s",
                                provider,
                                model
                            ),
                            false => {
                                std::thread::sleep(delay);
                                delay = std::cmp::min(
                                    delay.saturating_mul(2),
                                    std::time::Duration::from_secs(1),
                                );
                            }
                        }
                    }
                },
            }
        }
    })
    .await?
}
