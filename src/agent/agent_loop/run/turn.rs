//! Turn-level provider coordination and cross-session locking.

use crate::agent::source_ledger::SourceLedger;
use crate::tools::shared_memory::AutoCaptureSummary;
use anyhow::Result;
use fs2::FileExt;
use std::collections::HashSet;

use super::super::TurnContext;

/// Monotonic append-only tool window for a single user turn.
///
/// Locks the Iteration 0 tool prefix `0..K` in exact position so Anthropic,
/// OpenAI, DeepSeek, and OpenRouter KV prompt caches remain warm across
/// multi-step tool loops while newly unlocked or hot-mounted tools append strictly
/// to the tail.
#[derive(Debug, Clone, Default)]
pub(crate) struct TurnToolWindow {
    ordered_tools: Vec<serde_json::Value>,
    seen_names: HashSet<String>,
}

impl TurnToolWindow {
    #[cfg(test)]
    pub(crate) fn resolve_iteration_tools(
        &mut self,
        fresh_tools: Vec<serde_json::Value>,
        explicit_mount_names: &HashSet<String>,
        preserve_prefix_cache: bool,
        max_expanded_tools: usize,
        max_provider_tools: usize,
    ) -> Vec<serde_json::Value> {
        self.resolve_iteration_tools_for_step(
            0,
            fresh_tools,
            explicit_mount_names,
            preserve_prefix_cache,
            max_expanded_tools,
            max_provider_tools,
        )
    }

    pub(crate) fn resolve_iteration_tools_for_step(
        &mut self,
        iteration: usize,
        fresh_tools: Vec<serde_json::Value>,
        explicit_mount_names: &HashSet<String>,
        preserve_prefix_cache: bool,
        max_expanded_tools: usize,
        max_provider_tools: usize,
    ) -> Vec<serde_json::Value> {
        let provider_cap = max_provider_tools.max(1);
        let phase_headroom = (iteration / 4).saturating_mul(8);
        let expanded_cap = max_expanded_tools
            .max(1)
            .saturating_add(phase_headroom)
            .min(provider_cap);
        match preserve_prefix_cache {
            false => {
                let mut direct = fresh_tools;
                direct.truncate(provider_cap);
                direct
            }
            true => {
                match self.ordered_tools.is_empty() {
                    true => {
                        for schema in fresh_tools.into_iter().take(provider_cap) {
                            let name = schema["function"]["name"]
                                .as_str()
                                .unwrap_or("")
                                .to_ascii_lowercase();
                            let inserted = (!name.is_empty()) && self.seen_names.insert(name);
                            let _ = inserted.then(|| self.ordered_tools.push(schema));
                        }
                    }
                    false => {
                        let mut prioritized_candidates: Vec<(bool, usize, String, serde_json::Value)> =
                            fresh_tools
                                .into_iter()
                                .enumerate()
                                .filter_map(|(idx, schema)| {
                                    let name = schema["function"]["name"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_ascii_lowercase();
                                    let unseen = (!name.is_empty()) && (!self.seen_names.contains(&name));
                                    unseen.then(|| {
                                        let is_explicit = explicit_mount_names.contains(&name);
                                        (!is_explicit, idx, name, schema)
                                    })
                                })
                                .collect();
                        prioritized_candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                        for (not_explicit, _, name, schema) in prioritized_candidates {
                            let is_explicit = !not_explicit;
                            let target_cap = match is_explicit {
                                true => provider_cap,
                                false => expanded_cap.max(self.ordered_tools.len()).min(provider_cap),
                            };
                            let has_room = self.ordered_tools.len() < target_cap;
                            let _ = (has_room && self.seen_names.insert(name))
                                .then(|| self.ordered_tools.push(schema));
                        }
                    }
                }
                self.ordered_tools.clone()
            }
        }
    }
}

/// State that belongs to one Run phase and must survive across provider/tool
/// iterations without being mixed into the durable TurnContext.
pub(crate) struct RunContext {
    pub(crate) iterations: usize,
    pub(crate) loop_state: super::super::security_approval::LoopEscalationState,
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
    pub(crate) tool_window: TurnToolWindow,
}

impl RunContext {
    pub(crate) fn new(ctx: &TurnContext<'_>) -> Self {
        Self {
            iterations: 0,
            loop_state: super::super::security_approval::LoopEscalationState::default(),
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
            tool_window: TurnToolWindow::default(),
        }
    }
}

pub(super) fn build_iteration_routing_prompt(
    user_content: &str,
    messages: &[crate::session::Message],
    tools_used: &[String],
    turn_errors: &[String],
    iteration: usize,
) -> String {
    match iteration == 0 {
        true => user_content.to_string(),
        false => {
            let mut parts = vec![user_content.to_string()];
            let latest_assistant = messages.iter().rev().find(|msg| msg.role == "assistant");
            let _ = latest_assistant.map(|msg| {
                let content_trimmed = msg.content.trim();
                let _ = (!content_trimmed.is_empty()).then(|| {
                    parts.push(content_trimmed.chars().take(400).collect::<String>());
                });
                let _ = msg
                    .extra
                    .get("reasoning_content")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|reasoning| {
                        parts.push(reasoning.chars().take(400).collect::<String>());
                    });
            });
            let recent_tools: Vec<&str> = tools_used
                .iter()
                .rev()
                .take(6)
                .map(String::as_str)
                .collect();
            let _ = (!recent_tools.is_empty()).then(|| {
                parts.push(recent_tools.join(" "));
            });
            let recent_errors: Vec<String> = turn_errors
                .iter()
                .rev()
                .take(2)
                .map(|err| err.chars().take(240).collect::<String>())
                .collect();
            let _ = (!recent_errors.is_empty()).then(|| {
                parts.push(recent_errors.join("\n"));
            });
            parts.join("\n")
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

#[cfg(test)]
mod tests {
    use super::TurnToolWindow;
    use std::collections::HashSet;

    fn mock_tool(name: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": name,
                "description": format!("Mock {name}"),
                "parameters": { "type": "object", "properties": {} }
            }
        })
    }

