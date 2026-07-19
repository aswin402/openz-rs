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
