use super::*;
use anyhow::Result;

struct PendingProvider;

struct CountingErrorProvider {
    calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait::async_trait]
impl LLMProvider for CountingErrorProvider {
    async fn chat(
        &self,
        _system_prompt: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _settings: &GenerationSettings,
    ) -> Result<crate::providers::LLMResponse> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        anyhow::bail!("counting provider failed")
    }
}

#[async_trait::async_trait]
impl LLMProvider for PendingProvider {
    async fn chat(
        &self,
        _system_prompt: &str,
        _messages: &[Message],
        _tools: &[serde_json::Value],
        _settings: &GenerationSettings,
    ) -> Result<crate::providers::LLMResponse> {
        std::future::pending::<Result<crate::providers::LLMResponse>>().await
    }
}

#[tokio::test]
async fn turn_cancellation_context_is_visible_inside_scope() {
    let context = TurnCancellationContext {
        turn_id: "turn-context-test".to_string(),
        token: crate::tools::subagent::CancellationToken::new(),
    };
    let observed = with_turn_cancellation_context(context.clone(), async {
        current_turn_cancellation_context()
    })
    .await
    .expect("turn cancellation context should be scoped");

    assert_eq!(observed.turn_id, context.turn_id);
    observed.token.cancel();
    assert!(context.token.is_cancelled());
}

#[test]
fn runtime_subagent_model_override_survives_config_reload() {
    let mut runtime_config = Config::default();
    runtime_config.agents.defaults.model = "google/gemma-4-31b-it:free".to_string();
    let mut latest_config = Config::default();
    latest_config.agents.defaults.model = "deepseek-v4-flash-free".to_string();

    let merged = merge_latest_config_for_runtime(
        &runtime_config,
        latest_config,
        "subagent:vision_agent:test",
        None,
    );

    assert_eq!(merged.agents.defaults.model, "google/gemma-4-31b-it:free");
}
#[test]
fn fallback_models_for_turn_limits_configured_fallbacks_by_default() {
    let mut config = Config::default();
    config.agents.defaults.fallback_models = vec![
        serde_json::json!("groq/llama-3.3-70b-versatile"),
        serde_json::json!("mistral/mistral-small-latest"),
        serde_json::json!("openrouter/free"),
    ];
    std::env::remove_var("OPENZ_MAX_FALLBACK_ATTEMPTS");

    let fallbacks = fallback_models_for_turn(&config);

    assert_eq!(fallbacks.len(), 2);
    assert_eq!(fallbacks[0], "groq/llama-3.3-70b-versatile");
    assert_eq!(fallbacks[1], "mistral/mistral-small-latest");
}

fn fallback_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[tokio::test]
async fn chat_with_fallback_returns_when_cli_cancel_fires() {
    let _guard = fallback_test_lock().lock().unwrap();
    let agent = AgentLoop::new(
        Config::default(),
        Arc::new(PendingProvider),
        ToolRegistry::new(),
        SessionManager::new(std::env::temp_dir()),
    );
    let mut provider: Arc<dyn LLMProvider> = Arc::new(PendingProvider);
    let settings = GenerationSettings {
        temperature: 0.0,
        max_tokens: 1,
        reasoning_effort: None,
    };

    let result = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            crate::shutdown::trigger_cli_cancel();
        });
        agent
            .chat_with_fallback(&mut provider, "", &[], &[], &settings, "test")
            .await
    })
    .await
    .expect("chat_with_fallback should not hang after CLI cancel");

    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Cancelled by user")
    );
}

#[tokio::test]
async fn chat_with_fallback_does_not_retry_failed_primary_when_no_fallback_resolves() {
    let mut config = Config::default();
    config.agents.defaults.fallback_models.clear();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let provider_impl = CountingErrorProvider {
        calls: calls.clone(),
    };
    let agent = AgentLoop::new(
        config,
        Arc::new(CountingErrorProvider {
            calls: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }),
        ToolRegistry::new(),
        SessionManager::new(std::env::temp_dir()),
    );
    let mut provider: Arc<dyn LLMProvider> = Arc::new(provider_impl);
    let settings = GenerationSettings {
        temperature: 0.0,
        max_tokens: 1,
        reasoning_effort: None,
    };

    let result = agent
        .chat_with_fallback(&mut provider, "", &[], &[], &settings, "test")
        .await;

    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("counting provider failed")
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn chat_with_fallback_times_out_unresponsive_provider_attempt() {
    let _guard = fallback_test_lock().lock().unwrap();
    let mut config = Config::default();
    config.agents.defaults.fallback_models.clear();
    config.agents.defaults.tool_timeout_secs = 1;
    let agent = AgentLoop::new(
        config,
        Arc::new(PendingProvider),
        ToolRegistry::new(),
        SessionManager::new(std::env::temp_dir()),
    );
    let mut provider: Arc<dyn LLMProvider> = Arc::new(PendingProvider);
    let settings = GenerationSettings {
        temperature: 0.0,
        max_tokens: 1,
        reasoning_effort: None,
    };

    let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        agent
            .chat_with_fallback(&mut provider, "", &[], &[], &settings, "test")
            .await
    })
    .await
    .expect("provider attempt timeout should finish before outer timeout");

    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Provider response timed out after 1s")
    );
}

#[test]
fn test_apply_session_overrides_basic() {
    let mut config = Config::default();
    config.agents.defaults.model = "default-model".to_string();
    config.agents.defaults.temperature = 0.1;
    config.agents.defaults.tool_timeout_secs = 30;

    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "model".to_string(),
        serde_json::Value::String("overridden-model".to_string()),
    );
    metadata.insert("temperature".to_string(), serde_json::json!(0.7));

    apply_session_overrides(&mut config, &metadata);

    assert_eq!(config.agents.defaults.model, "overridden-model");
    assert_eq!(config.agents.defaults.temperature, 0.7);
    assert_eq!(config.agents.defaults.tool_timeout_secs, 30); // Unchanged
}

#[test]
fn test_apply_session_overrides_nested() {
    let mut config = Config::default();
    config.agents.defaults.model = "default-model".to_string();
    config.agents.defaults.temperature = 0.1;

    let mut metadata = serde_json::Map::new();
    let mut nested = serde_json::Map::new();
    nested.insert(
        "model".to_string(),
        serde_json::Value::String("nested-model".to_string()),
    );
    nested.insert("temperature".to_string(), serde_json::json!(0.5));
    metadata.insert(
        "config_override".to_string(),
        serde_json::Value::Object(nested),
    );

    apply_session_overrides(&mut config, &metadata);

    assert_eq!(config.agents.defaults.model, "nested-model");
    assert_eq!(config.agents.defaults.temperature, 0.5);
}