    fn extract_names(tools: &[serde_json::Value]) -> Vec<String> {
        tools
            .iter()
            .filter_map(|t| t["function"]["name"].as_str().map(String::from))
            .collect()
    }

    #[test]
    fn turn_tool_window_locks_iteration_zero_prefix_and_appends_new_tools_monotonically() {
        let mut window = TurnToolWindow::default();
        let no_mounts = HashSet::new();

        let iter0 = window.resolve_iteration_tools(
            vec![
                mock_tool("read_file"),
                mock_tool("tool_catalog"),
                mock_tool("patch_file"),
            ],
            &no_mounts,
            true,
            8,
            128,
        );
        assert_eq!(extract_names(&iter0), vec!["read_file", "tool_catalog", "patch_file"]);

        // Iteration 1: router drops patch_file and reorders, while unlocking cargo_manager and opendoc_create_xlsx
        let mut explicit = HashSet::new();
        explicit.insert("opendoc_create_xlsx".to_string());
        let iter1 = window.resolve_iteration_tools(
            vec![
                mock_tool("cargo_manager"),
                mock_tool("tool_catalog"),
                mock_tool("opendoc_create_xlsx"),
            ],
            &explicit,
            true,
            4,
            128,
        );

        // Prefix [0..3] stays 100% identical for KV-cache reuse; explicit mount appends cleanly!
        assert_eq!(
            extract_names(&iter1),
            vec!["read_file", "tool_catalog", "patch_file", "opendoc_create_xlsx"]
        );

        // Late-phase multi-step turn (iteration 8): phase headroom unlocks phase-3/4 routed tools beyond initial 4-tool expanded cap
        let iter8 = window.resolve_iteration_tools_for_step(
            8,
            vec![
                mock_tool("git_manager"),
                mock_tool("browser"),
            ],
            &no_mounts,
            true,
            4,
            128,
        );
        assert_eq!(
            extract_names(&iter8),
            vec![
                "read_file",
                "tool_catalog",
                "patch_file",
                "opendoc_create_xlsx",
                "git_manager",
                "browser"
            ]
        );
    }
}
