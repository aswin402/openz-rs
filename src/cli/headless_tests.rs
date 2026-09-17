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

