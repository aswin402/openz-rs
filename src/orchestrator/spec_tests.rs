use super::*;

#[test]
fn workflow_spec_round_trips_from_json() {
    let raw = serde_json::json!({
        "goal": "Research and implement feature",
        "mode": "sequential",
        "agents": [
            { "name": "researcher" },
            { "name": "reviewer", "model": "openrouter/qwen/qwen2.5-vl-72b-instruct:free" }
        ],
        "steps": [
            {
                "id": "research",
                "agent": "researcher",
                "goal": "Collect references",
                "depends_on": [],
                "expected_output": "bullet summary with sources"
            },
            {
                "id": "review",
                "agent": "reviewer",
                "goal": "Check result quality",
                "depends_on": ["research"],
                "expected_output": "approve or revision notes"
            }
        ],
        "termination": { "max_rounds": 4, "success_keyword": "APPROVE" },
        "review": { "mode": "required", "reviewer": "reviewer" },
        "capabilities": { "allowed_tools": ["searchxyz_read_url"], "deny_shell": true, "deny_network": true }
    });

    let spec: WorkflowSpec = serde_json::from_value(raw).expect("valid workflow spec");
    assert_eq!(spec.goal, "Research and implement feature");
    assert!(matches!(spec.mode, WorkflowMode::Sequential));
    assert_eq!(spec.agents.len(), 2);
    assert_eq!(spec.steps[1].depends_on, vec!["research"]);
    assert_eq!(spec.termination.max_rounds, 4);
    assert_eq!(spec.capabilities.allowed_tools, vec!["searchxyz_read_url"]);
    assert!(spec.capabilities.deny_network);
}

#[test]
fn workflow_spec_accepts_model_friendly_aliases() {
    let raw = serde_json::json!({
        "goal": "Run a small workflow",
        "mode": "sequential",
        "agents": [
            { "agent": "planner" }
        ],
        "steps": [
            {
                "id": "plan",
                "agent": "planner",
                "prompt": "Summarize hello"
            }
        ]
    });

    let spec: WorkflowSpec = serde_json::from_value(raw).expect("aliases are accepted");
    assert_eq!(spec.agents[0].name, "planner");
    assert_eq!(spec.steps[0].goal, "Summarize hello");
}
