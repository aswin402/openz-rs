use super::*;

#[test]
fn self_improvement_prompt_documents_workflow_outputs() {
    let prompt = self_improvement_review_prompt();
    assert!(prompt.contains("workflows_to_save"));
    assert!(prompt.contains("sources_to_save"));
    assert!(prompt.contains("Large static website"));
    assert!(prompt.contains("render in shorter segments"));
    assert!(prompt.contains("manage_servers"));
    assert!(prompt.contains("openz_inventory"));
}

#[test]
fn curator_spawn_debounces_fast_repeated_session() {
    let key = format!(
        "test-curator-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    assert!(should_spawn_curator(&key, Duration::from_secs(20)));
    assert!(!should_spawn_curator(&key, Duration::from_secs(20)));
}
