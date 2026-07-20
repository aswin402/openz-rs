use crate::config::schema::Config;
use crate::providers::LLMProvider;
use std::sync::Arc;

tokio::task_local! {
    pub static DELEGATION_DEPTH: usize;
    pub static ACTIVE_SUBAGENT: String;
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
pub use optimize_profile::{CreateSubagentTool, DeleteSubagentTool, OptimizeSubagentTool};
pub use parallel_research::ParallelResearchTool;

pub fn subagent_tool_metadata(name: &str) -> crate::tools::ToolMetadata {
    let mut metadata = crate::tools::ToolMetadata::infer(name);
    metadata.domain = "subagent";
    metadata.risk = crate::tools::ToolRisk::Medium;
    metadata.spawns_process = true;
    metadata.requires_approval = false;
    metadata.priority = 100;
    metadata.recommended_timeout_secs = Some(600);
    metadata
}

pub fn resolve_subagent_timeout_secs(
    requested_timeout_secs: Option<u64>,
    default_timeout_secs: u64,
) -> u64 {
    crate::tools::clamp_tool_timeout_secs(requested_timeout_secs.unwrap_or(default_timeout_secs))
}

// Shared utility function used across tools:
pub fn build_provider_for_model(
    config: &Config,
    model: &str,
) -> anyhow::Result<Arc<dyn LLMProvider>> {
    let resolved = crate::providers::resolver::resolve_provider_full(config, model)?;
    Ok(resolved.instance)
}

pub fn scan_for_images(goal: &str, context: &str) -> Vec<String> {
    let mut image_paths = Vec::new();
    if let Ok(path_regex) = regex::Regex::new(r"(?:file://)?(/[a-zA-Z0-9_\-\./]+|~/[a-zA-Z0-9_\-\./]+)") {
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
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
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
            if text_lower.contains("image") || text_lower.contains("picture") || text_lower.contains("screenshot") {
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

    let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir, async move {
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
    let run_res_timeout = tokio::time::timeout(
        std::time::Duration::from_secs(sub_timeout),
        run_res_fut,
    );
    match crate::agent::style::with_spinner(spinner_msg, run_res_timeout).await {
        Ok(res) => res,
        Err(_) => Err(anyhow::anyhow!("Subagent execution timed out after {sub_timeout}s")),
    }
}
