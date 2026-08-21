use crate::config::schema::Config;
use crate::providers::LLMProvider;
use std::sync::Arc;

tokio::task_local! {
    pub static DELEGATION_DEPTH: usize;
    pub static ACTIVE_SUBAGENT: String;
    pub static ORCHESTRATED_NESTED_DELEGATION_ALLOWED: bool;
}

pub mod cancellation_token;
pub mod delegate_profile;
pub mod delegate_task;
pub mod evaluator_optimizer;
pub mod lifecycle;
pub mod optimize_profile;
pub mod parallel_research;
pub mod schema_retry;

#[cfg(test)]
mod tests;

pub use cancellation_token::CancellationToken;
pub use delegate_profile::DelegateProfileTool;
pub use delegate_task::{DelegateTaskTool, cleanup_registered_worktrees, cleanup_stale_resources};
pub use evaluator_optimizer::EvaluatorOptimizerLoopTool;
pub use lifecycle::{
    SubagentRunStatus, cancellation_result_json, classify_subagent_error, compact_lifecycle_line,
    status_json,
};
pub use optimize_profile::{CreateSubagentTool, DeleteSubagentTool, OptimizeSubagentTool};
pub use parallel_research::ParallelResearchTool;

pub fn can_spawn_nested_subagents(profile_name: &str) -> bool {
    matches!(
        profile_name,
        "planner" | "sop_designer" | "openz_coordinator"
    )
}

pub fn nested_delegation_allowed_for_active_context(profile_name: &str) -> bool {
    if let Ok(allowed) = ORCHESTRATED_NESTED_DELEGATION_ALLOWED.try_with(|allowed| *allowed) {
        return allowed;
    }
    can_spawn_nested_subagents(profile_name)
}

pub(crate) fn filesystem_write_denied_by_policy(
    policy: &Option<crate::orchestrator::spec::CapabilityPolicy>,
) -> bool {
    policy
        .as_ref()
        .map(|policy| policy.deny_filesystem_write)
        .unwrap_or(false)
}

/// Cancels the subagent token if the owning tool future is dropped before the
/// initial child run completes (panic, early return, forced shutdown).
pub(crate) struct CancelOnDrop {
    pub(crate) token: CancellationToken,
    pub(crate) completed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if !self.completed {
            self.token.cancel();
        }
    }
}

/// Result of attempting to set up an isolated workspace for a subagent run.
pub(crate) struct WorkspaceIsolation {
    pub(crate) dir: std::path::PathBuf,
    /// "isolated_worktree" | "scratch_workspace" | "fallback_active_workspace"
    pub(crate) label: String,
    pub(crate) reason: Option<String>,
}

/// Attempt to create an isolated workspace (worktree, scratch fallback, or
/// active-workspace fallback) for a subagent run. Shared by delegate_task and
/// delegate_profile; prints the same status lines both tools printed inline.
pub(crate) async fn create_workspace_isolation(parent_dir: &std::path::Path) -> WorkspaceIsolation {
    let parent_dir_clone = parent_dir.to_path_buf();
    let workspace_res = tokio::task::spawn_blocking(move || {
        delegate_task::create_isolated_workspace(&parent_dir_clone)
    })
    .await;
    match workspace_res {
        Ok(Ok(dir)) => {
            if delegate_task::is_scratch_workspace(&dir) {
                let reason = Some(format!(
                    "Active workspace '{}' is unsafe to copy; using an empty scratch workspace with no sync-back.",
                    parent_dir.display()
                ));
                crate::tui_println!(
                    "{}  ✓ Scratch subagent workspace created at {:?}{}",
                    crate::agent::style::EMERALD_GREEN,
                    dir,
                    crate::agent::style::COLOR_RESET
                );
                WorkspaceIsolation {
                    dir,
                    label: "scratch_workspace".to_string(),
                    reason,
                }
            } else {
                crate::tui_println!(
                    "{}  ✓ Isolated workspace worktree created at {:?}{}",
                    crate::agent::style::EMERALD_GREEN,
                    dir,
                    crate::agent::style::COLOR_RESET
                );
                WorkspaceIsolation {
                    dir,
                    label: "isolated_worktree".to_string(),
                    reason: None,
                }
            }
        }
        Ok(Err(e)) => {
            let reason = e.to_string();
            crate::tui_println!(
                "{}⚠️  Failed to create isolated workspace ({}). Running in active workspace without isolation.{}",
                crate::agent::style::AURA_GOLD,
                reason,
                crate::agent::style::COLOR_RESET
            );
            WorkspaceIsolation {
                dir: parent_dir.to_path_buf(),
                label: "fallback_active_workspace".to_string(),
                reason: Some(reason),
            }
        }
        Err(e) => {
            let reason = format!("join error: {:?}", e);
            crate::tui_println!(
                "{}⚠️  Failed to create isolated workspace ({}). Running in active workspace without isolation.{}",
                crate::agent::style::AURA_GOLD,
                reason,
                crate::agent::style::COLOR_RESET
            );
            WorkspaceIsolation {
                dir: parent_dir.to_path_buf(),
                label: "fallback_active_workspace".to_string(),
                reason: Some(reason),
            }
        }
    }
}

