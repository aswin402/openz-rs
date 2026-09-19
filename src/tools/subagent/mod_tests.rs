use super::*;
use crate::config::schema::Config;
use crate::session::SessionManager;
use std::sync::Arc;

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
    assert!(should_skip_evolution_capture(
        "Summarize hello",
        "\"Hello\" means greeting."
    ));
}

#[test]
fn does_not_skip_evolution_for_substantial_new_skill_output() {
    let output = "A reliable code review workflow should inspect diffs, map risk areas, run focused tests, and report file-line findings with severity.";
    assert!(!should_skip_evolution_capture(
        "Design a reusable review workflow for Rust services",
        output
    ));
}

#[test]
fn simple_step_does_not_allow_nested_delegation() {
    assert!(!step_allows_nested_delegation(
        "Summarize hello"
    ));
}

#[test]
fn explicit_specialist_step_allows_nested_delegation() {
    assert!(step_allows_nested_delegation(
        "Delegate research to a specialist and summarize findings"
    ));
}

#[test]
fn test_extract_string_arg_aliases_and_trim() {
    let args = serde_json::json!({
        "prompt": "  Analyze the codebase  ",
        "details": "extra context"
    });
    assert_eq!(
        extract_string_arg(&args, &["goal", "task", "prompt"]).unwrap(),
        "Analyze the codebase"
    );
    assert_eq!(
        extract_string_arg(&args, &["context", "details"]).unwrap(),
        "extra context"
    );
    assert!(extract_string_arg(&args, &["missing", "none"]).is_none());
}

#[test]
fn test_extract_u64_arg_coercions() {
    let args_num = serde_json::json!({ "timeout_secs": 45 });
    assert_eq!(extract_u64_arg(&args_num, &["timeout_secs"]).unwrap(), 45);

    let args_str = serde_json::json!({ "timeout": " 120 " });
    assert_eq!(extract_u64_arg(&args_str, &["timeout_secs", "timeout"]).unwrap(), 120);

    let args_float = serde_json::json!({ "max_iterations": 5.0 });
    assert_eq!(extract_u64_arg(&args_float, &["max_iterations"]).unwrap(), 5);

    let args_invalid = serde_json::json!({ "timeout": "not_a_number" });
    assert!(extract_u64_arg(&args_invalid, &["timeout"]).is_none());
}

