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

#[test]
fn fallback_parse_curator_handles_escaped_quotes_and_code_braces() {
    let raw = r##"{
        "user_preferences": "- Always quote \"strict_mode\" in configs",
        "skills_to_save": [
            {
                "operation": "patch",
                "name": "rust_brace_skill",
                "new_text": "# Rust Example\n```rust\nfn demo() { let map = \"{\\\"k\\\": 1}\"; }\n```"
            }
        ],
        "truncated_tail": "unfinished..."##;

    let salvaged = fallback_parse_curator(raw).expect("escape-aware salvage should succeed");
    assert_eq!(
        salvaged.user_preferences,
        "- Always quote \"strict_mode\" in configs"
    );
    assert_eq!(salvaged.skills_to_save.len(), 1);
    assert_eq!(salvaged.skills_to_save[0].name, "rust_brace_skill");
    assert_eq!(salvaged.skills_to_save[0].operation.as_deref(), Some("patch"));
}

#[test]
fn trajectory_recovered_after_error_detects_error_then_clean_fix() {
    let make_msg = |role: &str, content: &str| crate::session::Message {
        role: role.to_string(),
        content: content.to_string(),
        timestamp: None,
        extra: serde_json::Map::new(),
    };
    let recovered = vec![
        make_msg("user", "compile project"),
        make_msg("tool", r#"{"error":"compile failed"}"#),
        make_msg("tool", r#"{"status":"ok","status_code":0}"#),
    ];
    assert!(trajectory_recovered_after_error(&recovered));

    let unrecovered = vec![
        make_msg("user", "compile project"),
        make_msg("tool", r#"{"status":"ok"}"#),
        make_msg("tool", r#"{"error":"still broken"}"#),
    ];
    assert!(!trajectory_recovered_after_error(&unrecovered));
}
