use anyhow::{anyhow, Result};
use std::collections::HashSet;

use super::spec::{ReviewMode, WorkflowMode, WorkflowSpec};

pub fn validate_workflow_spec(spec: &WorkflowSpec, known_agents: &[String]) -> Result<()> {
    match spec.goal.trim().is_empty() {
        true => return Err(anyhow!("workflow goal is required")),
        false => {}
    }
    match spec.steps.is_empty() {
        true => return Err(anyhow!("workflow must include at least one step")),
        false => {}
    }
    match spec.termination.max_rounds == 0 || spec.termination.max_rounds > 64 {
        true => return Err(anyhow!("termination.max_rounds must be between 1 and 64")),
        false => {}
    }

    let known: HashSet<&str> = known_agents.iter().map(String::as_str).collect();
    let declared: HashSet<&str> = spec
        .agents
        .iter()
        .map(|agent| agent.name.as_str())
        .collect();
    let mut step_ids = HashSet::new();

    for step in &spec.steps {
        match step.id.trim().is_empty() {
            true => return Err(anyhow!("step id is required")),
            false => {}
        }
        match step_ids.insert(step.id.as_str()) {
            false => return Err(anyhow!("duplicate step id: {}", step.id)),
            true => {}
        }
        match known.contains(step.agent.as_str()) || declared.contains(step.agent.as_str()) {
            false => {
                return Err(anyhow!(
                    "unknown agent '{}' for step '{}'",
                    step.agent,
                    step.id
                ));
            }
            true => {}
        }
        match step.goal.trim().is_empty() {
            true => return Err(anyhow!("step '{}' goal is required", step.id)),
            false => {}
        }
    }

    for step in &spec.steps {
        for dep in &step.depends_on {
            match step_ids.contains(dep.as_str()) {
                false => {
                    return Err(anyhow!(
                        "step '{}' has missing dependency '{}'",
                        step.id,
                        dep
                    ));
                }
                true => {}
            }
        }
    }

    let invalid_parallel_deps = matches!(&spec.mode, WorkflowMode::Parallel)
        && spec.steps.iter().any(|step| !step.depends_on.is_empty());
    match invalid_parallel_deps {
        true => {
            return Err(anyhow!(
                "parallel mode does not accept step dependencies; use graph mode"
            ));
        }
        false => {}
    }

    let missing_reviewer = matches!(&spec.review.mode, ReviewMode::Required)
        && spec
            .review
            .reviewer
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty();
    match missing_reviewer {
        true => return Err(anyhow!("required review mode needs review.reviewer")),
        false => {}
    }

    Ok(())
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
