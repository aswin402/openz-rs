use super::*;
use crate::agent::AgentLoop;
use crate::config::schema::Config;
use crate::providers::mock::MockProvider;
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
use std::sync::Arc;

#[test]
fn test_limit_subagent_models_to_try_keeps_primary_plus_two_fallbacks() {
    std::env::remove_var("OPENZ_MAX_FALLBACK_ATTEMPTS");
    let mut models = vec![
        "primary".to_string(),
        "fallback-1".to_string(),
        "fallback-2".to_string(),
        "fallback-3".to_string(),
    ];

    limit_subagent_models_to_try(&mut models);

    assert_eq!(models, vec!["primary", "fallback-1", "fallback-2"]);
}

#[test]
fn test_resolve_subagent_timeout_uses_default_and_clamps() {
    assert_eq!(resolve_subagent_timeout_secs(None, 300), 300);
    assert_eq!(
        resolve_subagent_timeout_secs(Some(1), 300),
        crate::tools::MIN_TOOL_TIMEOUT_SECS
    );
    assert_eq!(
        resolve_subagent_timeout_secs(Some(999_999), 300),
        crate::tools::MAX_TOOL_TIMEOUT_SECS
    );
    assert_eq!(resolve_subagent_timeout_secs(Some(120), 300), 120);
}

#[test]
fn test_subagent_provider_prefixed_model_overrides_default_provider() {
    let mut config = Config::default();
    config.agents.defaults.provider = "opencode_zen".to_string();
    config.agents.defaults.model = "deepseek-v4-flash-free".to_string();
    config.providers.groq = Some(crate::config::schema::ProviderConfig {
        api_key: Some("groq-key".to_string()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: Default::default(),
    });

    let resolved =
        resolve_provider_for_subagent_model(&config, "groq/llama-3.2-11b-vision-preview")
            .expect("provider-prefixed subagent model should resolve");

    assert_eq!(resolved.provider_name, "groq");
    assert_eq!(resolved.model, "llama-3.2-11b-vision-preview");
    assert_eq!(resolved.api_base, "https://api.groq.com/openai/v1");
}

#[test]
fn filesystem_write_denied_policy_helper_detects_denial() {
    use crate::orchestrator::spec::CapabilityPolicy;
    assert!(!filesystem_write_denied_by_policy(&None));
    assert!(!filesystem_write_denied_by_policy(&Some(
        CapabilityPolicy::default()
    )));
    assert!(filesystem_write_denied_by_policy(&Some(
        CapabilityPolicy {
            deny_filesystem_write: true,
            ..Default::default()
        }
    )));
}

#[test]
fn test_ensure_markdown_images_wraps_paths_and_urls() {
    let input = "Check this file: /tmp/screenshot.png and url https://example.com/image.jpg";
    let formatted = ensure_markdown_images(input);
    assert!(formatted.contains("![](file:///tmp/screenshot.png)") || formatted.contains("![](/tmp/screenshot.png)"));
    assert!(formatted.contains("![](https://example.com/image.jpg)"));

    // Already formatted markdown image should not be double-wrapped
    let already = "Here is ![label](https://example.com/pic.png)";
    let formatted_already = ensure_markdown_images(already);
    assert_eq!(already, formatted_already);
}

#[test]
fn test_build_subagent_prompt_includes_sections_and_schema() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "result": {"type": "string"}
        },
        "required": ["result"]
    });

    let prompt = build_subagent_prompt(
        "You are a test agent.",
        "Solve equation 2+2",
        "Use arithmetic rules",
        Some(&schema),
    );

    assert!(prompt.contains("You are a test agent."));
    assert!(prompt.contains("TASK:\nSolve equation 2+2"));
    assert!(prompt.contains("CONTEXT:\nUse arithmetic rules"));
    assert!(prompt.contains("CRITICAL REQUIREMENT: Your final response MUST be a raw JSON object strictly conforming to this JSON Schema:"));
    assert!(prompt.contains("\"result\""));
}

#[tokio::test]
async fn test_run_subagent_attempt_returns_cancelled_when_token_pre_cancelled() {
    let token = CancellationToken::new();
    token.cancel();

    let parent_provider: Arc<dyn LLMProvider> = Arc::new(MockProvider::new());
    let temp_dir = std::env::temp_dir().join(format!("test_subagent_pre_cancel_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let session_mgr = SessionManager::new(temp_dir.clone());
    let child_agent = AgentLoop::new(
        Config::default(),
        parent_provider.clone(),
        ToolRegistry::new(),
        session_mgr,
    );

    let attempt = SubagentRunAttempt {
        tool_name: "delegate_task",
        profile_name: None,
        subagent_name: "delegate_task",
        model_name: "test-model",
        child_session_id: "subagent:test1",
        prompt: "do something",
        clean_goal: "goal",
        clean_context: "context",
        current_depth: 0,
        timeout_secs: Some(10),
        default_timeout_secs: 30,
        spinner_msg: "running...",
        json_schema: None,
        parent_dir: &temp_dir,
        workspace_dir: temp_dir.clone(),
        filesystem_write_denied: true,
        workspace_isolation: "not_required",
        workspace_isolation_reason: &None,
        announce_branch: false,
    };

    let outcome = run_subagent_attempt(&child_agent, &parent_provider, &token, attempt).await;
    match outcome {
        SubagentRunOutcome::Cancelled(val) => {
            assert_eq!(val["status"], "cancelled");
            assert_eq!(val["lifecycle"]["code"], "cancelled");
            assert_eq!(val["tool"], "delegate_task");
        }
        other => panic!("Expected Cancelled outcome, got: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}
