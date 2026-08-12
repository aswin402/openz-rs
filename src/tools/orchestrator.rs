use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::config::schema::Config;
use crate::orchestrator::events::NoopEventSink;
use crate::orchestrator::runtime::{build_step_prompt, StepExecutor, WorkflowRuntime};
use crate::orchestrator::spec::{WorkflowSpec, WorkflowStep};
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::subagents::SubagentProfile;
use crate::tools::subagent::{CancellationToken, DelegateProfileTool};
use crate::tools::Tool;

#[derive(Default)]
pub struct OrchestrateWorkflowTool {
    pub config: Option<Config>,
    pub parent_provider: Option<Arc<dyn LLMProvider>>,
    pub session_manager: Option<SessionManager>,
    pub parent_tools: Vec<Arc<dyn Tool>>,
    pub cancellation_token: CancellationToken,
}

impl OrchestrateWorkflowTool {
    pub fn new(
        config: Config,
        parent_provider: Arc<dyn LLMProvider>,
        session_manager: SessionManager,
        parent_tools: Vec<Arc<dyn Tool>>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            config: Some(config),
            parent_provider: Some(parent_provider),
            session_manager: Some(session_manager),
            parent_tools,
            cancellation_token,
        }
    }
}

pub struct SubagentStepExecutor {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
    pub cancellation_token: CancellationToken,
}

#[async_trait]
impl StepExecutor for SubagentStepExecutor {
    async fn execute_step(
        &self,
        step: &WorkflowStep,
        spec: &WorkflowSpec,
        prior_results: &[String],
    ) -> Result<String> {
        if self.cancellation_token.is_cancelled() {
            return Err(anyhow!("cancelled before orchestrator step '{}'", step.id));
        }

        let mut profile = find_step_profile(step)?;
        if let Some(model) = spec
            .agents
            .iter()
            .find(|agent| agent.name == step.agent)
            .and_then(|agent| agent.model.as_deref())
            .map(str::trim)
            .filter(|model| !model.is_empty())
        {
            profile.model = Some(model.to_string());
            profile.fallbacks = None;
        }

        let prompt = build_step_prompt(step, &spec.goal, prior_results);
        let delegate = DelegateProfileTool {
            config: self.config.clone(),
            parent_provider: self.parent_provider.clone(),
            session_manager: self.session_manager.clone(),
            profile,
            parent_tools: self.parent_tools.clone(),
            cancellation_token: self.cancellation_token.clone(),
        };
        let response = delegate.call(&json!({ "goal": step.goal, "context": prompt })).await?;
        response_to_step_output(step, response)
    }
}

fn find_step_profile(step: &WorkflowStep) -> Result<SubagentProfile> {
    crate::subagents::load_profiles()?
        .into_iter()
        .find(|profile| profile.name == step.agent)
        .ok_or_else(|| anyhow!("unknown subagent profile '{}'", step.agent))
}

fn response_to_step_output(step: &WorkflowStep, response: Value) -> Result<String> {
    match response.get("status").and_then(Value::as_str) {
        Some("success") => Ok(response
            .get("summary")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| response.to_string())),
        Some("cancelled") => Err(anyhow!(
            "cancelled orchestrator step '{}': {}",
            step.id,
            response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("subagent task cancelled")
        )),
        Some(status) => Err(anyhow!(
            "subagent '{}' failed with status '{}': {}",
            step.agent,
            status,
            response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or_else(|| response
                    .get("summary")
                    .and_then(Value::as_str)
                    .unwrap_or("missing error details"))
        )),
        None => Err(anyhow!(
            "subagent '{}' returned malformed response: {}",
            step.agent,
            response
        )),
    }
}

#[async_trait]
impl Tool for OrchestrateWorkflowTool {
    fn name(&self) -> &str {
        "orchestrate_workflow"
    }

    fn description(&self) -> &str {
        "Run a typed multi-agent workflow using OpenZ subagent profiles, modes, termination policies, and review gates. Each workflow step executes through the assigned subagent profile."
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
        let executor = SubagentStepExecutor {
            config: self
                .config
                .clone()
                .ok_or_else(|| anyhow!("orchestrate_workflow requires tool registry context"))?,
            parent_provider: self
                .parent_provider
                .clone()
                .ok_or_else(|| anyhow!("orchestrate_workflow requires provider context"))?,
            session_manager: self
                .session_manager
                .clone()
                .ok_or_else(|| anyhow!("orchestrate_workflow requires session context"))?,
            parent_tools: self.parent_tools.clone(),
            cancellation_token: self.cancellation_token.clone(),
        };
        let runtime = WorkflowRuntime::new(executor, NoopEventSink);
        let result = runtime.run(spec, &known_agents).await?;

        Ok(json!({
            "status": "success",
            "result": result
        }))
    }
}
