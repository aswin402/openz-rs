use crate::cli::args::{CliArgs, Command, HeadlessArgs};
use clap::Parser;

#[test]
fn test_headless_top_level_flag_parsing() {
    let args = CliArgs::try_parse_from(["openz", "-p", "inspect codebase", "--output-format", "json", "-y"])
        .expect("should parse top-level -p flag");
    assert_eq!(args.prompt.as_deref(), Some("inspect codebase"));
    assert_eq!(args.output_format.as_deref(), Some("json"));
    assert!(args.yes);
}

#[test]
fn test_headless_run_subcommand_parsing() {
    let args = CliArgs::try_parse_from([
        "openz",
        "run",
        "analyze diff",
        "--output-format",
        "text",
        "--allowed-tools",
        "read_file,grep_search",
        "--session",
        "test-session-123",
        "--model",
        "anthropic/claude-3-5-sonnet",
    ])
    .expect("should parse run subcommand");

    match args.command {
        Some(Command::Run(headless)) => {
            let _args: &HeadlessArgs = &headless;
            assert_eq!(headless.prompt.as_deref(), Some("analyze diff"));
            assert_eq!(headless.output_format, "text");
            assert_eq!(headless.allowed_tools.as_deref(), Some("read_file,grep_search"));
            assert_eq!(headless.session.as_deref(), Some("test-session-123"));
            assert_eq!(headless.model.as_deref(), Some("anthropic/claude-3-5-sonnet"));
        }
        other => panic!("expected Command::Run, got {:?}", other),
    }
}

#[test]
fn test_headless_exec_subcommand_alias() {
    let args = CliArgs::try_parse_from(["openz", "exec", "quick check"])
        .expect("should parse exec alias");
    assert!(matches!(args.command, Some(Command::Run(_))));
}

use crate::cli::headless::{
    resolve_session_key, HeadlessFormat, HeadlessRunOutput, HeadlessSecurityPolicy,
};

#[test]
fn test_headless_run_output_json_serialization() {
    let output = HeadlessRunOutput {
        status: "success".to_string(),
        content: "Operation completed successfully".to_string(),
        session_id: "cli:headless_test".to_string(),
        tools_used: vec!["read_file".to_string(), "grep_search".to_string()],
        tool_iterations: 2,
        duration_ms: 450,
        model: "anthropic/claude-3-5-sonnet".to_string(),
        provider: "anthropic".to_string(),
        error: None,
        exit_code: 0,
    };

    let json_str = serde_json::to_string_pretty(&output).expect("must serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("must parse");
    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["exit_code"], 0);
    assert_eq!(parsed["tools_used"].as_array().unwrap().len(), 2);
}

#[test]
fn test_headless_security_policy_evaluation() {
    // 1. Strict mode without --yes or allowed tools
    let policy = HeadlessSecurityPolicy::new(false, None);
    assert!(policy.is_tool_permitted("read_file", false));
    assert!(!policy.is_tool_permitted("exec_command", true));
    assert!(!policy.is_tool_permitted("write_file", true));

    // 2. Auto-approved with --yes
    let policy_yes = HeadlessSecurityPolicy::new(true, None);
    assert!(policy_yes.is_tool_permitted("read_file", false));
    assert!(policy_yes.is_tool_permitted("exec_command", true));
    assert!(policy_yes.is_tool_permitted("write_file", true));

    // 3. Allowed tools list
    let policy_allowed = HeadlessSecurityPolicy::new(false, Some("read_file,exec_command"));
    assert!(policy_allowed.is_tool_permitted("exec_command", true));
    assert!(!policy_allowed.is_tool_permitted("write_file", true));
}

#[test]
fn test_headless_session_key_resolution() {
    let ephemeral = resolve_session_key(None, false, None);
    assert!(ephemeral.starts_with("cli:headless_"));

    let custom = resolve_session_key(Some("custom-thread-key"), false, None);
    assert_eq!(custom, "custom-thread-key");
}

#[test]
fn test_headless_format_parsing() {
    assert_eq!(HeadlessFormat::parse("text"), HeadlessFormat::Text);
    assert_eq!(HeadlessFormat::parse("json"), HeadlessFormat::Json);
    assert_eq!(HeadlessFormat::parse("stream-json"), HeadlessFormat::StreamJson);
    assert_eq!(HeadlessFormat::parse("stream_json"), HeadlessFormat::StreamJson);
    assert_eq!(HeadlessFormat::parse("ndjson"), HeadlessFormat::StreamJson);
    assert_eq!(HeadlessFormat::parse("unknown"), HeadlessFormat::Text);
}

use crate::cli::headless::execute_headless_turn;
use crate::config::schema::Config;
use crate::providers::mock::{MockProvider, MockResponse};
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
use std::sync::Arc;

