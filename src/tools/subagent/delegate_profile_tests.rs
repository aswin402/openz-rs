use super::*;
use crate::config::schema::Config;
use crate::session::SessionManager;
use anyhow::Result;
use std::sync::Arc;

static TEST_CANCEL_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

async fn cancel_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_CANCEL_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
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
