use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::orchestrator::events::NoopEventSink;
use crate::orchestrator::runtime::{StepExecutor, WorkflowRuntime};
use crate::orchestrator::spec::{WorkflowSpec, WorkflowStep};
use crate::tools::Tool;

pub struct OrchestrateWorkflowTool;

struct SummaryOnlyExecutor;

#[async_trait]
impl StepExecutor for SummaryOnlyExecutor {
    async fn execute_step(&self, step: &WorkflowStep, _spec: &WorkflowSpec) -> Result<String> {
        Ok(format!(
            "Planned execution for agent '{}': {}",
            step.agent, step.goal
        ))
    }
}

#[async_trait]
impl Tool for OrchestrateWorkflowTool {
    fn name(&self) -> &str {
        "orchestrate_workflow"
    }

    fn description(&self) -> &str {
        "Run a typed multi-agent workflow using OpenZ subagent profiles, modes, termination policies, and review gates. This initial implementation returns summary-only planned step results and does not spawn real subagents."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "goal": { "type": "string" },
                "mode": {
                    "type": "string",
                    "enum": ["sequential", "parallel", "manager_worker", "review_loop", "selector_group", "graph"]
                },
                "agents": { "type": "array", "items": { "type": "object" } },
                "steps": { "type": "array", "items": { "type": "object" } },
                "termination": { "type": "object" },
                "review": { "type": "object" },
                "capabilities": { "type": "object" }
            },
            "required": ["goal", "mode", "steps"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let spec: WorkflowSpec = serde_json::from_value(arguments.clone())?;
        let known_agents = crate::subagents::load_profiles()
            .unwrap_or_default()
            .into_iter()
            .map(|profile| profile.name)
            .collect::<Vec<_>>();
        let runtime = WorkflowRuntime::new(SummaryOnlyExecutor, NoopEventSink);
        let result = runtime.run(spec, &known_agents).await?;

        Ok(json!({
            "status": "success",
            "result": result
        }))
    }
}