#[tokio::test]
async fn test_execute_headless_turn_with_mock_provider() {
    let mut config = Config::default();
    config.agents.defaults.model = "mock-model".to_string();
    config.agents.defaults.provider = "mock".to_string();

    let provider = Arc::new(MockProvider::new());
    provider.add_response("Headless execution was successful.");

    let registry = ToolRegistry::new();
    let temp_dir = std::env::temp_dir().join(format!("openz-headless-test-{}", uuid::Uuid::new_v4()));
    let session_manager = SessionManager::new(temp_dir);

    let agent_loop = crate::agent::AgentLoop::new(
        config.clone(),
        provider.clone(),
        registry,
        session_manager,
    );

    let args = HeadlessArgs {
        prompt: Some("test prompt".to_string()),
        output_format: "json".to_string(),
        yes: true,
        ..Default::default()
    };

    let output = execute_headless_turn(&agent_loop, &args, "test prompt", "cli:headless_test_run")
        .await
        .expect("turn should succeed");

    assert_eq!(output.status, "success");
    assert_eq!(output.exit_code, 0);
    assert!(output.content.contains("Headless execution was successful"));
}

use crate::cli::headless::{current_headless_policy, resolve_prompt, CURRENT_HEADLESS_POLICY};
use std::str::FromStr;

#[test]
fn test_headless_format_default_and_from_str() {
    assert_eq!(HeadlessFormat::default(), HeadlessFormat::Text);
    assert_eq!(HeadlessFormat::from_str("json").unwrap(), HeadlessFormat::Json);
    assert_eq!(
        HeadlessFormat::from_str("stream-json").unwrap(),
        HeadlessFormat::StreamJson
    );
    assert_eq!(
        HeadlessFormat::from_str("ndjson").unwrap(),
        HeadlessFormat::StreamJson
    );
    assert_eq!(
        HeadlessFormat::from_str("anything").unwrap(),
        HeadlessFormat::Text
    );
}

#[test]
fn test_headless_tool_permitted_trimming_and_casing() {
    let policy = HeadlessSecurityPolicy::new(false, Some(" read_file , EXEC_COMMAND "));
    assert!(policy.is_tool_permitted("  exec_command  ", true));
    assert!(policy.is_tool_permitted("EXEC_COMMAND", true));
    assert!(policy.is_tool_permitted("read_file", false));
    assert!(!policy.is_tool_permitted("write_file", true));
}

#[test]
fn test_headless_session_key_resolution_continue() {
    let temp_dir = std::env::temp_dir()
        .join(format!("openz-headless-session-test-{}", uuid::Uuid::new_v4()));
    let manager = SessionManager::new(temp_dir);

    // When session list is empty, resolve_session_key with continue generates new key
    let key1 = resolve_session_key(None, true, Some(&manager));
    assert!(key1.starts_with("cli:headless_"));

    // Save a dummy session to manager
    let dummy_session = crate::session::Session::new("test-continued-session");
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(manager.save(&dummy_session)).unwrap();

    // Now resolve_session_key with continue should find and return the latest session key
    let key2 = resolve_session_key(None, true, Some(&manager));
    assert_eq!(key2, "test-continued-session");
}

#[tokio::test]
async fn test_resolve_prompt_positional_and_flag() {
    let p1 = resolve_prompt(Some("positional prompt"), Some("flag prompt"))
        .await
        .unwrap();
    assert_eq!(p1, "positional prompt");

    let p2 = resolve_prompt(None, Some("flag prompt")).await.unwrap();
    assert_eq!(p2, "flag prompt");
}

#[tokio::test]
async fn test_headless_security_policy_task_local_and_ask_approval() {
    let policy = HeadlessSecurityPolicy::new(false, Some("read_file,allowed_tool"));
    let dummy_args = serde_json::json!({});

    CURRENT_HEADLESS_POLICY
        .scope(policy.clone(), async {
            let active = current_headless_policy();
            assert!(active.is_some());

            // Allowed tool auto-approves
            let approved = crate::agent::security::ask_approval(
                "cli:headless_test",
                "allowed_tool",
                &dummy_args,
            )
            .await
            .unwrap();
            assert!(approved);

            // Denied tool auto-denies without prompting
            let denied = crate::agent::security::ask_approval(
                "cli:headless_test",
                "disallowed_tool",
                &dummy_args,
            )
            .await
            .unwrap();
            assert!(!denied);
            assert_eq!(policy.last_denial().as_deref(), Some("disallowed_tool"));
        })
        .await;

    assert!(current_headless_policy().is_none());
}

