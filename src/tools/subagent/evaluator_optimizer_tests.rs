use super::*;
use crate::config::schema::Config;
use crate::session::SessionManager;
use crate::tools::Tool;
use anyhow::Result;
use std::sync::Arc;

static TEST_CANCEL_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

async fn cancel_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_CANCEL_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
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

    assert!(validate_schema(&value, &schema).is_ok());
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
    assert!(validate_schema(&val_missing, &schema).is_err());

    // Incorrect type
    let val_bad_type = serde_json::json!({
        "name": "Aswin",
        "age": "twenty-five"
    });
    assert!(validate_schema(&val_bad_type, &schema).is_err());
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
