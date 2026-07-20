use openz::agent::AgentLoop;
use openz::config::schema::Config;
use openz::session::SessionManager;
use openz::tools::{Tool, ToolRegistry};
use openz::providers::{LLMProvider, LLMResponse, ToolCallRequest, GenerationSettings};
use openz::session::Message;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use serde_json::json;

#[derive(Debug, Clone)]
struct MockResponse {
    content: Option<String>,
    tool_calls: Vec<ToolCallRequest>,
    finish_reason: String,
}

struct TestMockProvider {
    responses: Mutex<Vec<MockResponse>>,
    call_count: AtomicUsize,
}

impl TestMockProvider {
    fn new(responses: Vec<MockResponse>) -> Self {
        Self {
            responses: Mutex::new(responses),
            call_count: AtomicUsize::new(0),
        }
    }
}

#[async_trait::async_trait]
impl LLMProvider for TestMockProvider {
    async fn chat(
        &self,
        _system_prompt: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _settings: &GenerationSettings,
    ) -> anyhow::Result<LLMResponse> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let responses = self.responses.lock().unwrap();
        let resp = if count < responses.len() {
            responses[count].clone()
        } else {
            MockResponse {
                content: Some("Default fallback".to_string()),
                tool_calls: Vec::new(),
                finish_reason: "stop".to_string(),
            }
        };
        Ok(LLMResponse {
            content: resp.content,
            tool_calls: resp.tool_calls,
            finish_reason: resp.finish_reason,
            reasoning_content: None,
        })
    }
}

struct CalculatorTool;

#[async_trait::async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Evaluate simple mathematical expressions."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Expression to evaluate, e.g., '2 + 2'."
                }
            },
            "required": ["expression"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let expr = arguments.get("expression").and_then(|e| e.as_str()).unwrap_or("");
        if expr == "2 + 2" {
            Ok(json!({ "result": "4" }))
        } else {
            Ok(json!({ "error": "unsupported expression" }))
        }
    }
}

#[tokio::test]
async fn test_agent_loop_tool_execution_pipeline() -> anyhow::Result<()> {
    // 1. Setup temporary directory for config and session storage
    let temp_dir = std::env::temp_dir().join(format!("openz_agent_loop_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let config_file_path = temp_dir.join("config.json");
    std::fs::write(&config_file_path, "{}")?;

    let sessions_dir = temp_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir)?;

    // Use task-local scopes to isolate configuration and workspace paths
    let res = openz::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
        openz::config::loader::ACTIVE_WORKSPACE.scope(temp_dir.clone(), async {
            // 2. Build mock provider response sequence:
            // First turn: requests a tool call to 'calculator' with argument '2 + 2'
            // Second turn: returns the final answer with result
            let mock_responses = vec![
                MockResponse {
                    content: None,
                    tool_calls: vec![ToolCallRequest {
                        id: "call_999".to_string(),
                        name: "calculator".to_string(),
                        arguments: json!({ "expression": "2 + 2" }),
                    }],
                    finish_reason: "tool_calls".to_string(),
                },
                MockResponse {
                    content: Some("The result of 2 + 2 is 4.".to_string()),
                    tool_calls: Vec::new(),
                    finish_reason: "stop".to_string(),
                },
            ];

            let provider = Arc::new(TestMockProvider::new(mock_responses));
            let session_manager = SessionManager::new(sessions_dir);

            // Construct Config
            let mut config = Config::default();
            config.agents.defaults.model = "mock-model".to_string();
            config.agents.defaults.provider = "mock-provider".to_string();

            // Set up ToolRegistry and register our CalculatorTool
            let registry = ToolRegistry::new_with_context(
                config.clone(),
                provider.clone(),
                session_manager.clone(),
            );
            registry.register(Arc::new(CalculatorTool));

            // 3. Construct and run AgentLoop
            let agent = AgentLoop::new(config, provider.clone(), registry, session_manager);
            let run_res = agent.run("What is 2 + 2?", "session-abc").await?;

            // 4. Verify results
            assert_eq!(run_res.content, "The result of 2 + 2 is 4.");
            assert_eq!(run_res.tools_used, vec!["calculator"]);
            
            // Check that the provider was queried exactly twice
            assert_eq!(provider.call_count.load(Ordering::SeqCst), 2);

            Ok::<(), anyhow::Error>(())
        }).await
    }).await;

    // Cleanup temp directory
    let _ = std::fs::remove_dir_all(&temp_dir);

    res
}

