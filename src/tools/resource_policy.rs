use crate::config::schema::AgentDefaults;
use crate::tools::ToolMetadata;
use std::sync::atomic::{AtomicUsize, Ordering};

static ACTIVE_PROCESS_TOOLS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeResourceSnapshot {
    pub free_disk_gb: Option<f64>,
    pub active_process_tools: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResourceDecision {
    Allow,
    RequireApproval { reason: String },
    Block { reason: String },
}

impl ToolResourceDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::RequireApproval { .. } => "require_approval",
            Self::Block { .. } => "block",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Allow => None,
            Self::RequireApproval { reason } | Self::Block { reason } => Some(reason),
        }
    }
}

pub struct ToolResourcePolicy;

#[derive(Debug)]
pub struct ProcessToolGuard;

impl Drop for ProcessToolGuard {
    fn drop(&mut self) {
        ACTIVE_PROCESS_TOOLS.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn active_process_tools() -> usize {
    ACTIVE_PROCESS_TOOLS.load(Ordering::SeqCst)
}

pub fn try_acquire_process_tool(max_concurrent: usize) -> Result<ProcessToolGuard, String> {
    loop {
        let current = ACTIVE_PROCESS_TOOLS.load(Ordering::SeqCst);
        match current >= max_concurrent {
            true => {
                return Err(format!(
                    "process tool limit reached ({}/{})",
                    current, max_concurrent
                ));
            }
            false => {}
        }
        match ACTIVE_PROCESS_TOOLS
            .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            true => return Ok(ProcessToolGuard),
            false => {}
        }
    }
}

impl RuntimeResourceSnapshot {
    pub fn current() -> Self {
        Self {
            free_disk_gb: current_openz_free_disk_gb(),
            active_process_tools: active_process_tools(),
        }
    }
}

fn current_openz_free_disk_gb() -> Option<f64> {
    let path = crate::config::loader::runtime_data_dir();
    let _ = std::fs::create_dir_all(&path);
    crate::core::process::available_bytes(&path)
        .map(|bytes| bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}

pub fn evaluate_artifact_write(
    action: &str,
    min_free_disk_gb: f64,
    runtime: &RuntimeResourceSnapshot,
) -> ToolResourceDecision {
    match runtime.free_disk_gb.filter(|free| *free < min_free_disk_gb) {
        Some(free_disk_gb) => ToolResourceDecision::Block {
            reason: format!(
                "{} blocked: free disk {:.1}GB is below required minimum {:.1}GB",
                action, free_disk_gb, min_free_disk_gb
            ),
        },
        None => ToolResourceDecision::Allow,
    }
}

pub fn ensure_artifact_write_allowed(action: &str) -> anyhow::Result<()> {
    let defaults = AgentDefaults::default();
    let runtime = RuntimeResourceSnapshot::current();
    match evaluate_artifact_write(action, defaults.min_free_disk_gb, &runtime) {
        ToolResourceDecision::Allow | ToolResourceDecision::RequireApproval { .. } => Ok(()),
        ToolResourceDecision::Block { reason } => Err(anyhow::anyhow!(
            "Resource policy blocked artifact generation: {}",
            reason
        )),
    }
}

pub fn is_low_disk_recovery_tool(tool_name: &str, arguments: &serde_json::Value) -> bool {
    match tool_name {
        "manage_sessions" => matches!(
            arguments.get("action").and_then(|v| v.as_str()),
            Some("list" | "prune")
        ),
        "cache_stats" | "clear_cache" => true,
        _ => false,
    }
}

pub fn is_free_disk_block(reason: &str) -> bool {
    reason.contains("free disk") && reason.contains("below required minimum")
}

impl ToolResourcePolicy {
    pub fn evaluate(
        metadata: &ToolMetadata,
        defaults: &AgentDefaults,
        runtime: &RuntimeResourceSnapshot,
    ) -> ToolResourceDecision {
        let network_blocked = metadata.uses_network && !defaults.allow_network_tools;
        let low_disk = metadata
            .writes_disk
            .then_some(runtime.free_disk_gb)
            .flatten()
            .filter(|free_disk_gb| *free_disk_gb < defaults.min_free_disk_gb);
        let process_limit_reached = metadata.spawns_process
            && runtime.active_process_tools >= defaults.max_concurrent_process_tools;
        let expensive_approval = defaults.warn_before_expensive_tools
            && metadata.uses_network
            && (metadata.writes_disk || metadata.spawns_process);

        match (
            network_blocked,
            low_disk,
            process_limit_reached,
            metadata.requires_conservative_approval(),
            metadata.requires_risk_approval(),
            expensive_approval,
        ) {
            (true, _, _, _, _, _) => ToolResourceDecision::Block {
                reason: "Network tools are disabled by resource policy".to_string(),
            },
            (false, Some(free_disk_gb), _, _, _, _) => ToolResourceDecision::Block {
                reason: format!(
                    "free disk {:.1}GB is below required minimum {:.1}GB",
                    free_disk_gb, defaults.min_free_disk_gb
                ),
            },
            (false, None, true, _, _, _) => ToolResourceDecision::Block {
                reason: format!(
                    "process tool limit reached ({}/{})",
                    runtime.active_process_tools, defaults.max_concurrent_process_tools
                ),
            },
            (false, None, false, true, _, _) => ToolResourceDecision::RequireApproval {
                reason: "unclassified tool requires approval".to_string(),
            },
            (false, None, false, false, true, _) => ToolResourceDecision::RequireApproval {
                reason: "high risk tool requires approval".to_string(),
            },
            (false, None, false, false, false, true) => ToolResourceDecision::RequireApproval {
                reason: "expensive network/process/disk tool requires approval".to_string(),
            },
            (false, None, false, false, false, false) => ToolResourceDecision::Allow,
        }
    }
}

#[cfg(test)]
#[path = "resource_policy_tests.rs"]
mod tests;
