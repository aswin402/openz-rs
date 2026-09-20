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

#[test]
fn fallback_parse_curator_salvages_user_preferences_and_skills() {
    let raw = r##"{
        "memory_updated": true,
        "memory_content": "- User is Aswin\n- Project is OpenZ",
        "user_preferences": "- Loves Cyberpunk Neon with #00ffcc accents\n- Keep code minimal",
        "skills_to_save": [
            {
                "name": "cyberpunk_ui",
                "description": "Design system for cyberpunk landing pages",
                "triggers": ["cyberpunk", "neon landing page"],
                "vault_category": "websites",
                "content": "# Cyberpunk UI\nUse neon gradients"
            }
        ]
    }"##;

    let salvaged = fallback_parse_curator(raw).expect("fallback parsing should succeed");
    assert_eq!(salvaged.user_preferences, "- Loves Cyberpunk Neon with #00ffcc accents\n- Keep code minimal");
    assert_eq!(salvaged.memory_content, "- User is Aswin\n- Project is OpenZ");
    assert_eq!(salvaged.skills_to_save.len(), 1);
    assert_eq!(salvaged.skills_to_save[0].name, "cyberpunk_ui");
    assert_eq!(salvaged.skills_to_save[0].vault_category.as_deref(), Some("websites"));
}
