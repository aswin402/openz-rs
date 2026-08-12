use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::config::schema::Config;
use crate::orchestrator::events::NoopEventSink;
use crate::orchestrator::runtime::{build_step_prompt, StepExecutor, WorkflowRuntime};
use crate::orchestrator::spec::{CapabilityPolicy, WorkflowSpec, WorkflowStep};
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
    pub inherited_capability_policy: Option<CapabilityPolicy>,
}

impl OrchestrateWorkflowTool {
    pub fn new(
        config: Config,
        parent_provider: Arc<dyn LLMProvider>,
        session_manager: SessionManager,
        parent_tools: Vec<Arc<dyn Tool>>,
        cancellation_token: CancellationToken,
        inherited_capability_policy: Option<CapabilityPolicy>,
    ) -> Self {
        Self {
            config: Some(config),
            parent_provider: Some(parent_provider),
            session_manager: Some(session_manager),
            parent_tools,
            cancellation_token,
            inherited_capability_policy,
        }
    }
}

pub struct SubagentStepExecutor {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
    pub cancellation_token: CancellationToken,
    pub inherited_capability_policy: Option<CapabilityPolicy>,
}

pub fn tool_allowed_by_policy(name: &str, policy: &CapabilityPolicy) -> bool {
    let metadata = crate::tools::ToolMetadata::infer(name);
    tool_allowed_by_policy_with_metadata_inner(name, &metadata, policy)
}

fn tool_allowed_by_policy_with_metadata_inner(
    name: &str,
    metadata: &crate::tools::ToolMetadata,
    policy: &CapabilityPolicy,
) -> bool {
    if !policy.allowed_tools.is_empty() && !policy.allowed_tools.iter().any(|tool| tool == name) {
        return false;
    }

    if policy.denied_tools.iter().any(|tool| tool == name) {
        return false;
    }

    if policy.deny_shell && (metadata.domain == "shell" || metadata.spawns_process) {
        return false;
    }

    if policy.deny_filesystem_write && metadata.writes_disk {
        return false;
    }

    true
}

pub fn combine_capability_policies(
    inherited: Option<&CapabilityPolicy>,
    workflow: &CapabilityPolicy,
) -> CapabilityPolicy {
    let Some(inherited) = inherited else {
        return workflow.clone();
    };

    let allowed_tools = match (
        inherited.allowed_tools.is_empty(),
        workflow.allowed_tools.is_empty(),
    ) {
        (true, true) => vec![],
        (true, false) => workflow.allowed_tools.clone(),
        (false, true) => inherited.allowed_tools.clone(),
        (false, false) => inherited
            .allowed_tools
            .iter()
            .filter(|tool| workflow.allowed_tools.iter().any(|candidate| candidate == *tool))
            .cloned()
            .collect(),
    };

    let mut denied_tools = inherited.denied_tools.clone();
    for denied in &workflow.denied_tools {
        if !denied_tools.iter().any(|tool| tool == denied) {
            denied_tools.push(denied.clone());
        }
    }

    CapabilityPolicy {
        allowed_tools,
        denied_tools,
        deny_shell: inherited.deny_shell || workflow.deny_shell,
        deny_filesystem_write: inherited.deny_filesystem_write || workflow.deny_filesystem_write,
    }
}

pub fn tool_allowed_by_policy_with_metadata(
    name: &str,
    metadata: &crate::tools::ToolMetadata,
    policy: &CapabilityPolicy,
) -> bool {
    tool_allowed_by_policy_with_metadata_inner(name, metadata, policy)
}

