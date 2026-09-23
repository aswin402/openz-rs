use super::parallel_research::get_status_from_goal;
use super::workspace::{current_workspace_root, WorktreeGuard};
use super::{
    build_provider_for_model, build_subagent_prompt, cancellation_result_json,
    ensure_markdown_images, run_subagent_attempt, status_json, CancellationToken,
    SubagentRunAttempt, SubagentRunOutcome, SubagentRunStatus, DELEGATION_DEPTH,
};
use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::config::schema::Config;
use crate::orchestrator::spec::CapabilityPolicy;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::subagents::SubagentProfile;
use crate::tools::Tool;
use crate::tools::ToolRegistry;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::Arc;

pub struct DelegateProfileTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub profile: SubagentProfile,
    pub parent_tools: Vec<Arc<dyn Tool>>,
    pub cancellation_token: CancellationToken,
    pub capability_policy: Option<CapabilityPolicy>,
}

#[async_trait::async_trait]
impl Tool for DelegateProfileTool {
    fn name(&self) -> &str {
        &self.profile.name
    }

    fn description(&self) -> &str {
        &self.profile.description
    }

    fn metadata(&self) -> crate::tools::ToolMetadata {
        super::subagent_tool_metadata(self.name())
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "goal": {
                    "type": "string",
                    "description": "The specific goal or task for this specialized subagent to accomplish."
                },
                "context": {
                    "type": "string",
                    "description": "Additional context or background details required for the task."
                },
                "json_schema": {
                    "type": "object",
                    "description": "Optional: A JSON Schema definition that this subagent's final output summary MUST strictly conform to."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Optional timeout in seconds for the subagent execution. Overrides the default tool timeout. Use higher values for complex multi-step tasks and lower values for quick reviews."
                }
            },
            "required": ["goal"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        crate::agent::style::spinner::IS_SILENT.scope(crate::agent::style::is_silent(), async {
        if let Some(policy) = &self.capability_policy {
            let metadata = super::subagent_tool_metadata(&self.profile.name);
            if !crate::tools::orchestrator::tool_allowed_by_policy_with_metadata(
                &self.profile.name,
                &metadata,
                policy,
            ) {
                return Err(anyhow!(
                    "Subagent profile '{}' is blocked by orchestrator capability policy",
                    self.profile.name
                ));
            }
        }

        let current_depth = DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
        if current_depth >= 3 {
            crate::tui_println!("{}⚠️ Delegation depth limit reached ({}). Aborting nested subagent '{}'.{}", AURA_GOLD, current_depth, self.profile.name, COLOR_RESET);
            return Err(anyhow!("Delegation limit reached. Max nesting depth is 3."));
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
        let json_schema = arguments.get("json_schema").cloned();
        let timeout_secs = super::extract_u64_arg(
            arguments,
            &["timeout_secs", "timeout"],
        );

        let clean_goal = ensure_markdown_images(&goal);
        let clean_context = ensure_markdown_images(&context);

        let models_to_try = delegate_profile_models_to_try(&self.config, &self.profile);

        let child_session_id = format!("subagent:{}:{}", self.profile.name, &uuid::Uuid::new_v4().to_string()[..8]);
        let subagent_prompt = build_subagent_prompt(
            &format!(
                "You are a specialized subagent operating under the following profile guidelines:\n\n{}",
                self.profile.system_prompt
            ),
            &clean_goal,
            &clean_context,
            json_schema.as_ref(),
        );

        let is_reviewer = self.profile.name == "reviewer";
        let is_vision = self.profile.name == "vision_agent";
        let is_vision_profile = is_vision;
        let formatted_name = format_subagent_name(&self.profile.name);
        let mut last_error = None;
        let filesystem_write_denied = super::filesystem_write_denied_by_policy(&self.capability_policy);

        let needs_workspace = !filesystem_write_denied && profile_needs_workspace(&self.profile.name);

        let parent_dir = current_workspace_root();
        let workspace =
            super::prepare_workspace(&parent_dir, filesystem_write_denied, needs_workspace).await;
        let workspace_dir = workspace.dir;
        let workspace_isolation = workspace.label;
        let workspace_isolation_reason = workspace.reason;

        let _worktree_guard = WorktreeGuard::new(parent_dir.clone(), workspace_dir.clone());

        for (idx, model_name) in models_to_try.iter().enumerate() {
            if self.cancellation_token.is_cancelled() {
                return Ok(cancellation_result_json(
                    "delegate_profile",
                    Some(&self.profile.name),
                    &child_session_id,
                    model_name,
                    "Subagent task cancelled",
                ));
            }

            // For vision_agent, skip models that don't support vision to avoid wasting fallbacks
            if is_vision_profile && !crate::providers::model_supports_vision(model_name) {
                crate::tui_println!("{}▲ Skipping non-vision model '{}' for vision task{}", AURA_GOLD, model_name, COLOR_RESET);
                continue;
            }

            if idx > 0 {
                let fallback_status = SubagentRunStatus::Fallback {
                    model: model_name.clone(),
                    attempt: idx,
                    total: models_to_try.len() - 1,
                };
                crate::tui_println!(
                    "{}▲ Primary model failed. Trying {}{}",
                    AURA_GOLD,
                    fallback_status.label(),
                    COLOR_RESET
                );
            }

            let provider = if std::env::var("OPENZ_USE_MOCK_PROVIDER").is_ok() {
                self.parent_provider.clone()
            } else {
                match build_provider_for_model(&self.config, model_name) {
                    Ok(p) => p,
                    Err(e) => {
                        let error_text = e.to_string();
                        let _ = crate::subagents::record_subagent_failure(&self.profile.name, &error_text);
                        last_error = Some(e);
                        continue;
                    }
                }
            };

            let filtered_parent_tools = super::allowlist::filter_tools_for_profile(&self.profile, &self.parent_tools);
            let mut child_config = self.config.clone();
            child_config.agents.defaults.model = model_name.clone();
            child_config.agents.defaults.fallback_models.clear();

            let child_registry = ToolRegistry::new_with_context(
                child_config.clone(),
                provider.clone(),
                self.session_manager.clone(),
            );
            child_registry.set_capability_policy(self.capability_policy.clone());
            for tool in &filtered_parent_tools {
                child_registry.register(tool.clone());
            }

            // Only manager-style profiles can spawn generic workers. Standard subagents must finish their own task.
            let allowed_delegate = super::can_spawn_nested_subagents(&self.profile.name);

            if allowed_delegate {
                child_registry.register(std::sync::Arc::new(super::delegate_task::DelegateTaskTool {
                    config: child_config.clone(),
                    parent_provider: provider.clone(),
                    session_manager: self.session_manager.clone(),
                    parent_tools: self.parent_tools.clone(),
                    cancellation_token: self.cancellation_token.clone(),
                    capability_policy: self.capability_policy.clone(),
                }));
            }

            let child_agent = AgentLoop::new(
                child_config,
                provider,
                child_registry,
                self.session_manager.clone(),
            );

            let label = if is_reviewer {
                "Reviewer".to_string()
            } else if is_vision {
                "Vision Agent".to_string()
            } else {
                formatted_name.clone()
            };

            if !crate::agent::style::is_silent() {
                let prefix = crate::agent::style::get_tree_prefix(false);
                crate::tui_println!(
                    "{}{}{}◎ {}{}{} {}{}subagent{} {}using {}{}",
                    AURA_SLATE, prefix, COLOR_RESET,
                    AURA_PURPLE, COLOR_BOLD, label, COLOR_RESET,
                    AURA_SLATE, COLOR_RESET,
                    AURA_SLATE, model_name, COLOR_RESET
                );

                let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                let status_text = get_status_from_goal(&goal);
                crate::tui_println!(
                    "{}{}{}{}",
                    AURA_SLATE, leaf_prefix, status_text, COLOR_RESET
                );
            }

            let spinner_msg = format!("{}{}{}Running...{}", AURA_SLATE, crate::agent::style::get_tree_prefix(true), AURA_SLATE, COLOR_RESET);

            let attempt = SubagentRunAttempt {
                tool_name: "delegate_profile",
                profile_name: Some(&self.profile.name),
                subagent_name: &self.profile.name,
                model_name,
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
                announce_branch: false,
            };

            match run_subagent_attempt(
                &child_agent,
                &self.parent_provider,
                &self.cancellation_token,
                attempt,
            )
            .await
            {
                SubagentRunOutcome::Success(val) => return Ok(val),
                SubagentRunOutcome::Cancelled(val) => return Ok(val),
                SubagentRunOutcome::Failed { error, .. } => {
                    last_error = Some(error);
                }
            }
        }

        let err_msg = format!("All configured models/fallbacks failed for subagent '{}'. Last error: {:?}", self.profile.name, last_error);
        let _ = crate::subagents::record_subagent_failure(&self.profile.name, &err_msg);
        let lifecycle = SubagentRunStatus::Failed {
            error: err_msg.clone(),
        };
        Ok(serde_json::json!({
            "status": "error",
            "lifecycle": status_json(&lifecycle),
            "workspaceIsolation": workspace_isolation,
            "workspaceIsolationReason": workspace_isolation_reason,
            "error": err_msg
        }))
        }).await
    }
}

pub fn format_subagent_name(name: &str) -> String {
    crate::tools::presentation_name(name)
}

pub use super::allowlist::*;

#[cfg(test)]
#[path = "delegate_profile_tests.rs"]
mod tests;

