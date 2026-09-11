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
        if current >= max_concurrent {
            return Err(format!(
                "process tool limit reached ({}/{})",
                current, max_concurrent
            ));
        }
        if ACTIVE_PROCESS_TOOLS
            .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return Ok(ProcessToolGuard);
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

#[cfg(unix)]
fn current_openz_free_disk_gb() -> Option<f64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let path = crate::config::loader::runtime_data_dir();
    let _ = std::fs::create_dir_all(&path);
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    let bytes = stat.f_bavail as f64 * stat.f_frsize as f64;
    Some(bytes / 1024.0 / 1024.0 / 1024.0)
}

#[cfg(not(unix))]
fn current_openz_free_disk_gb() -> Option<f64> {
    None
}

pub fn evaluate_artifact_write(
    action: &str,
    min_free_disk_gb: f64,
    runtime: &RuntimeResourceSnapshot,
) -> ToolResourceDecision {
    if let Some(free_disk_gb) = runtime.free_disk_gb {
        if free_disk_gb < min_free_disk_gb {
            return ToolResourceDecision::Block {
                reason: format!(
                    "{} blocked: free disk {:.1}GB is below required minimum {:.1}GB",
                    action, free_disk_gb, min_free_disk_gb
                ),
            };
        }
    }

    ToolResourceDecision::Allow
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
        if metadata.uses_network && !defaults.allow_network_tools {
            return ToolResourceDecision::Block {
                reason: "Network tools are disabled by resource policy".to_string(),
            };
        }

        if metadata.writes_disk {
            if let Some(free_disk_gb) = runtime.free_disk_gb {
                if free_disk_gb < defaults.min_free_disk_gb {
                    return ToolResourceDecision::Block {
                        reason: format!(
                            "free disk {:.1}GB is below required minimum {:.1}GB",
                            free_disk_gb, defaults.min_free_disk_gb
                        ),
                    };
                }
            }
        }

        if metadata.spawns_process
            && runtime.active_process_tools >= defaults.max_concurrent_process_tools
        {
            return ToolResourceDecision::Block {
                reason: format!(
                    "process tool limit reached ({}/{})",
                    runtime.active_process_tools, defaults.max_concurrent_process_tools
                ),
            };
        }

        if metadata.requires_conservative_approval() {
            return ToolResourceDecision::RequireApproval {
                reason: "unclassified tool requires approval".to_string(),
            };
        }

        if metadata.requires_risk_approval() {
            return ToolResourceDecision::RequireApproval {
                reason: "high risk tool requires approval".to_string(),
            };
        }

        if defaults.warn_before_expensive_tools
            && (metadata.uses_network && (metadata.writes_disk || metadata.spawns_process))
        {
            return ToolResourceDecision::RequireApproval {
                reason: "expensive network/process/disk tool requires approval".to_string(),
            };
        }

        ToolResourceDecision::Allow
    }
}

#[cfg(test)]
#[path = "resource_policy_tests.rs"]
mod tests;
