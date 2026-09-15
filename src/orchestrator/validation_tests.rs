use super::*;
use crate::orchestrator::spec::{AgentRef, WorkflowMode, WorkflowSpec, WorkflowStep};

fn base_spec() -> WorkflowSpec {
    WorkflowSpec {
        goal: "ship feature".to_string(),
        mode: WorkflowMode::Sequential,
        agents: vec![AgentRef {
            name: "planner".to_string(),
            model: None,
            tools: vec![],
        }],
        steps: vec![WorkflowStep {
            id: "plan".to_string(),
            agent: "planner".to_string(),
            goal: "make plan".to_string(),
            depends_on: vec![],
            expected_output: "plan".to_string(),
            max_retries: 0,
        }],
        termination: Default::default(),
        review: Default::default(),
        capabilities: Default::default(),
    }
}

#[test]
fn accepts_valid_spec() {
    let spec = base_spec();
    validate_workflow_spec(&spec, &["planner".to_string()]).expect("valid spec");
}

#[test]
fn rejects_unknown_agent() {
    let mut spec = base_spec();
    spec.steps[0].agent = "writer".to_string();
    let err = validate_workflow_spec(&spec, &["researcher".to_string()]).unwrap_err();
    assert!(err.to_string().contains("unknown agent"));
}

#[test]
fn rejects_duplicate_step_ids() {
    let mut spec = base_spec();
    spec.steps.push(spec.steps[0].clone());
    let err = validate_workflow_spec(&spec, &["planner".to_string()]).unwrap_err();
    assert!(err.to_string().contains("duplicate step id"));
}

#[test]
fn rejects_missing_dependency() {
    let mut spec = base_spec();
    spec.steps[0].depends_on.push("missing".to_string());
    let err = validate_workflow_spec(&spec, &["planner".to_string()]).unwrap_err();
    assert!(err.to_string().contains("missing dependency"));
}
