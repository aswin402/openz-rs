use super::*;
use crate::agent::agent_loop::intent::{IntentDecision, KnowledgePolicy, TurnIntent};

#[test]
fn local_repo_read_pack_includes_grep_and_read_not_web() {
    let decision = IntentDecision {
        intent: TurnIntent::LocalRepoRead,
        knowledge_policy: KnowledgePolicy::UseLocalContext,
        reasons: vec!["local_repo_query"],
    };
    let scope = ToolScopeEngine::default().decide(&decision, 12);
    assert!(scope.allowed_names.contains("grep_search"));
    assert!(scope.allowed_names.contains("read_file"));
    assert!(!scope.allowed_names.contains("web_fetch"));
}

#[test]
fn external_research_pack_includes_web_and_source_tools() {
    let decision = IntentDecision {
        intent: TurnIntent::ExternalResearch,
        knowledge_policy: KnowledgePolicy::RequireLiveResearch,
        reasons: vec!["live_research_intent"],
    };
    let scope = ToolScopeEngine::default().decide(&decision, 12);
    assert!(scope.allowed_names.contains("web_fetch"));
    assert!(scope.allowed_names.contains("web_search"));
    assert!(scope.allowed_names.contains("retrieve_original"));
}

#[test]
fn direct_answer_keeps_only_escape_hatch_and_inventory() {
    let decision = IntentDecision {
        intent: TurnIntent::DirectAnswer,
        knowledge_policy: KnowledgePolicy::ModelOk,
        reasons: vec!["direct_answer_default"],
    };
    let scope = ToolScopeEngine::default().decide(&decision, 12);
    assert!(scope.allowed_names.contains("request_tool_scope"));
    assert!(scope.allowed_names.contains("openz_inventory"));
    assert!(!scope.allowed_names.contains("exec_command"));
}
