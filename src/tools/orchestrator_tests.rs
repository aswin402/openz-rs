use super::*;
use async_trait::async_trait;

struct NamedTool(&'static str);

#[async_trait]
impl Tool for NamedTool {
    fn name(&self) -> &str {
        self.0
    }

    fn description(&self) -> &str {
        "test tool"
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object" })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        Ok(json!({ "ok": true }))
    }
}

fn tool_names(tools: Vec<Arc<dyn Tool>>) -> Vec<String> {
    let mut names = tools
        .iter()
        .map(|tool| tool.name().to_string())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn workflow_parse_error_explains_required_shape() {
    let err = OrchestrateWorkflowTool::parse_workflow_spec(&json!({
        "mode": "sequential",
        "steps": [{ "id": "plan", "agent": "planner" }]
    }))
    .unwrap_err()
    .to_string();

    assert!(err.contains("invalid orchestrate_workflow spec"));
    assert!(err.contains("top-level goal, mode, steps"));
    assert!(err.contains("prompt"));
}

#[test]
fn capability_policy_filters_shell_and_denied_tools() {
    let policy = CapabilityPolicy {
        denied_tools: vec!["web_fetch".to_string()],
        deny_shell: true,
        ..Default::default()
    };

    assert!(tool_allowed_by_policy("read_file", &policy));
    assert!(!tool_allowed_by_policy("exec_command", &policy));
    assert!(!tool_allowed_by_policy("python_sandbox", &policy));
    assert!(!tool_allowed_by_policy("wasm_sandbox", &policy));
    assert!(!tool_allowed_by_policy("web_fetch", &policy));

    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(NamedTool("read_file")),
        Arc::new(NamedTool("exec_command")),
        Arc::new(NamedTool("web_fetch")),
    ];

    assert_eq!(
        tool_names(filter_parent_tools_by_policy(&tools, &policy)),
        vec!["read_file".to_string()]
    );
}

#[test]
fn combine_capability_policies_preserves_parent_denials() {
    let inherited = CapabilityPolicy {
        allowed_tools: vec!["read_file".to_string(), "coding_agent".to_string()],
        denied_tools: vec!["coding_agent".to_string()],
        deny_shell: true,
        deny_filesystem_write: false,
        deny_network: false,
    };
    let workflow = CapabilityPolicy {
        allowed_tools: vec!["read_file".to_string(), "web_fetch".to_string()],
        denied_tools: vec!["web_fetch".to_string()],
        deny_shell: false,
        deny_filesystem_write: true,
        deny_network: true,
    };

    let effective = combine_capability_policies(Some(&inherited), &workflow);

    assert_eq!(effective.allowed_tools, vec!["read_file".to_string()]);
    assert!(effective
        .denied_tools
        .iter()
        .any(|tool| tool == "coding_agent"));
    assert!(effective
        .denied_tools
        .iter()
        .any(|tool| tool == "web_fetch"));
    assert!(effective.deny_shell);
    assert!(effective.deny_filesystem_write);
    assert!(effective.deny_network);
}

#[test]
fn capability_policy_can_block_network_tools() {
    let policy = CapabilityPolicy {
        deny_network: true,
        ..Default::default()
    };

    assert!(tool_allowed_by_policy("read_file", &policy));
    assert!(!tool_allowed_by_policy("web_fetch", &policy));
    assert!(!tool_allowed_by_policy("web_search", &policy));
    assert!(!tool_allowed_by_policy("searchxyz_read_url", &policy));
}

#[test]
fn step_grounding_policy_blocks_network_when_web_not_allowed() {
    let policy = CapabilityPolicy::default();
    let step_policy = crate::grounding::step_execution_policy(
        "Run smoke test workflow",
        "Summarize hello",
        "planner",
    );
    let effective = apply_step_grounding_policy(policy, &step_policy);

    assert!(effective.deny_network);
    assert!(!tool_allowed_by_policy("web_fetch", &effective));
}

#[test]
fn subagent_metadata_policy_allows_profile_when_shell_denied() {
    let policy = CapabilityPolicy {
        deny_shell: true,
        ..Default::default()
    };
    let metadata = crate::tools::subagent::subagent_tool_metadata("coding_agent");

    assert!(tool_allowed_by_policy_with_metadata(
        "coding_agent",
        &metadata,
        &policy
    ));
}

#[test]
fn allowlist_policy_only_allows_named_tools() {
    let policy = CapabilityPolicy {
        allowed_tools: vec!["read_file".to_string(), "grep_search".to_string()],
        denied_tools: vec!["grep_search".to_string()],
        ..Default::default()
    };

    assert!(tool_allowed_by_policy("read_file", &policy));
    assert!(!tool_allowed_by_policy("write_file", &policy));
    assert!(!tool_allowed_by_policy("db_write", &policy));
    assert!(!tool_allowed_by_policy("grep_search", &policy));

    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(NamedTool("read_file")),
        Arc::new(NamedTool("write_file")),
        Arc::new(NamedTool("grep_search")),
    ];

    assert_eq!(
        tool_names(filter_parent_tools_by_policy(&tools, &policy)),
        vec!["read_file".to_string()]
    );
}
