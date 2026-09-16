use super::*;
use crate::config::schema::Config;
use crate::session::SessionManager;
use crate::tools::Tool;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

static TEST_CANCEL_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();


async fn cancel_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_CANCEL_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
}

struct MockTool {
    name: String,
}

#[async_trait::async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        "mock"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({})
    }
    async fn call(&self, _args: &Value) -> Result<Value> {
        Ok(serde_json::json!({}))
    }
}

#[tokio::test]
async fn subagent_allowlisted_tools_exist_in_registry() {
    use crate::cli::tools::register_all_tools;
    use crate::providers::mock::MockProvider;
    use crate::tools::ToolRegistry;

    let registry = ToolRegistry::new();
    let config = Config::default();
    let provider = Arc::new(MockProvider::new());
    let sessions = SessionManager::new(
        std::env::temp_dir().join(format!("openz_subagent_allowlist_{}", uuid::Uuid::new_v4())),
    );
    register_all_tools(&registry, &config, provider, sessions).unwrap();
    let registered = registry.tool_names();

    for tool in crate::tools::subagent::delegate_profile::all_static_subagent_allowlist_tools() {
        assert!(
            registered.contains(&tool.to_string()),
            "subagent allowlist references unregistered tool: {tool}"
        );
    }

    for tool in crate::tools::subagent::parallel_research::read_only_tool_names() {
        assert!(
            registered.contains(&tool.to_string()),
            "parallel_research read-only allowlist references unregistered tool: {tool}"
        );
    }
}

#[test]
fn subagent_settings_validation_rejects_more_than_three_fallbacks() {
    let args = serde_json::json!({
        "fallbacks": ["a", "b", "c", "d"]
    });
    let result = super::optimize_profile::parse_subagent_settings(&args);
    assert!(result.is_err());
}

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
fn delegate_task_models_to_try_uses_override_fallbacks_then_default() {
    std::env::remove_var("OPENZ_MAX_FALLBACK_ATTEMPTS");
    let mut config = Config::default();
    config.agents.defaults.model = "opencode_zen/deepseek-v4-flash-free".to_string();
    config.agents.defaults.fallback_models = vec![
        serde_json::json!("google_ai_studio/gemini-2.5-flash"),
        serde_json::json!("mistral/mistral-large-latest"),
    ];

    let models = super::delegate_task::delegate_task_models_to_try(
        &config,
        Some("groq/llama-3.3-70b-versatile"),
        false,
    );

    assert_eq!(
        models,
        vec![
            "groq/llama-3.3-70b-versatile".to_string(),
            "google_ai_studio/gemini-2.5-flash".to_string(),
            "opencode_zen/deepseek-v4-flash-free".to_string(),
        ]
    );
}

#[test]
fn delegate_task_models_to_try_prefers_vision_fallback_for_images() {
    std::env::remove_var("OPENZ_MAX_FALLBACK_ATTEMPTS");
    let mut config = Config::default();
    config.agents.defaults.model = "opencode_zen/deepseek-v4-flash-free".to_string();
    config.agents.defaults.provider = "opencode_zen".to_string();
    config.providers.google_ai_studio = Some(crate::config::schema::ProviderConfig {
        api_key: Some("google-key".to_string()),
        api_key_env: None,
        api_key_file: None,
        api_base: None,
        default_model: None,
        extra: Default::default(),
    });
    config.agents.defaults.fallback_models = vec![
        serde_json::json!("deepseek/deepseek-chat"),
        serde_json::json!("mistral/pixtral-large-latest"),
    ];

    let models = super::delegate_task::delegate_task_models_to_try(&config, None, true);

    assert_eq!(models[0], "google_ai_studio/gemini-2.5-flash");
    assert!(models.contains(&"mistral/pixtral-large-latest".to_string()));
    assert!(models.contains(&"opencode_zen/deepseek-v4-flash-free".to_string()));
    assert!(!models.contains(&"deepseek/deepseek-chat".to_string()));
}

#[test]
fn evolution_gate_blocks_smoke_test_summary() {
    assert!(!should_run_evolution_review(
        "Run smoke test workflow",
        "demo",
        "Planner summary accurate. No issues.",
        false,
    ));
}