#[tokio::test]
async fn test_execute_headless_turn_security_denial_returns_code_2() {
    let mut config = Config::default();
    config.agents.defaults.model = "mock-model".to_string();
    config.agents.defaults.provider = "mock".to_string();

    let provider = Arc::new(
        MockProvider::new()
            .with_response(MockResponse::tool_call(
                "exec_command",
                serde_json::json!({"command": "sudo apt update"}),
            ))
            .with_response(MockResponse::text("Tool was denied.")),
    );

    let registry = ToolRegistry::new();
    let temp_dir = std::env::temp_dir().join(format!("openz-headless-denial-{}", uuid::Uuid::new_v4()));
    let session_manager = SessionManager::new(temp_dir);

    let agent_loop = crate::agent::AgentLoop::new(
        config.clone(),
        provider.clone(),
        registry,
        session_manager,
    );

    let args = HeadlessArgs {
        prompt: Some("run dangerous command".to_string()),
        output_format: "json".to_string(),
        yes: false,
        allowed_tools: None,
        ..Default::default()
    };

    let output = execute_headless_turn(&agent_loop, &args, "run dangerous command", "cli:headless_denial_test")
        .await
        .expect("turn should finish");

    assert_eq!(output.status, "security_denied");
    assert_eq!(output.exit_code, 2);
    assert!(output.error.is_some());
    let error_msg = output.error.unwrap();
    assert!(error_msg.contains("Tool 'exec_command' requires confirmation in headless mode"));
}

#[tokio::test]
async fn test_execute_headless_turn_timeout_returns_code_1() {
    struct HangingProvider;

    #[async_trait::async_trait]
    impl crate::providers::LLMProvider for HangingProvider {
        async fn chat(
            &self,
            _system_prompt: &str,
            _messages: &[crate::session::Message],
            _tools: &[serde_json::Value],
            _settings: &crate::providers::GenerationSettings,
        ) -> anyhow::Result<crate::providers::LLMResponse> {
            std::future::pending::<anyhow::Result<crate::providers::LLMResponse>>().await
        }
    }

    let mut config = Config::default();
    config.agents.defaults.model = "mock-model".to_string();
    config.agents.defaults.provider = "mock".to_string();

    let provider = Arc::new(HangingProvider);
    let registry = ToolRegistry::new();
    let temp_dir = std::env::temp_dir().join(format!("openz-headless-timeout-{}", uuid::Uuid::new_v4()));
    let session_manager = SessionManager::new(temp_dir);

    let agent_loop = crate::agent::AgentLoop::new(
        config.clone(),
        provider,
        registry,
        session_manager,
    );

    let args = HeadlessArgs {
        prompt: Some("run hanging command".to_string()),
        output_format: "json".to_string(),
        timeout: Some(1),
        ..Default::default()
    };

    let output = execute_headless_turn(&agent_loop, &args, "run hanging command", "cli:headless_timeout_test")
        .await
        .expect("turn should return timeout output");

    assert_eq!(output.status, "error");
    assert_eq!(output.exit_code, 1);
    assert!(output.error.is_some());
    assert!(output.error.unwrap().contains("timed out after 1s"));
}

#[test]
fn test_headless_args_merge_global() {
    let mut args = HeadlessArgs {
        output_format: "text".to_string(),
        yes: false,
        ..Default::default()
    };

    // Merging with None and false preserves defaults
    args.merge_global(None, false);
    assert_eq!(args.output_format, "text");
    assert!(!args.yes);

    // Merging with Some format and true overrides
    args.merge_global(Some("json".to_string()), true);
    assert_eq!(args.output_format, "json");
    assert!(args.yes);

    // Merging again with false does not reset yes
    args.merge_global(None, false);
    assert!(args.yes);
}

#[test]
fn test_cli_args_run_with_global_flags_merging() {
    let parsed = CliArgs::try_parse_from([
        "openz",
        "-y",
        "--output-format",
        "json",
        "run",
        "run query",
    ])
    .expect("should parse openz -y --output-format json run");

    assert!(parsed.yes);
    assert_eq!(parsed.output_format.as_deref(), Some("json"));

    match parsed.command {
        Some(Command::Run(mut headless_args)) => {
            assert_eq!(headless_args.prompt.as_deref(), Some("run query"));
            headless_args.merge_global(parsed.output_format, parsed.yes);
            assert!(headless_args.yes);
            assert_eq!(headless_args.output_format, "json");
        }
        other => panic!("expected Command::Run, got {:?}", other),
    }
}

#[test]
fn test_cli_args_top_level_prompt_headless_construction() {
    let parsed = CliArgs::try_parse_from([
        "openz",
        "-p",
        "top-level query",
        "--output-format",
        "stream-json",
        "-y",
    ])
    .expect("should parse openz -p flag");

    assert!(parsed.command.is_none());
    assert_eq!(parsed.prompt.as_deref(), Some("top-level query"));

    let headless_args = HeadlessArgs {
        prompt: parsed.prompt,
        output_format: parsed.output_format.unwrap_or_else(|| "text".to_string()),
        yes: parsed.yes,
        ..Default::default()
    };

    assert_eq!(headless_args.prompt.as_deref(), Some("top-level query"));
    assert_eq!(headless_args.output_format, "stream-json");
    assert!(headless_args.yes);
}




