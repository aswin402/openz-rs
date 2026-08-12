use anyhow::{anyhow, Result};
use std::collections::HashSet;

use super::spec::{ReviewMode, WorkflowMode, WorkflowSpec};

pub fn validate_workflow_spec(spec: &WorkflowSpec, known_agents: &[String]) -> Result<()> {
    if spec.goal.trim().is_empty() {
        return Err(anyhow!("workflow goal is required"));
    }
    if spec.steps.is_empty() {
        return Err(anyhow!("workflow must include at least one step"));
    }
    if spec.termination.max_rounds == 0 || spec.termination.max_rounds > 64 {
        return Err(anyhow!("termination.max_rounds must be between 1 and 64"));
    }

    let known: HashSet<&str> = known_agents.iter().map(String::as_str).collect();
    let declared: HashSet<&str> = spec.agents.iter().map(|agent| agent.name.as_str()).collect();
    let mut step_ids = HashSet::new();

    for step in &spec.steps {
        if step.id.trim().is_empty() {
            return Err(anyhow!("step id is required"));
        }
        if !step_ids.insert(step.id.as_str()) {
            return Err(anyhow!("duplicate step id: {}", step.id));
        }
        if !known.contains(step.agent.as_str()) && !declared.contains(step.agent.as_str()) {
            return Err(anyhow!(
                "unknown agent '{}' for step '{}'",
                step.agent,
                step.id
            ));
        }
        if step.goal.trim().is_empty() {
            return Err(anyhow!("step '{}' goal is required", step.id));
        }
    }

    for step in &spec.steps {
        for dep in &step.depends_on {
            if !step_ids.contains(dep.as_str()) {
                return Err(anyhow!(
                    "step '{}' has missing dependency '{}'",
                    step.id,
                    dep
                ));
            }
        }
    }

    if matches!(spec.mode, WorkflowMode::Parallel)
        && spec.steps.iter().any(|step| !step.depends_on.is_empty())
    {
        return Err(anyhow!(
            "parallel mode does not accept step dependencies; use graph mode"
        ));
    }

    if matches!(spec.review.mode, ReviewMode::Required)
        && spec.review.reviewer.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err(anyhow!("required review mode needs review.reviewer"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::spec::{AgentRef, WorkflowMode, WorkflowSpec, WorkflowStep};

    fn base_spec() -> WorkflowSpec {
        WorkflowSpec {
            goal: "ship feature".to_string(),
            mode: WorkflowMode::Sequential,
            agents: vec![AgentRef { name: "planner".to_string(), model: None, tools: vec![] }],
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
}
