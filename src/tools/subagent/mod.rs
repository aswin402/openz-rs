use std::sync::Arc;
use crate::providers::LLMProvider;
use crate::config::schema::Config;

tokio::task_local! {
    pub static DELEGATION_DEPTH: usize;
}

pub mod cancellation_token;
pub mod delegate_task;
pub mod delegate_profile;
pub mod evaluator_optimizer;
pub mod optimize_profile;
pub mod parallel_research;

#[cfg(test)]
mod tests;

pub use cancellation_token::CancellationToken;
pub use delegate_task::{DelegateTaskTool, cleanup_stale_resources};
pub use delegate_profile::DelegateProfileTool;
pub use evaluator_optimizer::EvaluatorOptimizerLoopTool;
pub use optimize_profile::{OptimizeSubagentTool, CreateSubagentTool, DeleteSubagentTool};
pub use parallel_research::ParallelResearchTool;

// Shared utility function used across tools:
pub fn build_provider_for_model(config: &Config, model: &str) -> anyhow::Result<Arc<dyn LLMProvider>> {
    let resolved = crate::providers::resolver::resolve_provider_full(config, model)?;
    Ok(resolved.instance)
}
