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
    };

    let metadata = tool.metadata();

    assert_eq!(metadata.domain, "subagent");
    assert_eq!(metadata.risk, crate::tools::ToolRisk::Medium);
    assert!(metadata.spawns_process);
    assert!(!metadata.requires_approval);
    assert_eq!(metadata.priority, 100);
    assert_eq!(metadata.recommended_timeout_secs, Some(600));
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
    };

    let metadata = tool.metadata();

    assert_eq!(metadata.domain, "subagent");
    assert_eq!(metadata.risk, crate::tools::ToolRisk::Medium);
    assert!(metadata.spawns_process);
    assert!(!metadata.requires_approval);
    assert_eq!(metadata.priority, 100);
    assert_eq!(metadata.recommended_timeout_secs, Some(600));
}

#[test]
fn test_lifecycle_status_labels_are_stable_for_tui() {
    use super::lifecycle::SubagentRunStatus;

    assert_eq!(SubagentRunStatus::Queued.label(), "queued");
    assert_eq!(SubagentRunStatus::Running.label(), "running");
    assert_eq!(
        SubagentRunStatus::Fallback {
            model: "gemini".into(),
            attempt: 1,
            total: 3
        }
        .label(),
        "fallback 1/3: gemini"
    );
    assert_eq!(SubagentRunStatus::Cancelling.label(), "cancelling");
    assert_eq!(SubagentRunStatus::Cancelled.label(), "cancelled");
    assert_eq!(
        SubagentRunStatus::TimedOut {
            duration_secs: None
        }
        .label(),
        "timed out"
    );
    assert_eq!(
        SubagentRunStatus::Failed {
            error: "boom".into()
        }
        .label(),
        "failed: boom"
    );
    assert_eq!(SubagentRunStatus::Completed.label(), "completed");
}

#[test]
fn test_compact_lifecycle_line_for_cancellation_is_stable() {
    use super::lifecycle::{compact_lifecycle_line, SubagentRunStatus};

    let line = compact_lifecycle_line(
        "vision_agent",
        "google_ai_studio/gemini-2.5-flash",
        &SubagentRunStatus::Cancelling,
    );

    assert_eq!(
        line,
        "vision_agent | google_ai_studio/gemini-2.5-flash | cancelling"
    );
    assert!(!line.contains("Running..."));
}

#[test]
fn test_lifecycle_classifies_timeout_without_user_cancel() {
    use super::lifecycle::{classify_subagent_error, SubagentRunStatus};
    let token = CancellationToken::new();

    assert_eq!(
        classify_subagent_error("Subagent execution timed out after 5 minutes", &token),
        SubagentRunStatus::TimedOut {
            duration_secs: Some(300)
        }
    );
}

#[test]
fn test_lifecycle_classifies_timeout_duration_seconds() {
    use super::lifecycle::{classify_subagent_error, SubagentRunStatus};
    let token = CancellationToken::new();

    assert_eq!(
        classify_subagent_error("Subagent execution timed out after 900s", &token),
        SubagentRunStatus::TimedOut {
            duration_secs: Some(900)
        }
    );
}

#[test]
fn test_lifecycle_timeout_status_json_includes_duration() {
    use super::lifecycle::{status_json, SubagentRunStatus};

    let value = status_json(&SubagentRunStatus::TimedOut {
        duration_secs: Some(900),
    });

    assert_eq!(value["code"], "timed_out");
    assert_eq!(value["label"], "timed out after 900s");
    assert_eq!(value["durationSecs"], 900);
}

#[test]
fn test_compact_lifecycle_line_includes_timeout_duration() {
    use super::lifecycle::{compact_lifecycle_line, SubagentRunStatus};

    let line = compact_lifecycle_line(
        "delegate_task",
        "deepseek/deepseek-chat",
        &SubagentRunStatus::TimedOut {
            duration_secs: Some(900),
        },
    );

    assert_eq!(
        line,
        "delegate_task | deepseek/deepseek-chat | timed out after 900s"
    );
}

#[test]
fn test_lifecycle_classifies_user_cancel_from_token() {
    use super::lifecycle::{classify_subagent_error, SubagentRunStatus};
    let token = CancellationToken::new();
    token.cancel();

    assert_eq!(
        classify_subagent_error("provider returned 429", &token),
        SubagentRunStatus::Cancelled
    );
}

