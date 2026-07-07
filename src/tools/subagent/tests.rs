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