/// Attach the workspace-isolation outcome fields to a cancellation result.
pub(crate) fn attach_workspace_fields(
    mut json: serde_json::Value,
    workspace_isolation: &str,
    workspace_isolation_reason: &Option<String>,
) -> serde_json::Value {
    if let Some(obj) = json.as_object_mut() {
        obj.insert(
            "workspaceIsolation".to_string(),
            serde_json::Value::String(workspace_isolation.to_string()),
        );
        obj.insert(
            "workspaceIsolationReason".to_string(),
            workspace_isolation_reason
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        );
    }
    json
}

pub fn subagent_tool_metadata(name: &str) -> crate::tools::ToolMetadata {
    let mut metadata = crate::tools::ToolMetadata::infer(name);
    metadata.domain = "subagent";
    metadata.risk = crate::tools::ToolRisk::Medium;
    metadata.spawns_process = false;
    metadata.requires_approval = false;
    metadata.priority = 100;
    metadata.recommended_timeout_secs = Some(600);
    metadata
}

pub fn max_subagent_model_attempts() -> usize {
    let fallback_attempts = std::env::var("OPENZ_MAX_FALLBACK_ATTEMPTS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(2)
        .min(8);
    1 + fallback_attempts
}

pub fn limit_subagent_models_to_try(models: &mut Vec<String>) {
    models.truncate(max_subagent_model_attempts());
}

pub fn should_skip_evolution_capture(goal: &str, output: &str) -> bool {
    let goal_lower = goal.to_lowercase();
    let output_words = output.split_whitespace().count();
    let smoke_goal = [
        "summarize hello",
        "review planner output",
        "where is orchestrate_workflow implemented",
        "simple two-step workflow",
        "smoke test workflow",
    ]
    .iter()
    .any(|needle| goal_lower.contains(needle));

    smoke_goal || output_words < 24
}

pub fn step_allows_nested_delegation(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    [
        "delegate",
        "subagent",
        "parallel",
        "specialist",
        "orchestrate",
        "workflow",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub fn should_run_evolution_review(
    goal: &str,
    context: &str,
    summary: &str,
    filesystem_write_denied: bool,
) -> bool {
    !filesystem_write_denied
        && !should_skip_evolution_capture(goal, summary)
        && !crate::grounding::should_suppress_evolution(goal, context, summary)
}

pub fn resolve_subagent_timeout_secs(
    requested_timeout_secs: Option<u64>,
    default_timeout_secs: u64,
) -> u64 {
    crate::tools::clamp_tool_timeout_secs(requested_timeout_secs.unwrap_or(default_timeout_secs))
}

fn resolve_provider_for_subagent_model(
    config: &Config,
    model: &str,
) -> anyhow::Result<crate::providers::resolver::ResolvedProvider> {
    let mut subagent_config = config.clone();
    subagent_config.agents.defaults.provider = "auto".to_string();
    crate::providers::resolver::resolve_provider_full(&subagent_config, model)
}

// Shared utility function used across tools:
pub fn build_provider_for_model(
    config: &Config,
    model: &str,
) -> anyhow::Result<Arc<dyn LLMProvider>> {
    let resolved = resolve_provider_for_subagent_model(config, model)?;
    Ok(resolved.instance)
}

pub fn scan_for_images(goal: &str, context: &str) -> Vec<String> {
    let mut image_paths = Vec::new();
    if let Ok(path_regex) =
        regex::Regex::new(r"(?:file://)?(/[a-zA-Z0-9_\-\./]+|~/[a-zA-Z0-9_\-\./]+)")
    {
        for cap in path_regex.captures_iter(&format!("{} {}", goal, context)) {
            if let Some(mat) = cap.get(1) {
                let path_str = mat.as_str();
                let resolved_path = crate::config::resolve_path(path_str);

                let mut final_path = None;
                if resolved_path.exists() && resolved_path.is_file() {
                    final_path = Some(resolved_path);
                } else {
                    for ext in &["png", "jpg", "jpeg", "webp", "gif"] {
                        let path_with_ext = resolved_path.with_extension(ext);
                        if path_with_ext.exists() && path_with_ext.is_file() {
                            final_path = Some(path_with_ext);
                            break;
                        }
                    }
                }

                if let Some(path) = final_path {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if ["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.as_str()) {
                        let canonical = path.to_string_lossy().to_string();
                        if !image_paths.contains(&canonical) {
                            image_paths.push(canonical);
                        }
                    }
                }
            }
        }
    }
    // Fallback to default clipboard image if no specific path was found but task mentions an image
    if image_paths.is_empty() {
        let default_clip = crate::config::resolve_path("~/.openz/clipboard_image_0.png");
        if default_clip.exists() && default_clip.is_file() {
            let text_lower = format!("{} {}", goal, context).to_lowercase();
            if text_lower.contains("image")
                || text_lower.contains("picture")
                || text_lower.contains("screenshot")
            {
                image_paths.push(default_clip.to_string_lossy().to_string());
            }
        }
    }
    image_paths
}

pub async fn execute_subagent_run(
    agent: &crate::agent::AgentLoop,
    prompt: &str,
    session_id: &str,
    subagent_name: &str,
    model_name: &str,
    workspace_dir: std::path::PathBuf,
    current_depth: usize,
    cancellation_token: &CancellationToken,
    timeout_secs: Option<u64>,
    default_timeout_secs: u64,
    spinner_msg: &str,
) -> anyhow::Result<crate::agent::agent_loop::RunResult> {
    let p_ref = prompt;
    let c_ref = session_id;
    let child_agent_ref = agent;
    let subagent_name_str = subagent_name.to_string();
    let model_name_str = model_name.to_string();
    let cancellation_token_clone = cancellation_token.clone();

    let run_res_fut =
        crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir, async move {
            DELEGATION_DEPTH.scope(current_depth + 1, async move {
            crate::tools::subagent::ACTIVE_SUBAGENT.scope(subagent_name_str.clone(), async move {
                tokio::select! {
                    biased;
                    _ = cancellation_token_clone.wait_for_cancellation() => {
                        if !crate::agent::style::is_silent() {
                            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                            let line = compact_lifecycle_line(
                                &subagent_name_str,
                                &model_name_str,
                                &SubagentRunStatus::Cancelling,
                            );
                            crate::tui_println!(
                                "{}{}{}▲ {}{}",
                                crate::agent::style::AURA_SLATE,
                                leaf_prefix,
                                crate::agent::style::AURA_GOLD,
                                line,
                                crate::agent::style::COLOR_RESET
                            );
                        }
                        Err(anyhow::anyhow!("Subagent task cancelled"))
                    }
                    res = child_agent_ref.run(p_ref, c_ref) => res,
                }
            }).await
        }).await
        });

    let sub_timeout = resolve_subagent_timeout_secs(timeout_secs, default_timeout_secs);
    let run_res_timeout =
        tokio::time::timeout(std::time::Duration::from_secs(sub_timeout), run_res_fut);
    match crate::agent::style::with_spinner(spinner_msg, run_res_timeout).await {
        Ok(res) => res,
        Err(_) => Err(anyhow::anyhow!(
            "Subagent execution timed out after {sub_timeout}s"
        )),
    }
}
