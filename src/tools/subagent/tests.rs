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

