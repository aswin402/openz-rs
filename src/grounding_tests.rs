use super::*;

#[test]
fn grounding_policy_classifies_trivial_hello() {
    let policy = step_execution_policy(
        "Run a smoke test workflow",
        "Summarize the word hello",
        "planner",
    );

    assert_eq!(policy.grounding_class, GroundingClass::Trivial);
    assert!(!policy.allow_web);
    assert!(!policy.allow_nested_delegation);
    assert!(!policy.require_sources);
    assert!(policy.suppress_evolution);
}

#[test]
fn grounding_policy_requires_web_for_latest_version() {
    let class = classify_grounding_text("What is the latest stable Rust version today?");
    assert_eq!(class, GroundingClass::CurrentExternal);

    let policy = step_execution_policy(
        "Answer current software version question",
        "Find the latest stable Rust version today",
        "researcher",
    );
    assert!(policy.allow_web);
    assert!(policy.require_sources);
    assert!(!policy.suppress_evolution);
}

#[test]
fn grounding_policy_detects_source_specific_requests() {
    assert_eq!(
        classify_grounding_text("Read https://example.com/docs and summarize it"),
        GroundingClass::SourceSpecific
    );
    assert_eq!(
        classify_grounding_text("Compare the PDF at /tmp/report.pdf"),
        GroundingClass::SourceSpecific
    );
}

#[test]
fn manager_agent_does_not_delegate_trivial_steps_without_explicit_need() {
    let policy = step_execution_policy(
        "Run simple smoke test workflow",
        "Summarize hello",
        "manager",
    );

    assert_eq!(policy.grounding_class, GroundingClass::Trivial);
    assert!(!policy.allow_nested_delegation);
}

#[test]
fn simple_grounded_lookups_do_not_allow_nested_delegation() {
    let local_policy = step_execution_policy(
        "Answer a project question",
        "In this repo, where is orchestrate_workflow implemented?",
        "planner",
    );
    let source_policy = step_execution_policy(
        "Summarize the provided source",
        "Read https://example.com/docs and summarize it",
        "planner",
    );

    assert!(!local_policy.allow_nested_delegation);
    assert!(!source_policy.allow_nested_delegation);
}

#[test]
fn explicit_research_or_delegation_allows_nested_delegation() {
    let research_policy = step_execution_policy(
        "Research the project implementation",
        "Research the relevant files and compare sources",
        "planner",
    );
    let delegation_policy = step_execution_policy(
        "Answer a project question",
        "Delegate this repo-wide scan to a subagent",
        "planner",
    );

    assert!(research_policy.allow_nested_delegation);
    assert!(delegation_policy.allow_nested_delegation);
}

#[test]
fn grounding_policy_detects_project_questions() {
    assert_eq!(
        classify_grounding_text("In this repo, where is orchestrate_workflow implemented?"),
        GroundingClass::LocalProject
    );
}

#[test]
fn step_policy_uses_step_goal_before_workflow_goal_for_trivial_steps() {
    let policy = step_execution_policy(
        "Research the latest external workflow behavior",
        "Summarize hello",
        "planner",
    );

    assert_eq!(policy.grounding_class, GroundingClass::Trivial);
    assert!(!policy.allow_web);
    assert!(!policy.require_sources);
    assert!(!policy.allow_nested_delegation);
}

#[test]
fn review_steps_do_not_inherit_current_external_workflow_grounding() {
    let policy = step_execution_policy(
        "Research the latest external workflow behavior",
        "Review planner output for clarity",
        "reviewer",
    );

    assert_eq!(policy.grounding_class, GroundingClass::Stable);
    assert!(!policy.allow_web);
    assert!(!policy.require_sources);
    assert!(!policy.allow_nested_delegation);
}

#[test]
fn grounding_policy_detects_high_stakes_variants() {
    assert_eq!(
        classify_grounding_text("Is this medication safe?"),
        GroundingClass::HighStakes
    );
    assert_eq!(
        classify_grounding_text("Should I sign this contract?"),
        GroundingClass::HighStakes
    );
    assert_eq!(
        classify_grounding_text("Any tax impact for this investment?"),
        GroundingClass::HighStakes
    );
}

#[test]
fn reusable_evolution_guidance_can_mention_demo_or_hello() {
    assert!(!should_suppress_evolution(
        "Refactor routing",
        "coding task",
        "When adding demo or hello routing, add a focused regression test and verify policy behavior with source-backed checks."
    ));
}

#[test]
fn evolution_suppressed_for_smoke_test_outputs() {
    assert!(should_suppress_evolution(
        "Run smoke test workflow",
        "demo context",
        "Planner summary accurate. No issues."
    ));
    assert!(should_suppress_evolution(
        "Review tiny output",
        "context",
        "Cannot research with available tools."
    ));
    assert!(!should_suppress_evolution(
        "Refactor tool routing",
        "Detailed coding task",
        "When adding tool schemas, keep required fields explicit and add focused regression tests before implementation."
    ));
}
