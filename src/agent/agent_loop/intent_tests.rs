use super::*;

#[test]
fn local_repo_question_prefers_repo_read_not_web() {
    let decision =
        classify_turn_intent("Where is orchestrate_workflow implemented in this repo?");
    assert_eq!(decision.intent, TurnIntent::LocalRepoRead);
    assert_eq!(decision.knowledge_policy, KnowledgePolicy::UseLocalContext);
    assert!(decision.reasons.contains(&"local_repo_query"));
}

#[test]
fn latest_external_question_requires_live_research() {
    let decision = classify_turn_intent("What is the latest Rust stable version today?");
    assert_eq!(decision.intent, TurnIntent::ExternalResearch);
    assert_eq!(
        decision.knowledge_policy,
        KnowledgePolicy::RequireLiveResearch
    );
    assert!(decision.reasons.contains(&"live_research_intent"));
}

#[test]
fn simple_general_task_uses_model_directly() {
    let decision = classify_turn_intent("summarize hello");
    assert_eq!(decision.intent, TurnIntent::DirectAnswer);
    assert_eq!(decision.knowledge_policy, KnowledgePolicy::ModelOk);
}

#[test]
fn build_request_prefers_local_execution_tools() {
    let decision = classify_turn_intent(
        "run the focused cargo test for workflow_spec_round_trips_from_json",
    );
    assert_eq!(decision.intent, TurnIntent::LocalExecution);
    assert_eq!(decision.knowledge_policy, KnowledgePolicy::UseLocalContext);
}