#[test]
fn evolution_gate_blocks_when_filesystem_writes_denied() {
    assert!(!should_run_evolution_review(
        "Refactor routing",
        "coding task",
        "When changing routing, add a focused regression test and verify the exposed tool list.",
        true,
    ));
}

#[test]
fn evolution_gate_allows_substantial_reusable_guidance() {
    assert!(should_run_evolution_review(
        "Refactor routing",
        "coding task",
        "When changing routing, add a focused regression test, inspect model-facing tool exposure, and verify the runtime lookup path uses the same policy.",
        false,
    ));
}

#[test]
fn skips_evolution_for_short_smoke_test_outputs() {
    assert!(crate::tools::subagent::should_skip_evolution_capture(
        "Summarize hello",
        "\"Hello\" means greeting."
    ));
}

#[test]
fn does_not_skip_evolution_for_substantial_new_skill_output() {
    let output = "A reliable code review workflow should inspect diffs, map risk areas, run focused tests, and report file-line findings with severity.";
    assert!(!crate::tools::subagent::should_skip_evolution_capture(
        "Design a reusable review workflow for Rust services",
        output
    ));
}

#[test]
fn simple_step_does_not_allow_nested_delegation() {
    assert!(!crate::tools::subagent::step_allows_nested_delegation(
        "Summarize hello"
    ));
}

#[test]
fn explicit_specialist_step_allows_nested_delegation() {
    assert!(crate::tools::subagent::step_allows_nested_delegation(
        "Delegate research to a specialist and summarize findings"
    ));
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
fn test_delegate_task_metadata_is_explicit_for_router() {
    let tool = DelegateTaskTool {
        config: Config::default(),
        parent_provider: Arc::new(crate::providers::openai::OpenAIProvider::new(
            "mock_key".to_string(),
            "mock_base".to_string(),
            "gpt-4o-mini".to_string(),
        )),
        session_manager: SessionManager::new(std::env::temp_dir()),
        parent_tools: Vec::new(),
        cancellation_token: CancellationToken::new(),
        capability_policy: None,
    };

    let metadata = tool.metadata();

    assert_eq!(metadata.domain, "subagent");
    assert_eq!(metadata.risk, crate::tools::ToolRisk::Medium);
    assert!(!metadata.spawns_process); // in-process child agent loop, no OS process
    assert!(!metadata.requires_approval);
    assert_eq!(metadata.priority, 100);
    assert_eq!(metadata.recommended_timeout_secs, Some(600));
}

#[test]
fn test_parallel_research_partial_response_shape() {
    let response = super::parallel_research::parallel_research_response(vec![
        serde_json::json!({
            "task": "marketplaces",
            "status": "success",
            "summary": "found sources"
        }),
        serde_json::json!({
            "task": "pricing",
            "status": "timeout",
            "error": "Parallel research aggregate deadline reached"
        }),
    ]);

    assert_eq!(response["status"], "partial_success");
    assert_eq!(response["succeeded"], 1);
    assert_eq!(response["failed"], 1);
    assert_eq!(response["results"][0]["summary"], "found sources");
    assert_eq!(response["results"][1]["status"], "timeout");
}

#[test]
fn test_parallel_research_flush_deadline_precedes_child_timeout() {
    assert!(super::parallel_research::parallel_research_flush_deadline_secs(300) < 300);
    assert_eq!(
        super::parallel_research::parallel_research_flush_deadline_secs(1),
        1
    );
}

#[test]
fn test_parallel_research_metadata_is_explicit_for_router() {
    let tool = ParallelResearchTool {
        config: Config::default(),
        parent_provider: Arc::new(crate::providers::openai::OpenAIProvider::new(
            "mock_key".to_string(),
            "mock_base".to_string(),
            "gpt-4o-mini".to_string(),
        )),
        session_manager: SessionManager::new(std::env::temp_dir()),
        parent_tools: Vec::new(),
        cancellation_token: CancellationToken::new(),
        capability_policy: None,
    };

    let metadata = tool.metadata();

    assert_eq!(metadata.domain, "subagent");
    assert_eq!(metadata.risk, crate::tools::ToolRisk::Medium);
    assert!(!metadata.spawns_process); // in-process child agent loop, no OS process
    assert!(!metadata.requires_approval);
    assert_eq!(metadata.priority, 100);
    assert_eq!(metadata.recommended_timeout_secs, Some(600));
}

#[tokio::test]
async fn test_cancellation_token_observes_cli_cancel_signal() {
    let _guard = cancel_test_guard().await;
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());

    crate::shutdown::trigger_cli_cancel();

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        token.wait_for_cancellation(),
    )
    .await
    .expect("token should observe CLI cancel signal");
    assert!(token.is_cancelled());
}

