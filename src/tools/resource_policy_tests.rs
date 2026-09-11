use super::*;
use crate::tools::ToolRisk;
use std::sync::Mutex;

static PROCESS_GUARD_TEST_LOCK: Mutex<()> = Mutex::new(());

fn metadata(
    risk: ToolRisk,
    uses_network: bool,
    writes_disk: bool,
    spawns_process: bool,
) -> ToolMetadata {
    ToolMetadata {
        presentation_name: "Test Tool".to_string(),
        domain: "test",
        risk,
        uses_network,
        writes_disk,
        spawns_process,
        requires_approval: matches!(risk, ToolRisk::High),
        priority: 1,
        aliases: &[],
        examples: &[],
        when_to_use: "",
        when_not_to_use: "",
        recommended_timeout_secs: None,
    }
}

fn defaults() -> AgentDefaults {
    AgentDefaults::default()
}

#[test]
fn low_disk_recovery_allows_only_safe_cleanup_actions() {
    assert!(is_low_disk_recovery_tool(
        "manage_sessions",
        &serde_json::json!({ "action": "prune" })
    ));
    assert!(is_low_disk_recovery_tool(
        "manage_sessions",
        &serde_json::json!({ "action": "list" })
    ));
    assert!(is_low_disk_recovery_tool(
        "clear_cache",
        &serde_json::json!({})
    ));
    assert!(!is_low_disk_recovery_tool(
        "manage_sessions",
        &serde_json::json!({ "action": "delete" })
    ));
    assert!(!is_low_disk_recovery_tool(
        "replace_lines",
        &serde_json::json!({})
    ));
}

#[test]
fn network_disabled_blocks_network_tools() {
    let mut defaults = defaults();
    defaults.allow_network_tools = false;
    let runtime = RuntimeResourceSnapshot {
        free_disk_gb: Some(100.0),
        active_process_tools: 0,
    };
    let decision = ToolResourcePolicy::evaluate(
        &metadata(ToolRisk::Low, true, false, false),
        &defaults,
        &runtime,
    );
    assert!(
        matches!(decision, ToolResourceDecision::Block { ref reason } if reason.contains("Network tools are disabled"))
    );
}

#[test]
fn low_disk_blocks_disk_writing_tools() {
    let mut defaults = defaults();
    defaults.min_free_disk_gb = 2.0;
    let runtime = RuntimeResourceSnapshot {
        free_disk_gb: Some(1.0),
        active_process_tools: 0,
    };
    let decision = ToolResourcePolicy::evaluate(
        &metadata(ToolRisk::Low, false, true, false),
        &defaults,
        &runtime,
    );
    assert!(
        matches!(decision, ToolResourceDecision::Block { ref reason } if reason.contains("free disk"))
    );
}

#[test]
fn low_disk_allows_read_only_tools() {
    let mut defaults = defaults();
    defaults.min_free_disk_gb = 2.0;
    let runtime = RuntimeResourceSnapshot {
        free_disk_gb: Some(1.0),
        active_process_tools: 0,
    };
    let decision = ToolResourcePolicy::evaluate(
        &metadata(ToolRisk::Low, false, false, false),
        &defaults,
        &runtime,
    );
    assert_eq!(decision, ToolResourceDecision::Allow);
}

#[test]
fn high_risk_tools_require_approval() {
    let defaults = defaults();
    let runtime = RuntimeResourceSnapshot {
        free_disk_gb: Some(100.0),
        active_process_tools: 0,
    };
    let decision = ToolResourcePolicy::evaluate(
        &metadata(ToolRisk::High, false, false, false),
        &defaults,
        &runtime,
    );
    assert!(
        matches!(decision, ToolResourceDecision::RequireApproval { ref reason } if reason.contains("high risk"))
    );
}

#[test]
fn artifact_write_blocks_when_free_disk_is_below_floor() {
    let runtime = RuntimeResourceSnapshot {
        free_disk_gb: Some(0.5),
        active_process_tools: 0,
    };
    let decision = evaluate_artifact_write("generate_video", 2.0, &runtime);
    assert!(
        matches!(decision, ToolResourceDecision::Block { ref reason } if reason.contains("generate_video") && reason.contains("free disk"))
    );
}

#[test]
fn artifact_write_allows_when_disk_is_unknown_or_sufficient() {
    let unknown = RuntimeResourceSnapshot {
        free_disk_gb: None,
        active_process_tools: 0,
    };
    assert_eq!(
        evaluate_artifact_write("generate_image", 2.0, &unknown),
        ToolResourceDecision::Allow
    );

    let enough = RuntimeResourceSnapshot {
        free_disk_gb: Some(5.0),
        active_process_tools: 0,
    };
    assert_eq!(
        evaluate_artifact_write("generate_image", 2.0, &enough),
        ToolResourceDecision::Allow
    );
}

#[test]
fn process_guard_tracks_active_count_and_releases_on_drop() {
    let _lock = PROCESS_GUARD_TEST_LOCK.lock().unwrap();
    let start = active_process_tools();
    let guard = try_acquire_process_tool(start + 1).expect("guard acquired");
    assert_eq!(active_process_tools(), start + 1);
    drop(guard);
    assert_eq!(active_process_tools(), start);
}

#[test]
fn process_guard_rejects_when_limit_reached() {
    let _lock = PROCESS_GUARD_TEST_LOCK.lock().unwrap();
    let start = active_process_tools();
    let guard = try_acquire_process_tool(start + 1).expect("guard acquired");
    let err = try_acquire_process_tool(start + 1).expect_err("limit reached");
    assert!(err.contains("process tool limit"));
    drop(guard);
    assert_eq!(active_process_tools(), start);
}

#[test]
fn process_limit_blocks_extra_process_tools() {
    let mut defaults = defaults();
    defaults.max_concurrent_process_tools = 1;
    let runtime = RuntimeResourceSnapshot {
        free_disk_gb: Some(100.0),
        active_process_tools: 1,
    };
    let decision = ToolResourcePolicy::evaluate(
        &metadata(ToolRisk::Low, false, false, true),
        &defaults,
        &runtime,
    );
    assert!(
        matches!(decision, ToolResourceDecision::Block { ref reason } if reason.contains("process tool limit"))
    );
}