fn filter_parent_tools_by_policy(
    tools: &[Arc<dyn Tool>],
    policy: &CapabilityPolicy,
) -> Vec<Arc<dyn Tool>> {
    tools
        .iter()
        .filter(|tool| tool_allowed_by_policy_with_metadata_inner(tool.name(), &tool.metadata(), policy))
        .cloned()
        .collect()
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

        let effective_policy = combine_capability_policies(
            self.inherited_capability_policy.as_ref(),
            &spec.capabilities,
        );
        let profile_metadata = crate::tools::subagent::subagent_tool_metadata(&profile.name);
        if !tool_allowed_by_policy_with_metadata_inner(&profile.name, &profile_metadata, &effective_policy) {
            return Err(anyhow!("workflow step agent '{}' denied by capability policy", profile.name));
        }

        let prompt = build_step_prompt(step, &spec.goal, prior_results);
        let delegate = DelegateProfileTool {
            config: self.config.clone(),
            parent_provider: self.parent_provider.clone(),
            session_manager: self.session_manager.clone(),
            profile,
            parent_tools: filter_parent_tools_by_policy(&self.parent_tools, &effective_policy),
            cancellation_token: self.cancellation_token.clone(),
            capability_policy: Some(effective_policy),
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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct NamedTool(&'static str);

    #[async_trait]
    impl Tool for NamedTool {
        fn name(&self) -> &str {
            self.0
        }

        fn description(&self) -> &str {
            "test tool"
        }

        fn parameters(&self) -> Value {
            json!({ "type": "object" })
        }

        async fn call(&self, _arguments: &Value) -> Result<Value> {
            Ok(json!({ "ok": true }))
        }
    }

    fn tool_names(tools: Vec<Arc<dyn Tool>>) -> Vec<String> {
        let mut names = tools
            .iter()
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    #[test]
    fn capability_policy_filters_shell_and_denied_tools() {
        let policy = CapabilityPolicy {
            denied_tools: vec!["web_fetch".to_string()],
            deny_shell: true,
            ..Default::default()
        };

        assert!(tool_allowed_by_policy("read_file", &policy));
        assert!(!tool_allowed_by_policy("exec_command", &policy));
        assert!(!tool_allowed_by_policy("python_sandbox", &policy));
        assert!(!tool_allowed_by_policy("wasm_sandbox", &policy));
        assert!(!tool_allowed_by_policy("web_fetch", &policy));

        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(NamedTool("read_file")),
            Arc::new(NamedTool("exec_command")),
            Arc::new(NamedTool("web_fetch")),
        ];

        assert_eq!(
            tool_names(filter_parent_tools_by_policy(&tools, &policy)),
            vec!["read_file".to_string()]
        );
    }

    #[test]
    fn combine_capability_policies_preserves_parent_denials() {
        let inherited = CapabilityPolicy {
            allowed_tools: vec!["read_file".to_string(), "coding_agent".to_string()],
            denied_tools: vec!["coding_agent".to_string()],
            deny_shell: true,
            deny_filesystem_write: false,
        };
        let workflow = CapabilityPolicy {
            allowed_tools: vec!["read_file".to_string(), "web_fetch".to_string()],
            denied_tools: vec!["web_fetch".to_string()],
            deny_shell: false,
            deny_filesystem_write: true,
        };

        let effective = combine_capability_policies(Some(&inherited), &workflow);

        assert_eq!(effective.allowed_tools, vec!["read_file".to_string()]);
        assert!(effective.denied_tools.iter().any(|tool| tool == "coding_agent"));
        assert!(effective.denied_tools.iter().any(|tool| tool == "web_fetch"));
        assert!(effective.deny_shell);
        assert!(effective.deny_filesystem_write);
    }

    #[test]
    fn subagent_metadata_policy_blocks_profile_when_shell_denied() {
        let policy = CapabilityPolicy {
            deny_shell: true,
            ..Default::default()
        };
        let metadata = crate::tools::subagent::subagent_tool_metadata("coding_agent");

        assert!(!tool_allowed_by_policy_with_metadata(
            "coding_agent",
            &metadata,
            &policy
        ));
    }

    #[test]
    fn allowlist_policy_only_allows_named_tools() {
        let policy = CapabilityPolicy {
            allowed_tools: vec!["read_file".to_string(), "grep_search".to_string()],
            denied_tools: vec!["grep_search".to_string()],
            ..Default::default()
        };

        assert!(tool_allowed_by_policy("read_file", &policy));
        assert!(!tool_allowed_by_policy("write_file", &policy));
        assert!(!tool_allowed_by_policy("db_write", &policy));
        assert!(!tool_allowed_by_policy("grep_search", &policy));

        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(NamedTool("read_file")),
            Arc::new(NamedTool("write_file")),
            Arc::new(NamedTool("grep_search")),
        ];

        assert_eq!(
            tool_names(filter_parent_tools_by_policy(&tools, &policy)),
            vec!["read_file".to_string()]
        );
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
            inherited_capability_policy: self.inherited_capability_policy.clone(),
        };
        let runtime = WorkflowRuntime::new(executor, NoopEventSink);
        let result = runtime.run(spec, &known_agents).await?;

        Ok(json!({
            "status": "success",
            "result": result
        }))
    }
}