#[tokio::test]
async fn test_delegation_depth_limit() {
    let tool = DelegateTaskTool {
        config: Config::default(),
        parent_provider: Arc::new(crate::providers::openai::OpenAIProvider::new(
            "mock_key".to_string(),
            "mock_base".to_string(),
            "gpt-4o-mini".to_string(),
        )),
        session_manager: SessionManager::new(std::env::temp_dir()),
        parent_tools: Vec::new(),
        cancellation_token: CancellationToken::new(),
        capability_policy: None,
    };

    // If DELEGATION_DEPTH is 3, calling the tool should return an error immediately
    let res = DELEGATION_DEPTH
        .scope(3, async {
            tool.call(&serde_json::json!({
                "goal": "Test nested delegation safety"
            }))
            .await
        })
        .await;

    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Delegation limit reached"));
}

#[test]
fn test_validate_schema_success() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" },
            "tags": {
                "type": "array",
                "items": { "type": "string" }
            },
            "status": {
                "type": "string",
                "enum": ["active", "inactive"]
            }
        },
        "required": ["name", "age"]
    });

    let value = serde_json::json!({
        "name": "Aswin",
        "age": 25,
        "tags": ["rust", "ai"],
        "status": "active"
    });

    assert!(evaluator_optimizer::validate_schema(&value, &schema).is_ok());
}

#[test]
fn test_validate_schema_failure() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" }
        },
        "required": ["name", "age"]
    });

    // Missing required field
    let val_missing = serde_json::json!({
        "name": "Aswin"
    });
    assert!(evaluator_optimizer::validate_schema(&val_missing, &schema).is_err());

    // Incorrect type
    let val_bad_type = serde_json::json!({
        "name": "Aswin",
        "age": "twenty-five"
    });
    assert!(evaluator_optimizer::validate_schema(&val_bad_type, &schema).is_err());
}

