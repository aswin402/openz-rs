use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::tools::Tool;
use serde_json::Value;
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
pub use delegate_task::{cleanup_registered_worktrees, cleanup_stale_resources, DelegateTaskTool};
pub use evaluator_optimizer::EvaluatorOptimizerLoopTool;
pub use lifecycle::{
    cancellation_result_json, classify_subagent_error, compact_lifecycle_line, status_json,
    SubagentRunStatus,
};
pub use optimize_profile::{
    CreateSubagentTool, DeleteSubagentTool, OptimizeSubagentTool, UpdateSubagentSettingsTool,
};
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
    /// | "policy_no_filesystem_write" | "not_required"
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

/// Resolve the workspace policy shared by all subagent entry points. The
/// caller decides whether a profile needs isolation; this helper owns the
/// policy/fallback labels and keeps their result shape consistent.
pub(crate) async fn prepare_workspace(
    parent_dir: &std::path::Path,
    filesystem_write_denied: bool,
    needs_workspace: bool,
) -> WorkspaceIsolation {
    if filesystem_write_denied {
        return WorkspaceIsolation {
            dir: parent_dir.to_path_buf(),
            label: "policy_no_filesystem_write".to_string(),
            reason: Some(
                "Capability policy denies filesystem writes; running without workspace isolation, graph branches, or sync-back."
                    .to_string(),
            ),
        };
    }

    if !needs_workspace {
        return WorkspaceIsolation {
            dir: parent_dir.to_path_buf(),
            label: "not_required".to_string(),
            reason: None,
        };
    }

    create_workspace_isolation(parent_dir).await
}

