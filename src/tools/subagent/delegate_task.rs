use super::{
    build_provider_for_model, build_subagent_prompt, run_subagent_attempt, CancellationToken,
    SubagentRunAttempt, SubagentRunOutcome, DELEGATION_DEPTH,
};
use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::config::schema::Config;
use crate::orchestrator::spec::CapabilityPolicy;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::Tool;
use crate::tools::ToolRegistry;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::Arc;

pub struct DelegateTaskTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
    pub cancellation_token: CancellationToken,
    pub capability_policy: Option<CapabilityPolicy>,
}

#[async_trait::async_trait]
impl Tool for DelegateTaskTool {
    fn name(&self) -> &str {
        "delegate_task"
    }

    fn description(&self) -> &str {
        "Delegate a specific subtask or research item to a focused subagent. The subagent runs in an isolated workspace, executes tools to accomplish the goal, and returns a summary."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "goal": {
                    "type": "string",
                    "description": "The specific goal/task for the subagent to accomplish. Be clear and detailed."
                },
                "context": {
                    "type": "string",
                    "description": "Additional context, details, files, or background information needed for the task."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override name (e.g., 'gpt-4o-mini', 'claude-3-5-haiku') for the subagent."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Optional timeout in seconds for the subagent execution. Overrides the default tool timeout. Use higher values for complex multi-step tasks (e.g., web research, code generation) and lower values for quick lookups."
                },
                "json_schema": {
                    "type": "object",
                    "description": "Optional: A JSON Schema definition that the subagent's final output summary MUST strictly conform to.",
                    "additionalProperties": true
                }
            },
            "required": ["goal"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        crate::agent::style::spinner::IS_SILENT.scope(crate::agent::style::is_silent(), async {
        let current_depth = DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
        match current_depth >= 3 {
            true => {
                crate::tui_println!("{}⚠️ Delegation depth limit reached ({}). Aborting nested delegate_task.{}", AURA_GOLD, current_depth, COLOR_RESET);
                return Err(anyhow!("Delegation limit reached. Max nesting depth is 3."));
            }
            false => {}
        }

        let goal = super::extract_string_arg(
            arguments,
            &["goal", "task", "prompt", "instruction", "description"],
        )
        .ok_or_else(|| anyhow!("Missing 'goal' argument"))?;
        let context = super::extract_string_arg(
            arguments,
            &["context", "background", "details"],
        )
        .unwrap_or_default();
        let model_override = super::extract_string_arg(
            arguments,
            &["model", "model_override"],
        );
        let json_schema = arguments.get("json_schema").cloned();
        let timeout_secs = super::extract_u64_arg(
            arguments,
            &["timeout_secs", "timeout"],
        );

        let clean_goal = ensure_markdown_images(&goal);
        let clean_context = ensure_markdown_images(&context);

        let has_images = crate::providers::parse_multimodal_content(&clean_goal).await.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }))
            || crate::providers::parse_multimodal_content(&clean_context).await.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }));

        let models_to_try = delegate_task_models_to_try(&self.config, model_override.as_deref(), has_images);
        let mut selected_model = self.config.agents.defaults.model.clone();
        let mut selected_fallback_models: Vec<String> = Vec::new();
        let provider = match std::env::var("OPENZ_USE_MOCK_PROVIDER").is_ok() {
            true => self.parent_provider.clone(),
            false => {
                let mut selected_provider = None;
                for (idx, model) in models_to_try.iter().enumerate() {
                    match build_provider_for_model(&self.config, model) {
                        Ok(provider) => {
                            selected_model = model.clone();
                            selected_fallback_models = models_to_try[idx + 1..].to_vec();
                            match has_images && crate::providers::model_supports_vision(model) {
                                true => {
                                    crate::tui_println!("{}  ✓ Auto-routed vision task to subagent model '{}'{}", EMERALD_GREEN, model, COLOR_RESET);
                                }
                                false => {}
                            }
                            selected_provider = Some(provider);
                            break;
                        }
                        Err(e) => {
                            crate::tui_println!("{}⚠️ Failed to configure subagent model '{}' ({}). Trying next fallback.{}", AURA_GOLD, model, e, COLOR_RESET);
                        }
                    }
                }
                selected_provider.unwrap_or_else(|| {
                    selected_fallback_models.clear();
                    self.parent_provider.clone()
                })
            }
        };

        let mut child_config = self.config.clone();
        child_config.agents.defaults.model = selected_model.clone();
        child_config.agents.defaults.fallback_models = selected_fallback_models
            .iter()
            .map(|model| serde_json::json!(model))
            .collect();
        let child_registry = ToolRegistry::new_with_context(
            child_config.clone(),
            provider.clone(),
            self.session_manager.clone(),
        );
        child_registry.set_capability_policy(self.capability_policy.clone());
        for tool in &self.parent_tools {
            let name = tool.name();
            match matches!(
                name,
                "delegate_task" | "parallel_research" | "evaluator_optimizer_loop"
            ) {
                true => {}
                false => {
                    child_registry.register(tool.clone());
                }
            }
        }
        child_registry.seed_subagent_working_set(
            &clean_goal,
            &clean_context,
            &super::active_parent_sticky_tools(),
        );

        let child_session_id = format!("subagent:{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let child_agent = AgentLoop::new(
            child_config,
            provider,
            child_registry,
            self.session_manager.clone(),
        );

        let subagent_prompt = build_subagent_prompt(
            "You are a focused subagent. Complete the following task using the tools available.",
            &clean_goal,
            &clean_context,
            json_schema.as_ref(),
        );

        let filesystem_write_denied = super::filesystem_write_denied_by_policy(&self.capability_policy);

        let parent_dir = current_workspace_root();
        let workspace = super::prepare_workspace(&parent_dir, filesystem_write_denied, true).await;
        let workspace_dir = workspace.dir;
        let workspace_isolation = workspace.label;
        let workspace_isolation_reason = workspace.reason;

        let _worktree_guard = WorktreeGuard::new(parent_dir.clone(), workspace_dir.clone());

        match !crate::agent::style::is_silent() {
            true => {
                let prefix = crate::agent::style::get_tree_prefix(false);
                crate::tui_println!(
                    "{}{}{}• {}{}Subagent{} {}using {}{}",
                    AURA_SLATE, prefix, COLOR_RESET,
                    RED_ORANGE, COLOR_BOLD, COLOR_RESET,
                    AURA_SLATE, selected_model, COLOR_RESET
                );
            }
            false => {}
        }
        let spinner_msg = crate::agent::style::get_tree_spinner_msg("subagent", "");

        let attempt = SubagentRunAttempt {
            tool_name: "delegate_task",
            profile_name: None,
            subagent_name: "delegate_task",
            model_name: &selected_model,
            child_session_id: &child_session_id,
            prompt: &subagent_prompt,
            clean_goal: &clean_goal,
            clean_context: &clean_context,
            current_depth,
            timeout_secs,
            default_timeout_secs: self.config.agents.defaults.tool_timeout_secs,
            spinner_msg: &spinner_msg,
            json_schema: json_schema.as_ref(),
            parent_dir: &parent_dir,
            workspace_dir: workspace_dir.clone(),
            filesystem_write_denied,
            workspace_isolation: &workspace_isolation,
            workspace_isolation_reason: &workspace_isolation_reason,
            announce_branch: true,
        };

        match run_subagent_attempt(
            &child_agent,
            &self.parent_provider,
            &self.cancellation_token,
            attempt,
        )
        .await
        {
            SubagentRunOutcome::Success(val) => Ok(val),
            SubagentRunOutcome::Cancelled(val) => Ok(val),
            SubagentRunOutcome::Failed { response_json, .. } => Ok(response_json),
        }
        }).await
    }
}

