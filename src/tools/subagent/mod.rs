#[cfg(test)]
static TEST_CANCEL_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) async fn cancel_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_CANCEL_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
}

tokio::task_local! {
    pub static DELEGATION_DEPTH: usize;
    pub static ACTIVE_SUBAGENT: String;
    pub static ORCHESTRATED_NESTED_DELEGATION_ALLOWED: bool;
    pub static ACTIVE_PARENT_STICKY_TOOLS: Vec<String>;
}

pub fn active_parent_sticky_tools() -> Vec<String> {
    ACTIVE_PARENT_STICKY_TOOLS
        .try_with(|sticky| sticky.clone())
        .unwrap_or_default()
}

pub mod allowlist;
pub mod cancellation_token;
pub mod delegate_profile;
pub mod delegate_task;
pub mod evaluator_optimizer;
pub mod lifecycle;
pub mod optimize_profile;
pub mod parallel_research;
pub mod schema_retry;
pub mod workspace;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

pub use allowlist::*;
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
        "planner" | "sop_designer" | "openz_coordinator" | "openz"
    )
}

pub fn nested_delegation_allowed_for_active_context(profile_name: &str) -> bool {
    ORCHESTRATED_NESTED_DELEGATION_ALLOWED
        .try_with(|allowed| *allowed)
        .unwrap_or_else(|_| can_spawn_nested_subagents(profile_name))
}

pub(crate) fn filesystem_write_denied_by_policy(
    policy: &Option<crate::orchestrator::spec::CapabilityPolicy>,
) -> bool {
    policy
        .as_ref()
        .map(|policy| policy.deny_filesystem_write)
        .unwrap_or(false)
}

pub(crate) fn extract_string_arg(arguments: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        let val = arguments.get(*key)?;
        let raw = val
            .as_str()
            .map(str::to_string)
            .or_else(|| (val.is_number() || val.is_boolean()).then(|| val.to_string()))?;
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

pub(crate) fn extract_u64_arg(arguments: &serde_json::Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        let val = arguments.get(*key)?;
        val.as_u64()
            .or_else(|| val.as_i64().filter(|&n| n >= 0).map(|n| n as u64))
            .or_else(|| val.as_f64().filter(|&f| f >= 0.0).map(|f| f as u64))
            .or_else(|| val.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
    })
}

pub mod runner;
pub use runner::*;