/// Create the temporary database branch used by subagent runs when writes are
/// allowed. Returning the ID keeps branch ownership with the caller while
/// centralizing generation and native-tool invocation.
pub(crate) async fn create_simulation_branch(
    enabled: bool,
) -> anyhow::Result<Option<String>> {
    if !enabled {
        return Ok(None);
    }

    let branch_id = format!("branch_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    crate::tools::graph_memory::CreateDatabaseBranchTool
        .call(&serde_json::json!({ "branchId": branch_id }))
        .await?;
    Ok(Some(branch_id))
}

/// Finalize a temporary database branch and optionally preserve the task-level
/// simulation-space announcement. Profile fallback attempts intentionally use
/// `announce = false` to retain their quieter existing output.
pub(crate) async fn finish_simulation_branch(
    branch_id: &str,
    run_success: bool,
    scratch_workspace: bool,
    announce: bool,
) -> anyhow::Result<()> {
    if run_success {
        crate::tools::graph_memory::CommitDatabaseBranchTool
            .call(&serde_json::json!({}))
            .await?;
    } else {
        crate::tools::graph_memory::RollbackDatabaseBranchTool
            .call(&serde_json::json!({}))
            .await?;
    }

    if announce {
        let message = delegate_task::simulation_space_teardown_message(
            run_success,
            branch_id,
            scratch_workspace,
        );
        let color = if run_success {
            crate::agent::style::EMERALD_GREEN
        } else {
            crate::agent::style::AURA_GOLD
        };
        crate::tui_println!(
            "{}  ✓ {}{}",
            color,
            message,
            crate::agent::style::COLOR_RESET
        );
    }

    Ok(())
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

/// Finalize a temporary database branch, handling errors safely and optionally
/// announcing the teardown.
pub(crate) async fn finalize_simulation_branch(
    branch_id: Option<&str>,
    run_success: bool,
    workspace_dir: &std::path::Path,
    announce: bool,
) {
    if let Some(branch_id) = branch_id {
        if let Err(e) = finish_simulation_branch(
            branch_id,
            run_success,
            delegate_task::is_scratch_workspace(workspace_dir),
            announce,
        )
        .await
        {
            tracing::warn!("Failed to finalize database branch: {:?}", e);
        }
    }
}

/// Synchronize modified files from the temporary subagent workspace back to the active parent workspace.
pub(crate) fn sync_workspace_changes_back(
    parent_dir: &std::path::Path,
    workspace_dir: &std::path::Path,
    filesystem_write_denied: bool,
) {
    if !filesystem_write_denied && delegate_task::should_sync_changes_back(parent_dir, workspace_dir) {
        if let Err(e) = delegate_task::sync_changes_back(workspace_dir, parent_dir) {
            if !crate::agent::style::is_silent() {
                let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                crate::tui_println!(
                    "{}{}{}↶ Failed to sync changes back to active workspace: {}{}",
                    crate::agent::style::AURA_SLATE,
                    leaf_prefix,
                    crate::agent::style::AURA_GOLD,
                    e,
                    crate::agent::style::COLOR_RESET
                );
            }
        } else if !crate::agent::style::is_silent() {
            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
            crate::tui_println!(
                "{}{}{}✓ Synchronized changes back to active workspace{}",
                crate::agent::style::AURA_SLATE,
                leaf_prefix,
                crate::agent::style::AURA_GREEN,
                crate::agent::style::COLOR_RESET
            );
        }
    }
}

/// Classifies an error and formats cancellation JSON if the subagent was cancelled.
pub(crate) fn handle_subagent_cancellation(
    tool_name: &str,
    profile_name: Option<&str>,
    model_name: &str,
    session_id: &str,
    error_text: &str,
    token: &CancellationToken,
    workspace_isolation: &str,
    workspace_isolation_reason: &Option<String>,
) -> Option<serde_json::Value> {
    let lifecycle = classify_subagent_error(error_text, token);
    if matches!(lifecycle, SubagentRunStatus::Cancelled) {
        if !crate::agent::style::is_silent() {
            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
            let target_name = profile_name.unwrap_or(tool_name);
            let line = compact_lifecycle_line(target_name, model_name, &lifecycle);
            crate::tui_println!(
                "{}{}{}▲ {}{}",
                crate::agent::style::AURA_SLATE,
                leaf_prefix,
                crate::agent::style::AURA_GOLD,
                line,
                crate::agent::style::COLOR_RESET
            );
        }
        let cancelled = attach_workspace_fields(
            cancellation_result_json(
                tool_name,
                profile_name,
                session_id,
                model_name,
                error_text,
            ),
            workspace_isolation,
            workspace_isolation_reason,
        );
        Some(cancelled)
    } else {
        None
    }
}

/// Formats the success response, renders lifecycle completion line, and runs evolution review.
pub(crate) async fn handle_subagent_success(
    parent_provider: &std::sync::Arc<dyn crate::providers::LLMProvider>,
    tool_or_profile_name: &str,
    model_name: &str,
    session_id: &str,
    content: &str,
    goal: &str,
    context: &str,
    filesystem_write_denied: bool,
    workspace_isolation: &str,
    workspace_isolation_reason: &Option<String>,
) -> serde_json::Value {
    if !crate::agent::style::is_silent() {
        let leaf_prefix = crate::agent::style::get_tree_prefix(true);
        let summary = crate::agent::style::format_subagent_summary(content);
        let line = compact_lifecycle_line(
            tool_or_profile_name,
            model_name,
            &SubagentRunStatus::Completed,
        );
        crate::tui_println!(
            "{}{}{}✓ {} - {}{}",
            crate::agent::style::AURA_SLATE,
            leaf_prefix,
            crate::agent::style::AURA_GREEN,
            line,
            summary,
            crate::agent::style::COLOR_RESET
        );
    }

    if should_run_evolution_review(goal, context, content, filesystem_write_denied) {
        let _ = delegate_task::run_evolution_review(
            parent_provider,
            tool_or_profile_name,
            goal,
            context,
            content,
        )
        .await;
    }

    let result = serde_json::json!({
        "status": "success",
        "lifecycle": status_json(&SubagentRunStatus::Completed),
        "session_id": session_id,
        "model_used": model_name,
        "summary": content
    });
    attach_workspace_fields(result, workspace_isolation, workspace_isolation_reason)
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

    smoke_goal || output_words < 18
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
        let default_clip = crate::config::runtime_data_dir().join("clipboard_image_0.png");
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

pub fn ensure_markdown_images(text: &str) -> String {
    let re = match regex::Regex::new(
        r"(?i)(file://[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif)|https?://[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif)|/[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif))",
    ) {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };

    let mut result = text.to_string();
    let mut matches: Vec<_> = re.find_iter(text).collect();
    matches.reverse();

    for mat in matches {
        let start = mat.start();
        let end = mat.end();
        let matched_str = mat.as_str();

        let mut already_formatted = false;
        if start > 0 {
            let before = &text[..start];
            if before.ends_with('(') || before.ends_with("](") {
                already_formatted = true;
            }
        }

        if !already_formatted {
            let replacement = format!("![]({})", matched_str);
            result.replace_range(start..end, &replacement);
        }
    }
    result
}

pub fn build_subagent_prompt(
    base_instructions: &str,
    clean_goal: &str,
    clean_context: &str,
    json_schema: Option<&Value>,
) -> String {
    let mut prompt = format!(
        "{base_instructions}\n\n\
        TASK:\n{clean_goal}\n\n\
        CONTEXT:\n{clean_context}\n\n\
        When finished, provide a clear, concise summary of what you did and found."
    );

    let image_paths = scan_for_images(clean_goal, clean_context);
    for img in image_paths {
        prompt.push_str(&format!(" ![](file://{img})"));
    }

    if let Some(schema) = json_schema {
        prompt.push_str(&format!(
            "\n\nCRITICAL REQUIREMENT: Your final response MUST be a raw JSON object strictly conforming to this JSON Schema:\n{}\nDo not wrap it in markdown code blocks, do not add any conversational text. Return only the raw valid JSON.",
            serde_json::to_string_pretty(schema).unwrap_or_default()
        ));
    }

    prompt
}

pub struct SubagentRunAttempt<'a> {
    pub tool_name: &'a str,
    pub profile_name: Option<&'a str>,
    pub subagent_name: &'a str,
    pub model_name: &'a str,
    pub child_session_id: &'a str,
    pub prompt: &'a str,
    pub clean_goal: &'a str,
    pub clean_context: &'a str,
    pub current_depth: usize,
    pub timeout_secs: Option<u64>,
    pub default_timeout_secs: u64,
    pub spinner_msg: &'a str,
    pub json_schema: Option<&'a Value>,
    pub parent_dir: &'a std::path::Path,
    pub workspace_dir: std::path::PathBuf,
    pub filesystem_write_denied: bool,
    pub workspace_isolation: &'a str,
    pub workspace_isolation_reason: &'a Option<String>,
    pub announce_branch: bool,
}

#[derive(Debug)]
pub enum SubagentRunOutcome {
    Success(Value),
    Cancelled(Value),
    Failed {
        error: anyhow::Error,
        response_json: Value,
    },
}

pub async fn run_subagent_attempt(
    child_agent: &crate::agent::AgentLoop,
    parent_provider: &Arc<dyn LLMProvider>,
    cancellation_token: &CancellationToken,
    attempt: SubagentRunAttempt<'_>,
) -> SubagentRunOutcome {
    if cancellation_token.is_cancelled() {
        let cancelled = cancellation_result_json(
            attempt.tool_name,
            attempt.profile_name,
            attempt.child_session_id,
            attempt.model_name,
            "Subagent task cancelled",
        );
        let cancelled = attach_workspace_fields(
            cancelled,
            attempt.workspace_isolation,
            attempt.workspace_isolation_reason,
        );
        return SubagentRunOutcome::Cancelled(cancelled);
    }

    let branch_id = if !attempt.filesystem_write_denied {
        match create_simulation_branch(true).await {
            Ok(Some(bid)) => {
                if attempt.announce_branch {
                    crate::tui_println!(
                        "{}  ✓ Isolated simulation space branch '{}' created{}",
                        crate::agent::style::EMERALD_GREEN,
                        bid,
                        crate::agent::style::COLOR_RESET
                    );
                }
                Some(bid)
            }
            Ok(None) => None,
            Err(e) => {
                tracing::warn!("Failed to create database branch: {:?}", e);
                None
            }
        }
    } else {
        None
    };

    let mut cancel_guard = CancelOnDrop {
        token: cancellation_token.clone(),
        completed: false,
    };

    let mut run_res = execute_subagent_run(
        child_agent,
        attempt.prompt,
        attempt.child_session_id,
        attempt.subagent_name,
        attempt.model_name,
        attempt.workspace_dir.clone(),
        attempt.current_depth,
        cancellation_token,
        attempt.timeout_secs,
        attempt.default_timeout_secs,
        attempt.spinner_msg,
    )
    .await;
    cancel_guard.completed = true;

    if let Some(schema) = attempt.json_schema {
        let child_agent = child_agent;
        let session_id = attempt.child_session_id;
        let subagent_name = attempt.subagent_name;
        let model_name = attempt.model_name;
        let workspace_dir = attempt.workspace_dir.clone();
        let current_depth = attempt.current_depth;
        let token = cancellation_token;
        let timeout_secs = attempt.timeout_secs;
        let default_timeout = attempt.default_timeout_secs;
        let spinner = attempt.spinner_msg;

        run_res = schema_retry::execute_with_schema_retries(
            run_res,
            schema,
            |retry_prompt| {
                let workspace = workspace_dir.clone();
                async move {
                    execute_subagent_run(
                        child_agent,
                        &retry_prompt,
                        session_id,
                        subagent_name,
                        model_name,
                        workspace,
                        current_depth,
                        token,
                        timeout_secs,
                        default_timeout,
                        spinner,
                    )
                    .await
                }
            },
        )
        .await;
    }

    finalize_simulation_branch(
        branch_id.as_deref(),
        run_res.is_ok(),
        &attempt.workspace_dir,
        attempt.announce_branch,
    )
    .await;

    match run_res {
        Ok(res) => {
            if let Some(profile) = attempt.profile_name {
                let _ = crate::subagents::record_subagent_success(profile, attempt.model_name);
            }

            sync_workspace_changes_back(
                attempt.parent_dir,
                &attempt.workspace_dir,
                attempt.filesystem_write_denied,
            );

            let success_val = handle_subagent_success(
                parent_provider,
                attempt.subagent_name,
                attempt.model_name,
                attempt.child_session_id,
                &res.content,
                attempt.clean_goal,
                attempt.clean_context,
                attempt.filesystem_write_denied,
                attempt.workspace_isolation,
                attempt.workspace_isolation_reason,
            )
            .await;

            SubagentRunOutcome::Success(success_val)
        }
        Err(e) => {
            let error_text = e.to_string();
            if let Some(cancelled) = handle_subagent_cancellation(
                attempt.tool_name,
                attempt.profile_name,
                attempt.model_name,
                attempt.child_session_id,
                &error_text,
                cancellation_token,
                attempt.workspace_isolation,
                attempt.workspace_isolation_reason,
            ) {
                return SubagentRunOutcome::Cancelled(cancelled);
            }

            let lifecycle = classify_subagent_error(&error_text, cancellation_token);
            if !crate::agent::style::is_silent() {
                let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                let line = compact_lifecycle_line(attempt.subagent_name, attempt.model_name, &lifecycle);
                if attempt.profile_name.is_some() {
                    crate::tui_println!(
                        "{}{}{}✕ {}{}",
                        crate::agent::style::AURA_SLATE,
                        leaf_prefix,
                        crate::agent::style::AURA_ROSE,
                        line,
                        crate::agent::style::COLOR_RESET
                    );
                } else {
                    crate::tui_println!(
                        "{}{}{}✗{} {}{}",
                        crate::agent::style::AURA_SLATE,
                        leaf_prefix,
                        crate::agent::style::COLOR_RESET,
                        crate::agent::style::ERROR_RED,
                        line,
                        crate::agent::style::COLOR_RESET
                    );
                }
            }

            if let Some(profile) = attempt.profile_name {
                let _ = crate::subagents::record_subagent_failure(profile, &error_text);
            }

            let response_json = serde_json::json!({
                "status": "error",
                "lifecycle": status_json(&lifecycle),
                "workspaceIsolation": attempt.workspace_isolation,
                "workspaceIsolationReason": attempt.workspace_isolation_reason,
                "error": format!("Subagent execution failed: {:?}", e)
            });

            SubagentRunOutcome::Failed {
                error: e,
                response_json,
            }
        }
    }
}