#[test]
fn test_filter_tools_for_new_default_subagents() {
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(MockTool {
            name: "read_file".to_string(),
        }),
        Arc::new(MockTool {
            name: "write_file".to_string(),
        }),
        Arc::new(MockTool {
            name: "list_dir".to_string(),
        }),
        Arc::new(MockTool {
            name: "find_files".to_string(),
        }),
        Arc::new(MockTool {
            name: "read_doc".to_string(),
        }),
        Arc::new(MockTool {
            name: "exec_command".to_string(),
        }),
        Arc::new(MockTool {
            name: "generate_image".to_string(),
        }),
        Arc::new(MockTool {
            name: "onpkg".to_string(),
        }),
        Arc::new(MockTool {
            name: "code_outline".to_string(),
        }),
        Arc::new(MockTool {
            name: "cargo_manager".to_string(),
        }),
        Arc::new(MockTool {
            name: "grep_search".to_string(),
        }),
        Arc::new(MockTool {
            name: "compile_template".to_string(),
        }),
        Arc::new(MockTool {
            name: "some_other_tool".to_string(),
        }),
        Arc::new(MockTool {
            name: "openmedia_diagram_generate_mermaid".to_string(),
        }),
        Arc::new(MockTool {
            name: "openmedia_video_create".to_string(),
        }),
        Arc::new(MockTool {
            name: "openmedia_video_preview".to_string(),
        }),
    ];

    // Test document_compiler
    let filtered = delegate_profile::filter_tools_for_subagent("document_compiler", &tools);
    assert_eq!(filtered.len(), 7);
    assert!(filtered.iter().any(|t| t.name() == "compile_template"));
    assert!(filtered.iter().any(|t| t.name() == "read_doc"));
    assert!(!filtered.iter().any(|t| t.name() == "onpkg"));

    // Test presentation_designer
    let filtered = delegate_profile::filter_tools_for_subagent("presentation_designer", &tools);
    assert_eq!(filtered.len(), 7);
    assert!(filtered.iter().any(|t| t.name() == "compile_template"));
    assert!(filtered.iter().any(|t| t.name() == "generate_image"));
    assert!(!filtered.iter().any(|t| t.name() == "read_doc"));

    // Test code_synthesizer
    let filtered = delegate_profile::filter_tools_for_subagent("code_synthesizer", &tools);
    assert_eq!(filtered.len(), 7);
    assert!(filtered.iter().any(|t| t.name() == "onpkg"));
    assert!(!filtered.iter().any(|t| t.name() == "generate_image"));

    // Test summarizer_agent
    let filtered = delegate_profile::filter_tools_for_subagent("summarizer_agent", &tools);
    assert_eq!(filtered.len(), 4);
    assert!(filtered.iter().any(|t| t.name() == "grep_search"));
    assert!(!filtered.iter().any(|t| t.name() == "onpkg"));

    // Test vision_agent
    let filtered = delegate_profile::filter_tools_for_subagent("vision_agent", &tools);
    assert_eq!(filtered.len(), 5);
    assert!(filtered.iter().any(|t| t.name() == "generate_image"));
    assert!(!filtered.iter().any(|t| t.name() == "exec_command"));

    // Test skill_creator
    let filtered = delegate_profile::filter_tools_for_subagent("skill_creator", &tools);
    assert_eq!(filtered.len(), 5);
    assert!(filtered.iter().any(|t| t.name() == "exec_command"));
    assert!(!filtered.iter().any(|t| t.name() == "generate_image"));

    // Test documentation_agent
    let filtered = delegate_profile::filter_tools_for_subagent("documentation_agent", &tools);
    assert_eq!(filtered.len(), 4);
    assert!(filtered.iter().any(|t| t.name() == "read_file"));
    assert!(!filtered.iter().any(|t| t.name() == "exec_command"));

    // Test diagram_designer
    let filtered = delegate_profile::filter_tools_for_subagent("diagram_designer", &tools);
    assert_eq!(filtered.len(), 4);
    assert!(filtered
        .iter()
        .any(|t| t.name() == "openmedia_diagram_generate_mermaid"));
    assert!(!filtered.iter().any(|t| t.name() == "exec_command"));

    // Test video_animator
    let filtered = delegate_profile::filter_tools_for_subagent("video_animator", &tools);
    assert_eq!(filtered.len(), 5);
    assert!(filtered
        .iter()
        .any(|t| t.name() == "openmedia_video_create"));
    assert!(!filtered.iter().any(|t| t.name() == "exec_command"));
}

struct LoopMockProvider {
    call_count: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait::async_trait]
