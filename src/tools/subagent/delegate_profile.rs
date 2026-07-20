use super::delegate_task::{
    create_isolated_workspace, current_workspace_root, ensure_markdown_images,
    run_evolution_review, sync_changes_back, WorktreeGuard,
};
use super::parallel_research::get_status_from_goal;
use super::schema_retry::{evaluate_schema_retry, SchemaRetryDecision};
use super::{
    build_provider_for_model, cancellation_result_json, classify_subagent_error,
    compact_lifecycle_line, execute_subagent_run, scan_for_images, status_json,
    CancellationToken, SubagentRunStatus, DELEGATION_DEPTH,
};
use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::config::schema::Config;
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
        let current_depth = DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
        if current_depth >= 3 {
            crate::tui_println!("{}⚠️ Delegation depth limit reached ({}). Aborting nested subagent '{}'.{}", AURA_GOLD, current_depth, self.profile.name, COLOR_RESET);
            return Err(anyhow!("Delegation limit reached. Max nesting depth is 3."));
        }

        let goal = arguments.get("goal").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'goal' argument"))?;
        let context = arguments.get("context").and_then(|v| v.as_str()).unwrap_or("");
        let json_schema = arguments.get("json_schema").cloned();
        let timeout_secs = arguments.get("timeout_secs").and_then(|v| v.as_u64());

        let clean_goal = ensure_markdown_images(goal);
        let clean_context = ensure_markdown_images(context);

        let mut models_to_try = Vec::new();

        // 1. Add primary model from profile override
        if let Some(m) = &self.profile.model {
            if !m.trim().is_empty() {
                models_to_try.push(m.trim().to_string());
            }
        }

        // 2. Add fallback models from profile overrides
        if let Some(fallbacks) = &self.profile.fallbacks {
            for fallback in fallbacks {
                if !fallback.trim().is_empty() && !models_to_try.contains(&fallback.trim().to_string()) {
                    models_to_try.push(fallback.trim().to_string());
                }
            }
        }

        // 3. If no profile overrides were specified, populate with system dynamic fallbacks for this subagent role
        if self.profile.model.is_none() && self.profile.fallbacks.is_none() {
            let dynamic_fallbacks = self.config.get_dynamic_fallbacks(&self.profile.name);
            for fallback in dynamic_fallbacks {
                if !models_to_try.contains(&fallback) {
                    models_to_try.push(fallback);
                }
            }
        }

        // 4. Finally, append our main agent model as the absolute last resort fallback
        let default_model = self.config.agents.defaults.model.clone();
        if !models_to_try.contains(&default_model) {
            models_to_try.push(default_model);
        } else {
            // Move the default model to the end of the list if it is already present
            if let Some(pos) = models_to_try.iter().position(|m| m == &default_model) {
                models_to_try.remove(pos);
                models_to_try.push(default_model);
            }
        }

        let child_session_id = format!("subagent:{}:{}", self.profile.name, &uuid::Uuid::new_v4().to_string()[..8]);
        let mut subagent_prompt = format!(
            "You are a specialized subagent operating under the following profile guidelines:\n\n\
            {}\n\n\
            TASK:\n{}\n\n\
            CONTEXT:\n{}\n\n\
            When finished, provide a clear, concise summary of what you did and found.",
            self.profile.system_prompt, clean_goal, clean_context
        );

        // Automatically scan goal and context for image paths and append markdown image links
        let image_paths = scan_for_images(&clean_goal, &clean_context);
        for img in image_paths {
            subagent_prompt.push_str(&format!(" ![](file://{})", img));
        }

        let is_reviewer = self.profile.name == "reviewer";
        let is_vision = self.profile.name == "vision_agent";
        let is_vision_profile = is_vision;
        let formatted_name = format_subagent_name(&self.profile.name);
        let mut last_error = None;

        let needs_workspace = match self.profile.name.as_str() {
            "orchestrator" | "architect" | "git_ops_agent" | "dependency_manager" |
            "frontend_architect" | "media_designer" | "sop_designer" | "api_integrator" |
            "performance_tuner" | "document_compiler" | "presentation_designer" |
            "code_synthesizer" | "automation_agent" | "coding_agent" | "debugger" |
            "test_engineer" | "devops_agent" | "refactor_agent" | "openz_maintainer" |
            "mcps_manager" => true,
            _ => false, // Skip isolated workspace setup for read-only, analytical, and config-focused agents
        };

        let parent_dir = current_workspace_root();
        let mut workspace_isolation = if needs_workspace {
            "isolated_worktree".to_string()
        } else {
            "not_required".to_string()
        };
        let mut workspace_isolation_reason: Option<String> = None;
        let workspace_dir = if !needs_workspace {
            parent_dir.clone()
        } else {
            let parent_dir_clone = parent_dir.clone();
            let workspace_res = tokio::task::spawn_blocking(move || {
                create_isolated_workspace(&parent_dir_clone)
            })
            .await;

            match workspace_res {
                Ok(Ok(dir)) => {
                    crate::tui_println!("{}  ✓ Isolated workspace worktree created at {:?}{}", EMERALD_GREEN, dir, COLOR_RESET);
                    dir
                }
                Ok(Err(e)) => {
                    let reason = e.to_string();
                    workspace_isolation = "fallback_active_workspace".to_string();
                    workspace_isolation_reason = Some(reason.clone());
                    crate::tui_println!("{}⚠️  Failed to create isolated workspace ({}). Running in active workspace without isolation.{}", AURA_GOLD, reason, COLOR_RESET);
                    parent_dir.clone()
                }
                Err(e) => {
                    let reason = format!("join error: {:?}", e);
                    workspace_isolation = "fallback_active_workspace".to_string();
                    workspace_isolation_reason = Some(reason.clone());
                    crate::tui_println!("{}⚠️  Failed to create isolated workspace ({}). Running in active workspace without isolation.{}", AURA_GOLD, reason, COLOR_RESET);
                    parent_dir.clone()
                }
            }
        };

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
                        last_error = Some(e);
                        continue;
                    }
                }
            };

            let filtered_parent_tools = filter_tools_for_subagent(&self.profile.name, &self.parent_tools);
            let mut child_config = self.config.clone();
            child_config.agents.defaults.model = model_name.clone();
            child_config.agents.defaults.fallback_models.clear();

            let child_registry = ToolRegistry::new_with_context(
                child_config.clone(),
                provider.clone(),
                self.session_manager.clone(),
            );
            for tool in &filtered_parent_tools {
                child_registry.register(tool.clone());
            }

            // Only manager-style profiles can spawn generic workers. Standard subagents must finish their own task.
            let allowed_delegate = match self.profile.name.as_str() {
                "planner" | "sop_designer" | "openz_coordinator" => true,
                _ => false,
            };

            if allowed_delegate {
                child_registry.register(std::sync::Arc::new(super::delegate_task::DelegateTaskTool {
                    config: child_config.clone(),
                    parent_provider: provider.clone(),
                    session_manager: self.session_manager.clone(),
                    parent_tools: self.parent_tools.clone(),
                    cancellation_token: self.cancellation_token.clone(),
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
                let status_text = get_status_from_goal(goal);
                crate::tui_println!(
                    "{}{}{}{}",
                    AURA_SLATE, leaf_prefix, status_text, COLOR_RESET
                );
            }

            let spinner_msg = format!("{}{}{}Running...{}", AURA_SLATE, crate::agent::style::get_tree_prefix(true), AURA_SLATE, COLOR_RESET);

            let branch_id = format!("branch_{}", &uuid::Uuid::new_v4().to_string()[..8]);
            let mut has_branch = false;
            if let Ok(_) = crate::tools::graph_memory::CreateDatabaseBranchTool.call(&serde_json::json!({ "branchId": branch_id })).await {
                has_branch = true;
            }

            let mut final_prompt = subagent_prompt.clone();
            if let Some(ref schema) = json_schema {
                final_prompt.push_str(&format!(
                    "\n\nCRITICAL REQUIREMENT: Your final response MUST be a raw JSON object strictly conforming to this JSON Schema:\n{}\nDo not wrap it in markdown code blocks, do not add any conversational text. Return only the raw valid JSON.",
                    serde_json::to_string_pretty(schema).unwrap_or_default()
                ));
            }

            struct CancelOnDrop {
                token: CancellationToken,
                completed: bool,
            }
            impl Drop for CancelOnDrop {
                fn drop(&mut self) {
                    if !self.completed {
                        self.token.cancel();
                    }
                }
            }
            let mut cancel_guard = CancelOnDrop {
                token: self.cancellation_token.clone(),
                completed: false,
            };

            let mut run_res = execute_subagent_run(
                &child_agent,
                &final_prompt,
                &child_session_id,
                &self.profile.name,
                model_name,
                workspace_dir.clone(),
                current_depth,
                &self.cancellation_token,
                timeout_secs,
                self.config.agents.defaults.tool_timeout_secs,
                &spinner_msg,
            ).await;
            cancel_guard.completed = true;

            // Enforce schema validation on child agent success
            if let Some(ref schema) = json_schema {
                let mut attempts = 0;
                while run_res.is_ok() {
                    match evaluate_schema_retry(
                        run_res.as_ref().map(|res| res.content.as_str()).unwrap_or_default(),
                        schema,
                        attempts,
                        2,
                    ) {
                        Ok(SchemaRetryDecision::Accepted(clean_json)) => {
                            if let Ok(ref mut res) = run_res {
                                res.content = clean_json;
                            }
                            break;
                        }
                        Ok(SchemaRetryDecision::Retry { prompt, reason }) => {
                            attempts += 1;
                            crate::tui_println!(
                                "{}▲ [Reflection] Subagent output needs correction: {}. Retrying attempt {} of 2...{}",
                                AURA_GOLD, reason, attempts, COLOR_RESET
                            );
                            run_res = execute_subagent_run(
                                &child_agent,
                                &prompt,
                                &child_session_id,
                                &self.profile.name,
                                model_name,
                                workspace_dir.clone(),
                                current_depth,
                                &self.cancellation_token,
                                timeout_secs,
                                self.config.agents.defaults.tool_timeout_secs,
                                &spinner_msg,
                            ).await;
                        }
                        Err(e) => {
                            run_res = Err(e);
                            break;
                        }
                    }
                }
            }

            match run_res {
                Ok(run_res) => {
                    if has_branch {
                        let _ = crate::tools::graph_memory::CommitDatabaseBranchTool.call(&serde_json::json!({})).await;
                    }
                    if !crate::agent::style::is_silent() {
                        let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                        let summary = crate::agent::style::format_subagent_summary(&run_res.content);
                        let line = compact_lifecycle_line(
                            &self.profile.name,
                            &model_name,
                            &SubagentRunStatus::Completed,
                        );
                        crate::tui_println!(
                            "{}{}{}✓ {} - {}{}",
                            AURA_SLATE,
                            leaf_prefix,
                            AURA_GREEN,
                            line,
                            summary,
                            COLOR_RESET
                        );
                    }

                    if workspace_dir != parent_dir {
                        let _ = sync_changes_back(&workspace_dir, &parent_dir);
                    }

                    // Run evolution review
                    let _ = run_evolution_review(&self.parent_provider, &self.profile.name, &clean_goal, &clean_context, &run_res.content).await;

                    return Ok(serde_json::json!({
                        "status": "success",
                        "lifecycle": status_json(&SubagentRunStatus::Completed),
                        "session_id": child_session_id,
                        "model_used": model_name,
                        "workspaceIsolation": workspace_isolation,
                        "workspaceIsolationReason": workspace_isolation_reason,
                        "summary": run_res.content
                    }));
                }
                Err(e) => {
                    if has_branch {
                        let _ = crate::tools::graph_memory::RollbackDatabaseBranchTool.call(&serde_json::json!({})).await;
                    }
                    let error_text = e.to_string();
                    let lifecycle = classify_subagent_error(&error_text, &self.cancellation_token);
                    if matches!(lifecycle, SubagentRunStatus::Cancelled) {
                        if !crate::agent::style::is_silent() {
                            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                            let line = compact_lifecycle_line(&self.profile.name, &model_name, &lifecycle);
                            crate::tui_println!(
                                "{}{}{}▲ {}{}",
                                AURA_SLATE,
                                leaf_prefix,
                                AURA_GOLD,
                                line,
                                COLOR_RESET
                            );
                        }
                        let mut cancelled = cancellation_result_json(
                            "delegate_profile",
                            Some(&self.profile.name),
                            &child_session_id,
                            &model_name,
                            &error_text,
                        );
                        if let Some(obj) = cancelled.as_object_mut() {
                            obj.insert(
                                "workspaceIsolation".to_string(),
                                serde_json::Value::String(workspace_isolation.clone()),
                            );
                            obj.insert(
                                "workspaceIsolationReason".to_string(),
                                workspace_isolation_reason.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        return Ok(cancelled);
                    }
                    if !crate::agent::style::is_silent() {
                        let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                        let line = compact_lifecycle_line(&self.profile.name, &model_name, &lifecycle);
                        crate::tui_println!(
                            "{}{}{}✕ {}{}",
                            AURA_SLATE,
                            leaf_prefix,
                            AURA_ROSE,
                            line,
                            COLOR_RESET
                        );
                    }
                    last_error = Some(e);
                }
            }
        }

        let err_msg = format!("All configured models/fallbacks failed for subagent '{}'. Last error: {:?}", self.profile.name, last_error);
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
    match name {
        "vision_agent" => "Vision Agent".to_string(),
        "documentation_agent" => "Documentation Agent".to_string(),
        "self_improvement" => "Self Improvement".to_string(),
        "skill_improvement" => "Skill Improvement".to_string(),
        "openz_maintainer" => "OpenZ Maintainer".to_string(),
        "mcps_manager" => "MCPs Manager".to_string(),
        "memory_manager" => "Memory Manager".to_string(),
        "code_auditor" => "Code Auditor".to_string(),
        "test_engineer" => "Test Engineer".to_string(),
        "devops_agent" => "Devops Agent".to_string(),
        "refactor_agent" => "Refactor Agent".to_string(),
        "skill_creator" => "Skill Creator".to_string(),
        "git_ops_agent" => "Git Operations Agent".to_string(),
        "ast_searcher" => "AST Searcher".to_string(),
        "database_specialist" => "Database Specialist".to_string(),
        "browser_operator" => "Browser Operator".to_string(),
        "dependency_manager" => "Dependency Manager".to_string(),
        "frontend_architect" => "Frontend Architect".to_string(),
        "docs_lookup_agent" => "Docs Lookup Agent".to_string(),
        "document_compiler" => "Document Compiler".to_string(),
        "presentation_designer" => "Presentation Designer".to_string(),
        "code_synthesizer" => "Code Synthesizer".to_string(),
        "summarizer_agent" => "Summarizer Agent".to_string(),
        "media_designer" => "Media Designer".to_string(),
        "openz_coordinator" => "OpenZ Coordinator".to_string(),
        "sop_designer" => "SOP Designer".to_string(),
        "api_integrator" => "API Integrator".to_string(),
        "performance_tuner" => "Performance Tuner".to_string(),
        "communication_manager" => "Communication Manager".to_string(),
        "automation_agent" => "Automation Agent".to_string(),
        "coding_agent" => "Coding Agent".to_string(),
        _ => {
            let mut chars = name.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    }
}

pub fn filter_tools_for_subagent(
    subagent_name: &str,
    all_tools: &[Arc<dyn Tool>],
) -> Vec<Arc<dyn Tool>> {
    let allowed_names: Option<&[&str]> = match subagent_name {
        "planner" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "code_outline",
            "parallel_research",
            "evaluator_optimizer_loop",
        ]),
        "researcher" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "web_fetch",
            "web_search",
            "doc_reader",
            "semantic_search",
            "crawl",
            "obscura",
        ]),
        "architect" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "code_outline",
            "ast_grep",
            "db_inspector",
        ]),
        "git_ops_agent" => Some(&["read_file", "list_dir", "git_manager"]),
        "ast_searcher" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "ast_grep",
            "code_outline",
            "grep_search",
        ]),
        "database_specialist" => Some(&["read_file", "list_dir", "db_inspector"]),
        "browser_operator" => Some(&["read_file", "list_dir", "web_fetch", "crawl", "obscura"]),
        "dependency_manager" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "cargo_manager",
            "onpkg",
        ]),
        "frontend_architect" => Some(&["read_file", "write_file", "list_dir", "generate_image"]),
        "docs_lookup_agent" => Some(&[
            "read_file",
            "list_dir",
            "web_fetch",
            "web_search",
            "rust_docs",
        ]),
        "media_designer" => Some(&["read_file", "write_file", "list_dir", "generate_image"]),
        "sop_designer" => Some(&["read_file", "write_file", "list_dir"]),
        "api_integrator" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "web_fetch",
            "web_search",
            "exec_command",
        ]),
        "performance_tuner" => Some(&["read_file", "list_dir", "system_info", "exec_command"]),
        "communication_manager" => Some(&["read_file", "list_dir", "check_port"]),
        "document_compiler" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "doc_reader",
            "exec_command",
            "compile_template",
        ]),
        "presentation_designer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "exec_command",
            "generate_image",
            "compile_template",
        ]),
        "code_synthesizer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "onpkg",
            "code_outline",
            "cargo_manager",
        ]),
        "summarizer_agent" => Some(&["read_file", "write_file", "list_dir", "grep_search"]),
        "automation_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "gsd_browser",
            "obscura",
            "crawl",
            "web_fetch",
            "schedule_job",
            "list_jobs",
            "remove_job",
            "exec_command",
            "manage_mcp",
        ]),
        "coding_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "find_files",
            "code_outline",
            "ast_grep",
            "grep_search",
            "exec_command",
            "cargo_manager",
        ]),
        "reviewer" | "code_auditor" => Some(&[
            "read_file",
            "list_dir",
            "code_outline",
            "ast_grep",
            "grep_search",
        ]),
        "debugger" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "code_outline",
            "grep_search",
            "exec_command",
            "cargo_manager",
        ]),
        "test_engineer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "exec_command",
            "cargo_manager",
        ]),
        "devops_agent" => Some(&["read_file", "write_file", "list_dir", "exec_command"]),
        "refactor_agent" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "code_outline",
            "ast_grep",
            "grep_search",
        ]),
        "memory_manager" | "self_improvement" | "skill_improvement" => {
            Some(&["read_file", "write_file", "list_dir", "find_files"])
        }
        "openz_maintainer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "exec_command",
            "cargo_manager",
        ]),
        "mcps_manager" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "manage_mcp",
            "exec_command",
        ]),
        "vision_agent" => Some(&[
            "read_file",
            "list_dir",
            "find_files",
            "generate_image",
            "doc_reader",
        ]),
        "skill_creator" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "exec_command",
            "cargo_manager",
        ]),
        "documentation_agent" => Some(&["read_file", "write_file", "list_dir", "find_files"]),
        "diagram_designer" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "openmedia_diagram_generate_mermaid",
        ]),
        "video_animator" => Some(&[
            "read_file",
            "write_file",
            "list_dir",
            "openmedia_video_create",
            "openmedia_video_preview",
        ]),
        _ => None,
    };

    let mut filtered: Vec<Arc<dyn Tool>> = if let Some(allowed) = allowed_names {
        all_tools
            .iter()
            .filter(|t| allowed.contains(&t.name()))
            .cloned()
            .collect()
    } else {
        all_tools.to_vec()
    };
    filtered.retain(|t| t.name() != "send_remote_input");
    filtered
}