#[test]
fn test_worktree_cleanup_removes_old_openz_worktrees() -> Result<()> {
    let root = std::env::temp_dir().join(format!(
        "openz_worktree_cleanup_age_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root)?;
    let old_dir = root.join("openz_worktree_old");
    let fresh_dir = root.join("openz_worktree_fresh");
    let unrelated_dir = root.join("not_openz_worktree_old");
    std::fs::create_dir_all(&old_dir)?;
    std::fs::create_dir_all(&fresh_dir)?;
    std::fs::create_dir_all(&unrelated_dir)?;
    std::fs::write(old_dir.join("data.txt"), b"old")?;
    std::fs::write(fresh_dir.join("data.txt"), b"fresh")?;
    std::fs::write(unrelated_dir.join("data.txt"), b"keep")?;

    let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
    delegate_task::set_directory_modified_time_for_test(&old_dir, old_time)?;

    delegate_task::cleanup_worktrees_dir(
        &root,
        delegate_task::WorktreeCleanupPolicy {
            max_age: std::time::Duration::from_secs(3600),
            max_count: 10,
            max_total_bytes: 1024 * 1024,
            min_free_bytes: 0,
        },
    );

    assert!(!old_dir.exists(), "old OpenZ worktree should be deleted");
    assert!(fresh_dir.exists(), "fresh OpenZ worktree should be kept");
    assert!(
        unrelated_dir.exists(),
        "non-OpenZ directory must never be deleted"
    );
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn test_worktree_cleanup_enforces_total_size_quota_oldest_first() -> Result<()> {
    let root = std::env::temp_dir().join(format!(
        "openz_worktree_cleanup_quota_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root)?;
    let old_dir = root.join("openz_worktree_old");
    let mid_dir = root.join("openz_worktree_mid");
    let new_dir = root.join("openz_worktree_new");
    for dir in [&old_dir, &mid_dir, &new_dir] {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("blob.bin"), vec![0_u8; 1024])?;
    }

    let now = std::time::SystemTime::now();
    delegate_task::set_directory_modified_time_for_test(
        &old_dir,
        now - std::time::Duration::from_secs(300),
    )?;
    delegate_task::set_directory_modified_time_for_test(
        &mid_dir,
        now - std::time::Duration::from_secs(200),
    )?;
    delegate_task::set_directory_modified_time_for_test(
        &new_dir,
        now - std::time::Duration::from_secs(100),
    )?;

    delegate_task::cleanup_worktrees_dir(
        &root,
        delegate_task::WorktreeCleanupPolicy {
            max_age: std::time::Duration::from_secs(3600),
            max_count: 10,
            max_total_bytes: 2048,
            min_free_bytes: 0,
        },
    );

    assert!(
        !old_dir.exists(),
        "oldest worktree should be deleted to satisfy size quota"
    );
    assert!(mid_dir.exists(), "middle worktree should remain");
    assert!(new_dir.exists(), "newest worktree should remain");
    assert!(delegate_task::directory_size_bytes(&root) <= 2048);
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[tokio::test]
async fn test_create_isolated_workspace_rejects_home_like_non_git_root() -> Result<()> {
    let _lock = crate::tools::graph_memory::test_lock().lock().await;
    let temp_root = std::env::temp_dir().join(format!(
        "openz_home_like_worktree_guard_{}",
        uuid::Uuid::new_v4()
    ));
    let fake_home = temp_root.join("home");
    let config_dir = temp_root.join("openz_config");
    std::fs::create_dir_all(fake_home.join(".cache/big"))?;
    std::fs::create_dir_all(&config_dir)?;
    std::fs::write(fake_home.join(".cache/big/blob.bin"), vec![0_u8; 1024])?;

    let old_home = std::env::var_os("HOME");
    std::env::set_var("HOME", &fake_home);

    let res = crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(config_dir.clone(), async {
            delegate_task::create_isolated_workspace(&fake_home)
        })
        .await;

    if let Some(old_home) = old_home {
        std::env::set_var("HOME", old_home);
    } else {
        std::env::remove_var("HOME");
    }

    assert!(
        res.is_err(),
        "home-like roots must not be recursively copied"
    );
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("unsafe workspace root"),
        "unexpected error: {err}"
    );
    assert!(
        err.contains("cd into a project git repository"),
        "error should include actionable guidance: {err}"
    );
    assert!(
        err.contains("disables isolation"),
        "error should warn about fallback behavior: {err}"
    );
    assert!(
        !config_dir.join("worktrees").exists()
            || std::fs::read_dir(config_dir.join("worktrees"))?
                .next()
                .is_none(),
        "rejecting a home-like root must not leave a worktree behind"
    );

    let _ = std::fs::remove_dir_all(&temp_root);
    Ok(())
}

#[test]
fn test_fallback_copy_skips_heavy_user_cache_dirs() -> Result<()> {
    let src =
        std::env::temp_dir().join(format!("openz_filtered_copy_src_{}", uuid::Uuid::new_v4()));
    let dst =
        std::env::temp_dir().join(format!("openz_filtered_copy_dst_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(src.join("src"))?;
    std::fs::create_dir_all(src.join(".cache/huge"))?;
    std::fs::create_dir_all(src.join(".local/share"))?;
    std::fs::create_dir_all(src.join(".openz/worktrees"))?;
    std::fs::create_dir_all(src.join("Downloads"))?;
    std::fs::write(src.join("src/main.rs"), "fn main() {}")?;
    std::fs::write(src.join(".cache/huge/blob.bin"), vec![0_u8; 1024])?;
    std::fs::write(src.join(".local/share/blob.bin"), vec![0_u8; 1024])?;
    std::fs::write(src.join(".openz/worktrees/blob.bin"), vec![0_u8; 1024])?;
    std::fs::write(src.join("Downloads/blob.bin"), vec![0_u8; 1024])?;

    delegate_task::copy_dir_recursive_filtered(&src, &dst)?;

    assert!(dst.join("src/main.rs").exists());
    assert!(!dst.join(".cache").exists());
    assert!(!dst.join(".local").exists());
    assert!(!dst.join(".openz").exists());
    assert!(!dst.join("Downloads").exists());

    let _ = std::fs::remove_dir_all(&src);
    let _ = std::fs::remove_dir_all(&dst);
    Ok(())
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
fn test_schema_retry_accepts_valid_fenced_json() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "answer": { "type": "string" } },
        "required": ["answer"]
    });

    let decision = schema_retry::evaluate_schema_retry(
        r#"```json
{"answer":"ok"}
```"#,
        &schema,
        0,
        2,
    )
    .unwrap();

    assert_eq!(
        decision,
        schema_retry::SchemaRetryDecision::Accepted(r#"{"answer":"ok"}"#.to_string())
    );
}

#[test]
fn test_schema_retry_retries_invalid_json_before_limit() {
    let schema = serde_json::json!({ "type": "object" });

    let decision = schema_retry::evaluate_schema_retry("not json", &schema, 0, 2).unwrap();

    match decision {
        schema_retry::SchemaRetryDecision::Retry { prompt, reason } => {
            assert!(reason.contains("Parse Error"));
            assert!(prompt.contains("not valid JSON"));
        }
        other => panic!("expected retry decision, got {other:?}"),
    }
}

#[test]
fn test_schema_retry_errors_invalid_json_at_limit() {
    let schema = serde_json::json!({ "type": "object" });

    let err = schema_retry::evaluate_schema_retry("not json", &schema, 2, 2).unwrap_err();

    assert!(err.to_string().contains("failed to parse as JSON"));
}

#[test]
fn test_schema_retry_retries_schema_mismatch_before_limit() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "answer": { "type": "string" } },
        "required": ["answer"]
    });

    let decision = schema_retry::evaluate_schema_retry("{}", &schema, 0, 2).unwrap();

    match decision {
        schema_retry::SchemaRetryDecision::Retry { prompt, reason } => {
            assert!(reason.contains("Missing required field"));
            assert!(prompt.contains("did not conform"));
        }
        other => panic!("expected retry decision, got {other:?}"),
    }
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
            name: "doc_reader".to_string(),
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
    assert!(filtered.iter().any(|t| t.name() == "doc_reader"));
    assert!(!filtered.iter().any(|t| t.name() == "onpkg"));

    // Test presentation_designer
    let filtered = delegate_profile::filter_tools_for_subagent("presentation_designer", &tools);
    assert_eq!(filtered.len(), 7);
    assert!(filtered.iter().any(|t| t.name() == "compile_template"));
    assert!(filtered.iter().any(|t| t.name() == "generate_image"));
    assert!(!filtered.iter().any(|t| t.name() == "doc_reader"));

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
