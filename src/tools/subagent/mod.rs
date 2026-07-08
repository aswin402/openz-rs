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

#[cfg(test)]
mod tests;

pub use cancellation_token::CancellationToken;
pub use delegate_profile::DelegateProfileTool;
pub use delegate_task::{cleanup_stale_resources, DelegateTaskTool};
pub use evaluator_optimizer::EvaluatorOptimizerLoopTool;
pub use lifecycle::{classify_subagent_error, status_json, SubagentRunStatus};
pub use optimize_profile::{CreateSubagentTool, DeleteSubagentTool, OptimizeSubagentTool};
pub use parallel_research::ParallelResearchTool;

// Shared utility function used across tools:
pub fn build_provider_for_model(
    config: &Config,
    model: &str,
) -> anyhow::Result<Arc<dyn LLMProvider>> {
    let resolved = crate::providers::resolver::resolve_provider_full(config, model)?;
    Ok(resolved.instance)
}
