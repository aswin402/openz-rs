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

#[test]
fn stemmed_learning_intent_avoids_substring_false_positives() {
    let make_user = |text: &str| {
        vec![crate::session::Message {
            role: "user".to_string(),
            content: text.to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        }]
    };
    assert!(!has_explicit_learning_intent(&make_user(
        "install sklearn and run k-means clustering"
    )));
    assert!(!has_explicit_learning_intent(&make_user(
        "rename unskilled_worker field in schema"
    )));
    assert!(has_explicit_learning_intent(&make_user(
        "remember that I prefer Swiss Minimalist layouts"
    )));
    assert!(has_explicit_learning_intent(&make_user(
        "save this deployment workflow for future runs"
    )));
}

#[test]
fn coalescing_curator_queue_retains_latest_concurrent_snapshot() {
    let session_key = format!("coalesce-test-{}", uuid::Uuid::new_v4());
    let temp_sessions = crate::session::SessionManager::new(
        std::env::temp_dir().join(format!("coalesce-sm-{}", uuid::Uuid::new_v4())),
    );
    let provider: std::sync::Arc<dyn crate::providers::LLMProvider> =
        std::sync::Arc::new(crate::providers::mock::MockProvider::new());

    let make_job = |query: &str| CuratorJobPayload {
        session_manager: temp_sessions.clone(),
        session_key: session_key.clone(),
        user_query: query.to_string(),
        provider: provider.clone(),
        messages: Vec::new(),
        tools_used: Vec::new(),
        activated_skills: Vec::new(),
        mutated_skill_in_turn: false,
        had_turn_errors: false,
        turn_succeeded_cleanly: true,
        turn_recovered: false,
    };

    let claimed = claim_or_coalesce_curator_job(make_job("turn 1"));
    assert!(claimed.is_some());

    // Concurrent turns 2 and 3 coalesce behind turn 1 without dropping the latest state
    assert!(claim_or_coalesce_curator_job(make_job("turn 2")).is_none());
    assert!(claim_or_coalesce_curator_job(make_job("turn 3")).is_none());

    let popped = finish_or_pop_coalesced_curator_job(&session_key)
        .expect("coalesced turn 3 should pop next");
    assert_eq!(popped.user_query, "turn 3");
    assert!(finish_or_pop_coalesced_curator_job(&session_key).is_none());
}

#[test]
fn in_loop_skill_nudge_fires_on_active_skill_recovery_and_milestone() {
    let config = crate::config::schema::Config::default();
    let provider: std::sync::Arc<dyn crate::providers::LLMProvider> =
        std::sync::Arc::new(crate::providers::mock::MockProvider::new());
    let ctx = crate::agent::agent_loop::TurnContext {
        session_key: "cli:nudge_test",
        user_content: "deploy app",
        active_provider: provider,
        session: crate::session::Session::new("cli:nudge_test"),
        messages: Vec::new(),
        system_prompt: String::new(),
        final_content: String::new(),
        tools_used: Vec::new(),
        interaction_id: None,
        turn_errors: Vec::new(),
        session_file_lock: None,
        streamed: false,
        config,
        workflow_notice_names: std::collections::HashSet::new(),
        skill_notice_names: std::collections::HashSet::new(),
        mutated_skill_in_turn: false,
    };
    let mut run = super::super::run::turn::RunContext::new(&ctx);
    let mut active_skills = std::collections::HashSet::new();
    active_skills.insert("docker_deploy".to_string());

    // 1. Tool fails while active skill is loaded -> marks error, no nudge yet
    let n1 = super::super::run::turn::evaluate_in_loop_skill_nudge(
        &mut run,
        &mut active_skills,
        "exec_command",
        &serde_json::json!({"command": "docker compose --old-flag"}),
        false,
    );
    assert!(n1.is_none());

    // 2. Subsequent tool succeeds -> emits one-time active_skill_self_heal nudge!
    let n2 = super::super::run::turn::evaluate_in_loop_skill_nudge(
        &mut run,
        &mut active_skills,
        "exec_command",
        &serde_json::json!({"command": "docker compose up -d"}),
        true,
    )
    .expect("self-heal nudge should fire");
    assert_eq!(n2["kind"].as_str(), Some("active_skill_self_heal"));

    // 3. Reaching milestone iteration 12 without curating -> emits workflow_crystallization_nudge!
    run.iterations = 11;
    let n3 = super::super::run::turn::evaluate_in_loop_skill_nudge(
        &mut run,
        &mut active_skills,
        "read_file",
        &serde_json::json!({"path": "Cargo.toml"}),
        true,
    )
    .expect("milestone nudge should fire at iteration 12");
    assert_eq!(n3["kind"].as_str(), Some("workflow_crystallization_nudge"));
}
