use super::*;
use crate::config::schema::Config;
use crate::session::SessionManager;
use anyhow::Result;
use std::sync::Arc;

use crate::tools::subagent::cancel_test_guard;

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

struct LoopMockProvider {
    call_count: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait::async_trait]
impl crate::providers::LLMProvider for LoopMockProvider {
    async fn chat(
        &self,
        _system_prompt: &str,
        _messages: &[crate::session::Message],
        _tools: &[serde_json::Value],
        _settings: &crate::providers::GenerationSettings,
    ) -> Result<crate::providers::LLMResponse> {
        let count = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(crate::providers::LLMResponse {
            content: Some(format!("Draft version {}", count)),
            tool_calls: Vec::new(),
            finish_reason: "stop".to_string(),
            reasoning_content: None,
        })
    }
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

    let models = delegate_task_models_to_try(
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

    let models = delegate_task_models_to_try(&config, None, true);

    assert_eq!(models[0], "google_ai_studio/gemini-2.5-flash");
    assert!(models.contains(&"mistral/pixtral-large-latest".to_string()));
    assert!(models.contains(&"opencode_zen/deepseek-v4-flash-free".to_string()));
    assert!(!models.contains(&"deepseek/deepseek-chat".to_string()));
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
