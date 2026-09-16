use super::*;
use crate::config::schema::Config;
use crate::subagents::SubagentProfile;
use crate::tools::Tool;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

struct DummyTool(&'static str);

#[async_trait]
impl Tool for DummyTool {
    fn name(&self) -> &str {
        self.0
    }
    fn description(&self) -> &str {
        "dummy tool for testing"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({})
    }
    async fn call(&self, _args: &Value) -> anyhow::Result<Value> {
        Ok(serde_json::json!({}))
    }
}

#[test]
fn test_static_allowlist_for_subagent_known_and_unknown() {
    let planner_tools = static_allowlist_for_subagent("planner").expect("planner allowlist");
    assert!(planner_tools.contains(&"read_file"));
    assert!(planner_tools.contains(&"code_outline"));
    assert!(planner_tools.contains(&"parallel_research"));

    let researcher_tools = static_allowlist_for_subagent("researcher").expect("researcher allowlist");
    assert!(researcher_tools.contains(&"web_search"));
    assert!(researcher_tools.contains(&"web_fetch"));
    assert!(researcher_tools.contains(&"crawl_website"));

    let coding_tools = static_allowlist_for_subagent("coding_agent").expect("coding_agent allowlist");
    assert!(coding_tools.contains(&"exec_command"));
    assert!(coding_tools.contains(&"cargo_manager"));

    assert!(static_allowlist_for_subagent("unknown_subagent_role").is_none());
}

#[test]
fn test_all_static_subagent_allowlist_tools() {
    let all_tools = all_static_subagent_allowlist_tools();
    assert!(!all_tools.is_empty());

    // Verify list is sorted and deduplicated
    let mut sorted = all_tools.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(all_tools, sorted);

    assert!(all_tools.contains(&"read_file"));
    assert!(all_tools.contains(&"write_file"));
    assert!(all_tools.contains(&"exec_command"));
}

#[test]
fn test_filter_tools_for_subagent_prunes_remote_input() {
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(DummyTool("read_file")),
        Arc::new(DummyTool("exec_command")),
        Arc::new(DummyTool("send_remote_input")),
        Arc::new(DummyTool("unknown_custom_tool")),
    ];

    // For planner: should only get read_file; exec_command and send_remote_input are filtered out
    let filtered_planner = filter_tools_for_subagent("planner", &tools);
    assert_eq!(filtered_planner.len(), 1);
    assert_eq!(filtered_planner[0].name(), "read_file");

    // For unknown subagent: gets unrestricted list except send_remote_input which is always pruned
    let filtered_unknown = filter_tools_for_subagent("custom_unknown_agent", &tools);
    assert_eq!(filtered_unknown.len(), 3);
    assert!(!filtered_unknown.iter().any(|t| t.name() == "send_remote_input"));
}

#[test]
fn test_profile_needs_workspace() {
    assert!(profile_needs_workspace("coding_agent"));
    assert!(profile_needs_workspace("architect"));
    assert!(profile_needs_workspace("debugger"));
    assert!(profile_needs_workspace("test_engineer"));
    assert!(profile_needs_workspace("devops_agent"));

    assert!(!profile_needs_workspace("researcher"));
    assert!(!profile_needs_workspace("planner"));
    assert!(!profile_needs_workspace("reviewer"));
    assert!(!profile_needs_workspace("database_specialist"));
}

#[test]
fn test_delegate_profile_models_to_try() {
    let mut config = Config::default();
    config.agents.defaults.model = "default-gpt-model".to_string();

    // 1. Profile with explicit model and fallback overrides
    let custom_profile = SubagentProfile {
        name: "custom_coder".to_string(),
        description: "custom coding profile".to_string(),
        system_prompt: "you are a coder".to_string(),
        model: Some("claude-3-5-sonnet".to_string()),
        fallbacks: Some(vec!["gpt-4o-mini".to_string()]),
        extra: serde_json::Map::new(),
    };

    let models = delegate_profile_models_to_try(&config, &custom_profile);
    assert_eq!(models[0], "claude-3-5-sonnet");
    assert!(models.contains(&"gpt-4o-mini".to_string()));
    assert_eq!(models.last().unwrap(), "default-gpt-model");

    // 2. Profile without explicit model inherits dynamic fallbacks and ends with default model
    let standard_profile = SubagentProfile {
        name: "reviewer".to_string(),
        description: "code review profile".to_string(),
        system_prompt: "you review code".to_string(),
        model: None,
        fallbacks: None,
        extra: serde_json::Map::new(),
    };

    let models_reviewer = delegate_profile_models_to_try(&config, &standard_profile);
    assert!(!models_reviewer.is_empty());
    assert_eq!(models_reviewer.last().unwrap(), "default-gpt-model");
}