pub(crate) fn delegate_task_models_to_try(
    config: &Config,
    model_override: Option<&str>,
    has_images: bool,
) -> Vec<String> {
    fn add_candidate(models: &mut Vec<String>, model: &str) {
        let trimmed = model.trim();
        match !trimmed.is_empty()
            && !models
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(trimmed))
        {
            true => models.push(trimmed.to_string()),
            false => {}
        }
    }

    let default_model = config.agents.defaults.model.trim();
    let mut models = Vec::new();

    match (
        model_override
            .map(str::trim)
            .filter(|model| !model.is_empty()),
        has_images && !crate::providers::model_supports_vision(default_model),
    ) {
        (Some(model), _) => add_candidate(&mut models, model),
        (None, true) => {
            for fallback in config
                .get_dynamic_fallbacks("vision_agent")
                .into_iter()
                .filter(|model| crate::providers::model_supports_vision(model))
            {
                add_candidate(&mut models, &fallback);
            }
        }
        (None, false) => add_candidate(&mut models, default_model),
    }

    for model in config
        .agents
        .defaults
        .fallback_models
        .iter()
        .filter_map(|v| v.as_str().map(str::trim).filter(|m| !m.is_empty()))
        .filter(|model| !has_images || crate::providers::model_supports_vision(model))
    {
        add_candidate(&mut models, model);
    }

    add_candidate(&mut models, default_model);
    super::limit_subagent_models_to_try(&mut models);

    match !default_model.is_empty()
        && !models
            .iter()
            .any(|model| model.eq_ignore_ascii_case(default_model))
    {
        true => match (
            models.len() >= super::max_subagent_model_attempts(),
            models.last_mut(),
        ) {
            (true, Some(last)) => *last = default_model.to_string(),
            (true, None) => {}
            (false, _) => models.push(default_model.to_string()),
        },
        false => {}
    }

    models
}

pub use super::ensure_markdown_images;

pub use super::workspace::*;

#[cfg(test)]
#[path = "delegate_task_tests.rs"]
mod tests;
