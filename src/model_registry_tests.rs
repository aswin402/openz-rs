use super::*;

#[test]
fn model_registry_key_is_stable() {
    assert_eq!(
        ModelRegistry::key("groq", "llama-3.1-8b-instant"),
        "groq::llama-3.1-8b-instant"
    );
}

#[test]
fn health_record_truncates_long_errors() {
    let mut record = ModelHealthRecord::new("p", "m", "risky", true, vec![]);
    record.mark_failure(&"x".repeat(800));
    assert_eq!(record.last_error.unwrap().len(), 500);
}

#[test]
fn health_record_tracks_success_and_failure() {
    let mut record = ModelHealthRecord::new(
        "opencode_zen",
        "big-pickle",
        "risky",
        true,
        vec!["unknown".to_string()],
    );
    record.mark_success(true, true, false);
    record.mark_failure("provider failed");
    assert_eq!(record.success_count, 1);
    assert_eq!(record.failure_count, 1);
    assert_eq!(record.blank_response_count, 1);
    assert_eq!(record.think_leak_count, 1);
    assert_eq!(record.last_error.as_deref(), Some("provider failed"));
}
