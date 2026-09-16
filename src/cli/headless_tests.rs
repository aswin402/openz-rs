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