impl crate::providers::LLMProvider for LoopMockProvider {
    async fn chat(
        &self,
        system_prompt: &str,
        messages: &[crate::session::Message],
        _tools: &[serde_json::Value],
        _settings: &crate::providers::GenerationSettings,
    ) -> Result<crate::providers::LLMResponse> {
        let count = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        println!(
            "MOCK PROVIDER CHAT: count={}, system_prompt_len={}",
            count,
            system_prompt.len()
        );
        for (idx, msg) in messages.iter().enumerate() {
            println!(
                "  Message {}: role={}, content={}",
                idx, msg.role, msg.content
            );
        }

        // Check if it's the evaluator call by looking at system prompt or content
        let is_evaluator = system_prompt.contains("Review the draft produced by the optimizer")
            || messages.iter().any(|m| {
                m.content
                    .contains("Review the draft produced by the optimizer")
            });

        if is_evaluator {
            // Check if optimizer draft has "Draft version 0" (meaning iteration 1)
            let has_v0 = system_prompt.contains("Draft version 0")
                || messages
                    .iter()
                    .any(|m| m.content.contains("Draft version 0"));
            if has_v0 {
                Ok(crate::providers::LLMResponse {
                    content: Some(r#"{"passed": false, "feedback": "Draft needs more detail and standard hello function"}"#.to_string()),
                    tool_calls: Vec::new(),
                    finish_reason: "stop".to_string(),
                    reasoning_content: None,
                })
            } else {
                Ok(crate::providers::LLMResponse {
                    content: Some(r#"{"passed": true, "feedback": ""}"#.to_string()),
                    tool_calls: Vec::new(),
                    finish_reason: "stop".to_string(),
                    reasoning_content: None,
                })
            }
        } else {
            // Optimizer call
            Ok(crate::providers::LLMResponse {
                content: Some(format!("Draft version {}", count)),
                tool_calls: Vec::new(),
                finish_reason: "stop".to_string(),
                reasoning_content: None,
            })
        }
    }
}

#[tokio::test]
async fn test_evaluator_optimizer_loop_success() -> Result<()> {
    let _guard = cancel_test_guard().await;
    let temp_dir =
        std::env::temp_dir().join(format!("openz_eval_opt_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    std::env::set_var("ANTHROPIC_API_KEY", "dummy");
    std::env::set_var("OPENAI_API_KEY", "dummy");
    std::env::set_var("OPENZ_USE_MOCK_PROVIDER", "true");

    let provider = Arc::new(LoopMockProvider {
        call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    let tool = EvaluatorOptimizerLoopTool {
        config: Config::default(),
        parent_provider: provider.clone(),
        session_manager: SessionManager::new(temp_dir.clone()),
        parent_tools: Vec::new(),
        cancellation_token: CancellationToken::new(),
        capability_policy: None,
    };

    let res = crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async move {
            tool.call(&serde_json::json!({
                "optimizer": "coding_agent",
                "evaluator": "reviewer",
                "goal": "Write a hello world program in Rust",
                "checklist": "Must have a main function and print hello",
                "max_iterations": 3
            }))
            .await
        })
        .await?;

    assert_eq!(res.get("status").and_then(|v| v.as_str()), Some("success"));
    assert_eq!(res.get("passed").and_then(|v| v.as_bool()), Some(true));
    assert!(res.get("iterations_run").and_then(|v| v.as_i64()).unwrap() > 1);
    assert!(res
        .get("final_output")
        .and_then(|v| v.as_str())
        .unwrap()
        .contains("Draft version"));

    // Cleanup env vars
    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENZ_USE_MOCK_PROVIDER");
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

struct BlockingMockProvider {
    started_tx: tokio::sync::watch::Sender<bool>,
    release_rx: tokio::sync::watch::Receiver<bool>,
}

#[async_trait::async_trait]
impl crate::providers::LLMProvider for BlockingMockProvider {
    async fn chat(
        &self,
        _system_prompt: &str,
        _messages: &[crate::session::Message],
        _tools: &[serde_json::Value],
        _settings: &crate::providers::GenerationSettings,
    ) -> Result<crate::providers::LLMResponse> {
        let _ = self.started_tx.send(true);
        let mut release_rx = self.release_rx.clone();
        while !*release_rx.borrow() {
            if release_rx.changed().await.is_err() {
                break;
            }
        }
        Ok(crate::providers::LLMResponse {
            content: Some("should not complete after cancellation".to_string()),
            tool_calls: Vec::new(),
            finish_reason: "stop".to_string(),
            reasoning_content: None,
        })
    }
}

#[tokio::test]
async fn test_delegate_profile_rejects_explicitly_denied_profile() -> Result<()> {
    let profile = crate::subagents::SubagentProfile {
        name: "coding_agent".to_string(),
        description: "mock coding profile".to_string(),
        system_prompt: "mock".to_string(),
        model: None,
        fallbacks: None,
        extra: serde_json::Map::new(),
    };
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_delegate_profile_policy_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir)?;

    let tool = DelegateProfileTool {
        config: Config::default(),
        parent_provider: Arc::new(crate::providers::mock::MockProvider::new()),
        session_manager: SessionManager::new(temp_dir.clone()),
        profile,
        parent_tools: Vec::new(),
        cancellation_token: CancellationToken::new(),
        capability_policy: Some(crate::orchestrator::spec::CapabilityPolicy {
            allowed_tools: vec![],
            denied_tools: vec!["coding_agent".to_string()],
            deny_shell: false,
            deny_filesystem_write: false,
            deny_network: false,
        }),
    };

    let err = tool
        .call(&serde_json::json!({ "goal": "should be blocked" }))
        .await
        .expect_err("explicit denied_tools should block subagent profiles before execution");
    assert!(err
        .to_string()
        .contains("blocked by orchestrator capability policy"));

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_evaluator_optimizer_rejects_denied_optimizer_profile() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_eval_opt_policy_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir)?;

    let tool = EvaluatorOptimizerLoopTool {
        config: Config::default(),
        parent_provider: Arc::new(crate::providers::mock::MockProvider::new()),
        session_manager: SessionManager::new(temp_dir.clone()),
        parent_tools: Vec::new(),
        cancellation_token: CancellationToken::new(),
        capability_policy: Some(crate::orchestrator::spec::CapabilityPolicy {
            allowed_tools: vec![],
            denied_tools: vec!["coding_agent".to_string()],
            deny_shell: false,
            deny_filesystem_write: false,
            deny_network: false,
        }),
    };

    let err = crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            tool.call(&serde_json::json!({
                "optimizer": "coding_agent",
                "evaluator": "reviewer",
                "goal": "produce a draft",
                "checklist": "must pass"
            }))
            .await
        })
        .await
        .expect_err("denied optimizer profile should be rejected before execution");
    assert!(err
        .to_string()
        .contains("blocked by orchestrator capability policy"));

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_delegate_task_cancels_while_child_run_is_active() -> Result<()> {
    let _guard = cancel_test_guard().await;
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_delegate_active_cancel_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir)?;

    std::env::set_var("ANTHROPIC_API_KEY", "dummy");
    std::env::set_var("OPENAI_API_KEY", "dummy");
    std::env::set_var("OPENZ_USE_MOCK_PROVIDER", "true");

    let (started_tx, mut started_rx) = tokio::sync::watch::channel(false);
    let (_release_tx, release_rx) = tokio::sync::watch::channel(false);
    let provider = Arc::new(BlockingMockProvider {
        started_tx,
        release_rx,
    });
    let cancellation_token = CancellationToken::new();

    let tool = DelegateTaskTool {
        config: Config::default(),
        parent_provider: provider,
        session_manager: SessionManager::new(temp_dir.clone()),
        parent_tools: Vec::new(),
        cancellation_token: cancellation_token.clone(),
        capability_policy: None,
    };

    let temp_for_task = temp_dir.clone();
    let handle = tokio::spawn(async move {
        crate::config::loader::CONFIG_DIR_OVERRIDE
            .scope(temp_for_task, async move {
                tool.call(&serde_json::json!({
                    "goal": "Block until cancelled",
                    "context": "This test cancels after the subagent starts"
                }))
                .await
            })
            .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !*started_rx.borrow() {
            started_rx.changed().await.unwrap();
        }
    })
    .await
    .expect("subagent child run should start before cancellation");

    let cancel_start = std::time::Instant::now();
    cancellation_token.cancel();
    let res = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("active subagent cancellation should not wait for timeout")
        .expect("delegate task join should complete");

    assert!(
        cancel_start.elapsed() < std::time::Duration::from_secs(2),
        "active cancellation should return promptly"
    );
    let value = res.expect("active cancellation should return structured JSON");
    assert_eq!(value["status"], "cancelled");
    assert_eq!(value["lifecycle"]["code"], "cancelled");
    assert_eq!(value["lifecycle"]["label"], "cancelled");
    assert_eq!(value["tool"], "delegate_task");
    assert!(value["session_id"]
        .as_str()
        .is_some_and(|id| !id.is_empty()));
    assert!(value["model_used"]
        .as_str()
        .is_some_and(|model| !model.is_empty()));
    assert!(
        value["error"]
            .as_str()
            .is_some_and(|error| error.contains("cancelled")),
        "cancellation result should retain the cancellation reason: {value:?}"
    );

    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENZ_USE_MOCK_PROVIDER");
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_delegate_task_cancellation_propagation() -> Result<()> {
    let _guard = cancel_test_guard().await;
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_delegate_cancel_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir)?;

    std::env::set_var("ANTHROPIC_API_KEY", "dummy");
    std::env::set_var("OPENAI_API_KEY", "dummy");
    std::env::set_var("OPENZ_USE_MOCK_PROVIDER", "true");

    let provider = Arc::new(LoopMockProvider {
        call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    let cancellation_token = CancellationToken::new();
    cancellation_token.cancel(); // Cancel it immediately!

    let tool = DelegateTaskTool {
        config: Config::default(),
        parent_provider: provider.clone(),
        session_manager: SessionManager::new(temp_dir.clone()),
        parent_tools: Vec::new(),
        cancellation_token,
        capability_policy: None,
    };

    let res = crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async move {
            tool.call(&serde_json::json!({
                "goal": "Write a hello world program in Rust",
                "context": "Keep it simple"
            }))
            .await
        })
        .await;

    let value = res.expect("cancelled task should return structured JSON");
    assert_eq!(value["status"], "cancelled");
    assert_eq!(value["lifecycle"]["code"], "cancelled");
    assert_eq!(value["tool"], "delegate_task");
    assert!(value["session_id"]
        .as_str()
        .is_some_and(|id| !id.is_empty()));
    assert!(value["model_used"]
        .as_str()
        .is_some_and(|model| !model.is_empty()));

    // Cleanup env vars
    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENZ_USE_MOCK_PROVIDER");
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_delegate_profile_cancels_while_child_run_is_active() -> Result<()> {
    let _guard = cancel_test_guard().await;
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_delegate_profile_active_cancel_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir)?;

    std::env::set_var("ANTHROPIC_API_KEY", "dummy");
    std::env::set_var("OPENAI_API_KEY", "dummy");
    std::env::set_var("OPENZ_USE_MOCK_PROVIDER", "true");

    let (started_tx, mut started_rx) = tokio::sync::watch::channel(false);
    let (_release_tx, release_rx) = tokio::sync::watch::channel(false);
    let provider = Arc::new(BlockingMockProvider {
        started_tx,
        release_rx,
    });
    let cancellation_token = CancellationToken::new();
    let profile = crate::subagents::SubagentProfile {
        name: "test_subagent".to_string(),
        description: "test subagent description".to_string(),
        system_prompt: "you are a test subagent".to_string(),
        model: Some("gpt-4o-mini".to_string()),
        fallbacks: None,
        extra: serde_json::Map::new(),
    };

    let tool = DelegateProfileTool {
        config: Config::default(),
        parent_provider: provider,
        session_manager: SessionManager::new(temp_dir.clone()),
        profile,
        parent_tools: Vec::new(),
        cancellation_token: cancellation_token.clone(),
        capability_policy: None,
    };

    let temp_for_task = temp_dir.clone();
    let handle = tokio::spawn(async move {
        crate::config::loader::CONFIG_DIR_OVERRIDE
            .scope(temp_for_task, async move {
                tool.call(&serde_json::json!({
                    "goal": "Block until cancelled",
                    "context": "This test cancels after the profile subagent starts"
                }))
                .await
            })
            .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !*started_rx.borrow() {
            started_rx.changed().await.unwrap();
        }
    })
    .await
    .expect("profile child run should start before cancellation");

    let cancel_start = std::time::Instant::now();
    cancellation_token.cancel();
    let res = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("active profile cancellation should not wait for timeout")
        .expect("profile delegate join should complete");

    assert!(
        cancel_start.elapsed() < std::time::Duration::from_secs(2),
        "active profile cancellation should return promptly"
    );
    let value = res.expect("active profile cancellation should return structured JSON");
    assert_eq!(value["status"], "cancelled");
    assert_eq!(value["lifecycle"]["code"], "cancelled");
    assert_eq!(value["lifecycle"]["label"], "cancelled");
    assert_eq!(value["tool"], "delegate_profile");
    assert_eq!(value["subagent"], "test_subagent");
    assert!(value["session_id"]
        .as_str()
        .is_some_and(|id| !id.is_empty()));
    assert!(value["model_used"]
        .as_str()
        .is_some_and(|model| !model.is_empty()));
    assert!(
        value["error"]
            .as_str()
            .is_some_and(|error| error.contains("cancelled")),
        "profile cancellation result should retain the cancellation reason: {value:?}"
    );

    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENZ_USE_MOCK_PROVIDER");
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_delegate_profile_cancellation_propagation() -> Result<()> {
    let _guard = cancel_test_guard().await;
    let temp_dir = std::env::temp_dir().join(format!(
        "openz_delegate_profile_cancel_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir)?;

    std::env::set_var("ANTHROPIC_API_KEY", "dummy");
    std::env::set_var("OPENAI_API_KEY", "dummy");
    std::env::set_var("OPENZ_USE_MOCK_PROVIDER", "true");

    let provider = Arc::new(LoopMockProvider {
        call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });

    let cancellation_token = CancellationToken::new();
    cancellation_token.cancel(); // Cancel it immediately!

    let profile = crate::subagents::SubagentProfile {
        name: "test_subagent".to_string(),
        description: "test subagent description".to_string(),
        system_prompt: "you are a test subagent".to_string(),
        model: Some("gpt-4o-mini".to_string()),
        fallbacks: None,
        extra: serde_json::Map::new(),
    };

    let tool = DelegateProfileTool {
        config: Config::default(),
        parent_provider: provider.clone(),
        session_manager: SessionManager::new(temp_dir.clone()),
        profile,
        parent_tools: Vec::new(),
        cancellation_token,
        capability_policy: None,
    };

    let res = crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async move {
            tool.call(&serde_json::json!({
                "goal": "Write a hello world program in Rust",
                "context": "Keep it simple"
            }))
            .await
        })
        .await;

    let value = res.expect("cancelled profile should return structured JSON");
    assert_eq!(value["status"], "cancelled");
    assert_eq!(value["lifecycle"]["code"], "cancelled");
    assert_eq!(value["tool"], "delegate_profile");
    assert_eq!(value["subagent"], "test_subagent");
    assert!(value["session_id"]
        .as_str()
        .is_some_and(|id| !id.is_empty()));
    assert!(value["model_used"]
        .as_str()
        .is_some_and(|model| !model.is_empty()));

    // Cleanup env vars
    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENZ_USE_MOCK_PROVIDER");
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn filesystem_write_denied_policy_helper_detects_denial() {
    use crate::orchestrator::spec::CapabilityPolicy;
    assert!(!super::filesystem_write_denied_by_policy(&None));
    assert!(!super::filesystem_write_denied_by_policy(&Some(
        CapabilityPolicy::default()
    )));
    assert!(super::filesystem_write_denied_by_policy(&Some(
        CapabilityPolicy {
            deny_filesystem_write: true,
            ..Default::default()
        }
    )));
}




#[test]
fn test_ensure_markdown_images_wraps_paths_and_urls() {
    let input = "Check this file: /tmp/screenshot.png and url https://example.com/image.jpg";
    let formatted = super::ensure_markdown_images(input);
    assert!(formatted.contains("![](file:///tmp/screenshot.png)") || formatted.contains("![](/tmp/screenshot.png)"));
    assert!(formatted.contains("![](https://example.com/image.jpg)"));

    // Already formatted markdown image should not be double-wrapped
    let already = "Here is ![label](https://example.com/pic.png)";
    let formatted_already = super::ensure_markdown_images(already);
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

    let prompt = super::build_subagent_prompt(
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
    use crate::agent::AgentLoop;
    use crate::providers::mock::MockProvider;
    use crate::tools::ToolRegistry;

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

    let attempt = super::SubagentRunAttempt {
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

    let outcome = super::run_subagent_attempt(&child_agent, &parent_provider, &token, attempt).await;
    match outcome {
        super::SubagentRunOutcome::Cancelled(val) => {
            assert_eq!(val["status"], "cancelled");
            assert_eq!(val["lifecycle"]["code"], "cancelled");
            assert_eq!(val["tool"], "delegate_task");
        }
        other => panic!("Expected Cancelled outcome, got: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

