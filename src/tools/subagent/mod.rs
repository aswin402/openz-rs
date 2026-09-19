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

pub(crate) fn extract_string_arg(arguments: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(val) = arguments.get(*key) {
            if let Some(s) = val.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            } else if val.is_number() || val.is_boolean() {
                let s = val.to_string();
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn extract_u64_arg(arguments: &serde_json::Value, keys: &[&str]) -> Option<u64> {
    for key in keys {
        if let Some(val) = arguments.get(*key) {
            if let Some(n) = val.as_u64() {
                return Some(n);
            }
            if let Some(n) = val.as_i64() {
                if n >= 0 {
                    return Some(n as u64);
                }
            }
            if let Some(f) = val.as_f64() {
                if f >= 0.0 {
                    return Some(f as u64);
                }
            }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.trim().parse::<u64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

pub mod runner;
pub use runner::*;