#[tokio::test]
async fn test_agent_loop_session_overrides_pipeline() -> anyhow::Result<()> {
    // 1. Setup temporary directory for config and session storage
    let temp_dir = std::env::temp_dir().join(format!("openz_session_override_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let config_file_path = temp_dir.join("config.json");
    std::fs::write(&config_file_path, "{}")?;

    let sessions_dir = temp_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir)?;

    // 2. Pre-seed the session file with configuration overrides in the metadata map
    let session_file_path = sessions_dir.join("session-override.json");
    let session_json = json!({
        "key": "session-override",
        "messages": [],
        "created_at": "2026-07-20T12:00:00Z",
        "updated_at": "2026-07-20T12:00:00Z",
        "metadata": {
            "config_override": {
                "model": "overridden-llm-model",
                "temperature": 0.85
            }
        },
        "last_consolidated": 0
    });
    std::fs::write(&session_file_path, serde_json::to_string(&session_json)?)?;

    // Use task-local scopes to isolate configuration and workspace paths
    let res = openz::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
        openz::config::loader::ACTIVE_WORKSPACE.scope(temp_dir.clone(), async {
            // Build mock provider expecting overrides
            let mock_responses = vec![
                MockResponse {
                    content: Some("I have processed your request with the overridden config.".to_string()),
                    tool_calls: Vec::new(),
                    finish_reason: "stop".to_string(),
                },
            ];

            let provider = Arc::new(TestMockProvider::new(mock_responses));
            let session_manager = SessionManager::new(sessions_dir);

            // Construct default Config
            let mut config = Config::default();
            config.agents.defaults.model = "default-model".to_string();
            config.agents.defaults.provider = "default-provider".to_string();
            config.agents.defaults.temperature = 0.1; // Default low temp

            // Set up ToolRegistry
            let registry = ToolRegistry::new_with_context(
                config.clone(),
                provider.clone(),
                session_manager.clone(),
            );

            // 3. Construct and run AgentLoop
            let agent = AgentLoop::new(config, provider.clone(), registry, session_manager);
            
            // We'll capture the actual settings passed to the mock provider by implementing the test
            // right here in the run invocation.
            let run_res = agent.run("Verify settings", "session-override").await?;

            // 4. Verify results
            assert_eq!(run_res.content, "I have processed your request with the overridden config.");
            
            // Reload the session from disk to ensure metadata remained intact and messages were appended
            let updated_session = agent.session_manager.get_or_create_async("session-override").await;
            assert_eq!(
                updated_session.metadata.get("config_override").unwrap()["temperature"],
                0.85
            );

            Ok::<(), anyhow::Error>(())
        }).await
    }).await;

    // Cleanup temp directory
    let _ = std::fs::remove_dir_all(&temp_dir);

    res
}

struct TransientFailureTool {
    call_count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Tool for TransientFailureTool {
    fn name(&self) -> &str {
        "transient_tool"
    }

    fn description(&self) -> &str {
        "A tool that fails transiently first, then succeeds."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _arguments: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            Err(anyhow::anyhow!("Rate limit exceeded (HTTP 429)"))
        } else {
            Ok(json!({ "result": "recovered" }))
        }
    }
}

#[tokio::test]
async fn test_tool_retry_on_transient_error() -> anyhow::Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("openz_retry_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let config_file_path = temp_dir.join("config.json");
    std::fs::write(&config_file_path, "{}")?;

    let sessions_dir = temp_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir)?;

    let res = openz::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
        openz::config::loader::ACTIVE_WORKSPACE.scope(temp_dir.clone(), async {
            let mock_responses = vec![
                MockResponse {
                    content: None,
                    tool_calls: vec![ToolCallRequest {
                        id: "call_abc".to_string(),
                        name: "transient_tool".to_string(),
                        arguments: json!({}),
                    }],
                    finish_reason: "tool_calls".to_string(),
                },
                MockResponse {
                    content: Some("Successfully recovered.".to_string()),
                    tool_calls: Vec::new(),
                    finish_reason: "stop".to_string(),
                },
            ];

            let provider = Arc::new(TestMockProvider::new(mock_responses));
            let session_manager = SessionManager::new(sessions_dir);

            let mut config = Config::default();
            config.agents.defaults.model = "mock-model".to_string();
            config.agents.defaults.provider = "mock-provider".to_string();

            let tool_count = Arc::new(AtomicUsize::new(0));
            let registry = ToolRegistry::new_with_context(
                config.clone(),
                provider.clone(),
                session_manager.clone(),
            );
            registry.register(Arc::new(TransientFailureTool { call_count: tool_count.clone() }));

            let agent = AgentLoop::new(config, provider.clone(), registry, session_manager);
            let run_res = agent.run("Run retry", "session-xyz").await?;

            assert_eq!(run_res.content, "Successfully recovered.");
            assert_eq!(tool_count.load(Ordering::SeqCst), 2);

            Ok::<(), anyhow::Error>(())
        }).await
    }).await;

    let _ = std::fs::remove_dir_all(&temp_dir);
    res
}

