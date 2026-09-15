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
    let declared: HashSet<&str> = spec
        .agents
        .iter()
        .map(|agent| agent.name.as_str())
        .collect();
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

    if matches!(&spec.mode, WorkflowMode::Parallel)
        && spec.steps.iter().any(|step| !step.depends_on.is_empty())
    {
        return Err(anyhow!(
            "parallel mode does not accept step dependencies; use graph mode"
        ));
    }

    if matches!(&spec.review.mode, ReviewMode::Required)
        && spec
            .review
            .reviewer
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return Err(anyhow!("required review mode needs review.reviewer"));
    }

    Ok(())
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
