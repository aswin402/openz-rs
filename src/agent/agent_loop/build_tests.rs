use super::*;
use crate::tools::graph_memory::test_lock;
use crate::tools::graph_memory::with_db;
use crate::tools::memory_extra::working::store_semantic_fact;

#[test]
fn recent_session_context_prioritizes_latest_user_assistant_turns() {
    let messages = vec![
        crate::session::Message {
            role: "user".to_string(),
            content: "old topic".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        crate::session::Message {
            role: "assistant".to_string(),
            content: "old answer".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        crate::session::Message {
            role: "user".to_string(),
            content: "we are testing model switching".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
        crate::session::Message {
            role: "assistant".to_string(),
            content: "listed weak model failures".to_string(),
            timestamp: None,
            extra: serde_json::Map::new(),
        },
    ];
    let block = recent_session_context(&messages, 500);
    assert!(block.contains("testing model switching"));
    assert!(block.contains("weak model failures"));
}

#[test]
fn identity_query_detection_catches_name_and_persona_questions() {
    assert!(is_identity_or_persona_query("what is my name"));
    assert!(is_identity_or_persona_query("who are you"));
    assert!(is_identity_or_persona_query("who are u"));
    assert!(is_identity_or_persona_query("what is your persona"));
    assert!(!is_identity_or_persona_query("fix the cargo build"));
}

#[test]
fn identity_answer_priority_is_injected_for_persona_questions_with_pinned_memory() {
    let prompt = identity_answer_priority_context(
        "who are u",
        "[Pinned Memory] Mivi persona active. Casual friend mode.",
    );

    assert!(prompt.contains("active persona"));
    assert!(prompt.contains("answer as the active persona first"));
    assert!(identity_answer_priority_context("fix cargo", "Mivi persona active").is_empty());
    assert!(identity_answer_priority_context("who are you", "").is_empty());
}

#[test]
fn test_identity_memory_candidate_matches_bot_name_and_custom_hints() {
    assert!(identity_memory_candidate("user prefers python", "openz"));
    assert!(identity_memory_candidate("bot persona is friendly", "openz"));
    assert!(identity_memory_candidate("call me Alice", "openz"));
    assert!(!identity_memory_candidate("compile the rust binary", "openz"));
}

#[test]
fn test_prompt_budget_resolution() {
    assert_eq!(resolve_prompt_budget(Some(64000), 128000), 64000);
    assert_eq!(resolve_prompt_budget(None, 128000), 48000);
    assert_eq!(resolve_prompt_budget(None, 32000), 12000);
    assert_eq!(resolve_prompt_budget(None, 16000), 8000);
}

#[test]
fn weak_model_detection_catches_small_or_free_models() {
    assert!(is_weak_or_risky_model("llama-3.1-8b-instant"));
    assert!(is_weak_or_risky_model("mimo-v2.5-free"));
    assert!(is_weak_or_risky_model("gemini-3.1-flash-lite"));
    assert!(!is_weak_or_risky_model("deepseek-v4-flash-free"));
}

#[test]
fn runtime_tool_discipline_requires_live_identity_for_model_questions() {
    let rule = runtime_tool_discipline_rules();
    assert!(rule.contains("model/provider identity"));
    assert!(rule.contains("openz_inventory"));
    assert!(rule.contains("Do not guess"));
    assert!(rule.contains("Do not delegate or research for trivial/general-knowledge tasks."));
    assert!(rule.contains("call 'request_tool_scope'"));
}

#[test]
fn generic_research_memory_guard_skips_topicless_update_queries() {
    assert!(should_skip_research_memory_for_generic_query(
        "hey whats new"
    ));
    assert!(should_skip_research_memory_for_generic_query("latest"));
    assert!(should_skip_research_memory_for_generic_query("any updates"));
    assert!(should_skip_research_memory_for_generic_query(
        "what are the tools u have"
    ));
    assert!(should_skip_research_memory_for_generic_query(
        "what features do you have"
    ));
    assert!(should_skip_research_memory_for_generic_query(
        "open the website in firefox"
    ));
    assert!(should_skip_research_memory_for_generic_query(
        "play the video"
    ));
    assert!(should_skip_research_memory_for_generic_query(
        "thats good one"
    ));
    assert!(!should_skip_research_memory_for_generic_query(
        "what features do you have vs hermes"
    ));
    assert!(!should_skip_research_memory_for_generic_query(
        "whats new in hermes"
    ));
    assert!(!should_skip_research_memory_for_generic_query(
        "latest mem0 release"
    ));
    assert!(!should_skip_research_memory_for_generic_query(
        "what is mem0"
    ));
    assert_eq!(
        concrete_research_topic_terms("whats new in hermes"),
        vec!["hermes"]
    );
}

#[test]
fn stable_brief_context_suppresses_source_notification() {
    assert!(should_notify_source_context(
        "what is turboquant-pytorch",
        false
    ));
    assert!(!should_notify_source_context(
        "what is turboquant-pytorch",
        true
    ));
    assert!(!should_notify_source_context(
        "check again https://example.com",
        false
    ));
}

#[test]
fn live_queries_suppress_saved_context_notifications() {
    assert!(!should_show_saved_context_notification(
        "check again https://example.com/path"
    ));
    assert!(!should_show_saved_context_notification(
        "research this https://example.com/path"
    ));
    assert!(should_show_saved_context_notification(
        "what is hermes agent"
    ));
}

#[test]
fn workflow_notice_is_emitted_once_per_turn() {
    let mut seen = std::collections::HashSet::new();

    assert!(should_emit_workflow_notice(
        &mut seen,
        "test_web_tools_health"
    ));
    assert!(!should_emit_workflow_notice(
        &mut seen,
        "test_web_tools_health"
    ));
    assert!(should_emit_workflow_notice(&mut seen, "another_workflow"));
}

#[test]
fn live_queries_keep_research_context_available() {
    assert!(!should_skip_research_memory_for_generic_query(
        "check this https://example.com/path"
    ));
    assert!(
        crate::agent::agent_loop::research_policy::has_live_research_intent(
            "go and check again mem0"
        )
    );
}

#[test]
fn research_brief_context_discourages_unneeded_fetches() {
    let item = crate::tools::shared_memory::ResearchBrief {
        id: "id".to_string(),
        topic: "mem0".to_string(),
        summary: "Mem0 is a memory layer for AI agents.".to_string(),
        source_ids: vec![],
        confidence: 0.8,
        stale_after_secs: 86400,
        freshness: "fresh".to_string(),
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
        use_count: 0,
        score: 9.0,
    };
    let block = format_research_brief_context_items(&[item], "what is mem0");
    assert!(block.contains("Do not call web/search tools"));
    assert!(block.contains("stable non-live question"));
    assert!(block.contains("Only state facts present"));
    assert!(block.contains("say unknown"));
}

#[test]
fn workflow_context_includes_reusable_steps() {
    let item = crate::tools::shared_memory::WorkflowCard {
        id: "id".to_string(),
        name: "screenshot_to_telegram".to_string(),
        triggers: vec!["send screenshot to telegram".to_string()],
        summary: "Capture active window and send it through Telegram".to_string(),
        steps: serde_json::json!([{ "tool": "exec_command", "note": "capture active window" }]),
        preconditions: vec!["Telegram configured".to_string()],
        verification: vec!["Telegram API ok=true".to_string()],
        risk: "normal".to_string(),
        status: "active".to_string(),
        success_count: 2,
        failure_count: 0,
        last_used: None,
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
        score: 7.5,
    };
    let block = format_workflow_context_items(&[item]);
    assert!(block.contains("steps:"));
    assert!(block.contains("exec_command"));
    assert!(block.contains("workflow_memory.record_run"));
}

#[test]
fn workflow_context_filters_weak_matches() {
    let weak = crate::tools::shared_memory::WorkflowCard {
        id: "id".to_string(),
        name: "test_web_tools_health".to_string(),
        triggers: vec!["test web tools health".to_string()],
        summary: "Verify browser search tooling".to_string(),
        steps: serde_json::json!([]),
        preconditions: vec![],
        verification: vec![],
        risk: "normal".to_string(),
        status: "active".to_string(),
        success_count: 0,
        failure_count: 0,
        last_used: None,
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
        score: 2.5,
    };
    let block = format_workflow_context_items(&[weak]);
    assert!(block.is_empty());
}

#[test]
fn test_get_version_history() {
    let history = get_version_history();
    assert!(!history.is_empty());
    // The latest release heading is always the first recorded block and is
    // guaranteed to match CARGO_PKG_VERSION by version_sync_tests.
    assert!(history.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn test_get_dynamic_tools_guideline() {
    let registry = crate::tools::ToolRegistry::new();
    struct DummyTool;
    #[async_trait::async_trait]
    impl crate::tools::Tool for DummyTool {
        fn name(&self) -> &str {
            "dummy_tool"
        }
        fn description(&self) -> &str {
            "dummy"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn call(&self, _args: &serde_json::Value) -> Result<serde_json::Value> {
            Ok(serde_json::json!({}))
        }
    }
    registry.register(std::sync::Arc::new(DummyTool));
    let guideline = get_dynamic_tools_guideline(&registry);
    assert!(guideline.contains("dummy_tool"));
    assert!(guideline.contains("Core Tools"));
}

#[tokio::test]
async fn test_cross_session_memory_excludes_stale_semantic_facts() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_prompt_stale_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let stale_fact = format!("stale-marker-{} Rust obsolete rule", scope);
    let active_fact = format!("active-marker-{} Rust current rule", scope);

    store_semantic_fact(
        &format!("{}-stale", scope),
        &stale_fact,
        0.9,
        "*",
        &scope,
        "*",
    )
    .unwrap();
    store_semantic_fact(
        &format!("{}-active", scope),
        &active_fact,
        0.9,
        "*",
        &scope,
        "*",
    )
    .unwrap();
    with_db(|conn| {
        conn.execute(
            "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE node_id = ?1",
            rusqlite::params![format!("{}-stale", scope)],
        )?;
        Ok(())
    })
    .unwrap();

    let memory = retrieve_cross_session_memories("Rust current obsolete rule", "openz").await;
    assert!(memory.contains(&active_fact));
    assert!(!memory.contains(&stale_fact));
}

#[tokio::test]
async fn test_cross_session_memory_is_top_k_budgeted() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_prompt_budget_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    for i in 0..35 {
        store_semantic_fact(
            &format!("{}-fact-{}", scope, i),
            &format!("budget-marker-{} Rust memory fact number {}", scope, i),
            0.9,
            "*",
            &scope,
            "*",
        )
        .unwrap();
    }

    let memory = retrieve_cross_session_memories("Rust memory budget marker", "openz").await;
    let fact_lines = memory
        .lines()
        .filter(|line| line.starts_with("- budget-marker-"))
        .count();
    assert!(
        fact_lines <= 30,
        "prompt memory should stay top-30 budgeted"
    );
}

#[tokio::test]
async fn test_cross_session_memory_is_query_relevant() {
    let _l = test_lock().lock().await;
    let scope = format!(
        "test_prompt_memory_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let rust_fact = format!("rust-query-marker-{} borrow checker lifetime rule", scope);
    let cooking_fact = format!(
        "cooking-query-marker-{} sourdough fermentation schedule",
        scope
    );

    store_semantic_fact(
        &format!("{}-rust", scope),
        &rust_fact,
        0.9,
        "*",
        &scope,
        "*",
    )
    .unwrap();
    store_semantic_fact(
        &format!("{}-cooking", scope),
        &cooking_fact,
        0.9,
        "*",
        &scope,
        "*",
    )
    .unwrap();

    let memory =
        retrieve_cross_session_memories("help with Rust lifetime borrow checker", "openz").await;

    assert!(memory.contains(&rust_fact));
    assert!(
        !memory.contains(&cooking_fact),
        "irrelevant memory should not be injected into every prompt"
    );

    with_db(|conn| {
        conn.execute(
            "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE session_id = ?1",
            rusqlite::params![scope],
        )?;
        Ok(())
    })
    .unwrap();
}
