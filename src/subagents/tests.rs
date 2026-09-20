use super::*;

#[test]
fn subagent_health_tracks_fallback_success_and_failure() {
    let mut health = SubagentHealthRecord::default();
    health.mark_failure("primary unavailable");
    health.mark_success("fallback/model");
    assert_eq!(
        health.last_successful_model.as_deref(),
        Some("fallback/model")
    );
    assert_eq!(health.failure_count, 1);
    assert!(health.last_error.is_none());
}

#[test]
fn subagent_health_registry_is_keyed_by_profile_name() {
    let mut registry = SubagentHealthRegistry::default();
    registry.record_failure("vision_agent", "no vision provider");
    registry.record_success("vision_agent", "google_ai_studio/gemini-2.5-flash");
    let record = registry.get("vision_agent").expect("health record");
    assert_eq!(record.failure_count, 1);
    assert_eq!(
        record.last_successful_model.as_deref(),
        Some("google_ai_studio/gemini-2.5-flash")
    );
}

#[test]
fn default_subagent_policy_includes_orchestrator() {
    assert!(is_default_subagent("orchestrator"));
    assert!(is_default_subagent("planner"));
    assert!(is_default_subagent("openz"));
    assert!(!is_default_subagent("custom_planner"));
    assert_eq!(MAX_SUBAGENT_FALLBACKS, 3);
}
