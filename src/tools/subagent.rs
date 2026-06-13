use crate::tools::Tool;
use crate::tools::ToolRegistry;
use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::providers::openai::OpenAIProvider;
use crate::providers::anthropic::AnthropicProvider;
use crate::session::SessionManager;
use crate::subagents::SubagentProfile;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use serde_json::Value;

tokio::task_local! {
    pub static DELEGATION_DEPTH: usize;
}

pub struct DelegateTaskTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
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
                "json_schema": {
                    "type": "object",
                    "description": "Optional: A JSON Schema definition that the subagent's final output summary MUST strictly conform to."
                }
            },
            "required": ["goal"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let current_depth = DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
        if current_depth >= 3 {
            crate::tui_println!("{}⚠️ Delegation depth limit reached ({}). Aborting nested delegate_task.{}", AURA_GOLD, current_depth, COLOR_RESET);
            return Err(anyhow!("Delegation limit reached. Max nesting depth is 3."));
        }

        let goal = arguments.get("goal").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'goal' argument"))?;
        let context = arguments.get("context").and_then(|v| v.as_str()).unwrap_or("");
        let model_override = arguments.get("model").and_then(|v| v.as_str());
        let json_schema = arguments.get("json_schema").cloned();

        let provider = if let Some(m) = model_override {
            match build_provider_for_model(&self.config, m) {
                Ok(p) => p,
                Err(e) => {
                    crate::tui_println!("{}⚠️ Failed to configure subagent model '{}' ({}). Falling back to parent model.{}", AURA_GOLD, m, e, COLOR_RESET);
                    self.parent_provider.clone()
                }
            }
        } else {
            self.parent_provider.clone()
        };

        let mut child_registry = ToolRegistry::new_with_context(
            self.config.clone(),
            provider.clone(),
            self.session_manager.clone(),
        );
        for tool in &self.parent_tools {
            child_registry.register(tool.clone());
        }
        child_registry.register(std::sync::Arc::new(DelegateTaskTool {
            config: self.config.clone(),
            parent_provider: provider.clone(),
            session_manager: self.session_manager.clone(),
            parent_tools: self.parent_tools.clone(),
        }));

        let child_session_id = format!("subagent:{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let child_agent = AgentLoop::new(
            self.config.clone(),
            provider,
            child_registry,
            self.session_manager.clone(),
        );

        let mut subagent_prompt = format!(
            "You are a focused subagent. Complete the following task using the tools available.\n\n\
            TASK:\n{}\n\n\
            CONTEXT:\n{}\n\n\
            When finished, provide a clear, concise summary of what you did and found.",
            goal, context
        );

        if let Some(ref schema) = json_schema {
            subagent_prompt.push_str(&format!(
                "\n\nCRITICAL REQUIREMENT: Your final response MUST be a raw JSON object strictly conforming to this JSON Schema:\n{}\nDo not wrap it in markdown code blocks, do not add any conversational text. Return only the raw valid JSON.",
                serde_json::to_string_pretty(schema).unwrap_or_default()
            ));
        }

        let branch_id = format!("branch_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let mut has_memory_mcp = false;
        if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
            match client.call_tool("create_database_branch", &serde_json::json!({ "branchId": branch_id })).await {
                Ok(_) => {
                    crate::tui_println!("{}  ✓ Isolated simulation space branch '{}' created{}", EMERALD_GREEN, branch_id, COLOR_RESET);
                    has_memory_mcp = true;
                }
                Err(e) => {
                    tracing::warn!("Failed to create database branch: {:?}", e);
                }
            }
        }

        let parent_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let workspace_dir = match create_isolated_workspace(&parent_dir) {
            Ok(dir) => {
                crate::tui_println!("{}  ✓ Isolated workspace worktree created at {:?}{}", EMERALD_GREEN, dir, COLOR_RESET);
                dir
            }
            Err(e) => {
                crate::tui_println!("{}⚠️ Failed to create isolated workspace ({:?}). Running in active workspace.{}", AURA_GOLD, e, COLOR_RESET);
                parent_dir.clone()
            }
        };

        let _worktree_guard = WorktreeGuard::new(parent_dir.clone(), workspace_dir.clone());

        crate::tui_println!("{}◎ Subagent{}", AURA_PURPLE, COLOR_RESET);
        let spinner_msg = format!("{}  Running...{}", AURA_SLATE, COLOR_RESET);

        let mut run_res = {
            let p_ref = &subagent_prompt;
            let c_ref = &child_session_id;
            let child_agent_ref = &child_agent;
            let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir.clone(), async {
                DELEGATION_DEPTH.scope(current_depth + 1, async {
                    child_agent_ref.run(p_ref, c_ref).await
                }).await
            });
            let run_res_timeout = tokio::time::timeout(std::time::Duration::from_secs(300), run_res_fut);
            match with_spinner(&spinner_msg, run_res_timeout).await {
                Ok(res) => res,
                Err(_) => Err(anyhow!("Subagent execution timed out after 5 minutes")),
            }
        };

        if let Some(ref schema) = json_schema {
            let mut attempts = 0;
            loop {
                if let Ok(ref mut res) = run_res {
                    let text_output = res.content.trim();
                    let clean_json_str = if text_output.starts_with("```json") {
                        text_output.strip_prefix("```json").unwrap().strip_suffix("```").unwrap_or(text_output).trim()
                    } else if text_output.starts_with("```") {
                        text_output.strip_prefix("```").unwrap().strip_suffix("```").unwrap_or(text_output).trim()
                    } else {
                        text_output
                    };

                    let parsed_val: Result<Value, _> = serde_json::from_str(clean_json_str);
                    match parsed_val {
                        Ok(val) => {
                            match validate_schema(&val, schema) {
                                Ok(_) => {
                                    res.content = clean_json_str.to_string();
                                    break;
                                }
                                Err(e) => {
                                    if attempts >= 2 {
                                        run_res = Err(anyhow!("Subagent output failed schema validation: {}", e));
                                        break;
                                    }
                                    attempts += 1;
                                    crate::tui_println!(
                                        "{}▲ [Reflection] Subagent JSON schema validation failed: {}. Retrying attempt {} of 2...{}",
                                        AURA_GOLD, e, attempts, COLOR_RESET
                                    );
                                    let retry_prompt = format!(
                                        "Your previous response did not conform to the JSON Schema. Validation Error: {}\n\n\
                                        Please correct your response. Return ONLY the raw valid JSON matching the schema.",
                                        e
                                    );
                                    let p_ref = &retry_prompt;
                                    let c_ref = &child_session_id;
                                    let child_agent_ref = &child_agent;
                                    let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir.clone(), async {
                                        DELEGATION_DEPTH.scope(current_depth + 1, async {
                                            child_agent_ref.run(p_ref, c_ref).await
                                        }).await
                                    });
                                    let run_res_timeout = tokio::time::timeout(std::time::Duration::from_secs(300), run_res_fut);
                                    run_res = match with_spinner(&spinner_msg, run_res_timeout).await {
                                        Ok(r) => r,
                                        Err(_) => Err(anyhow!("Subagent execution timed out after 5 minutes")),
                                    };
                                }
                            }
                        }
                        Err(e) => {
                            if attempts >= 2 {
                                run_res = Err(anyhow!("Subagent output failed to parse as JSON: {}. Parse Error: {}", e, text_output));
                                break;
                            }
                            attempts += 1;
                            crate::tui_println!(
                                "{}▲ [Reflection] Subagent output is not valid JSON: {}. Retrying attempt {} of 2...{}",
                                AURA_GOLD, e, attempts, COLOR_RESET
                            );
                            let retry_prompt = format!(
                                "Your previous response was not valid JSON. Parse Error: {}\n\n\
                                Please correct your response. Return ONLY the raw valid JSON matching the schema.",
                                e
                            );
                            let p_ref = &retry_prompt;
                            let c_ref = &child_session_id;
                            let child_agent_ref = &child_agent;
                            let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir.clone(), async {
                                DELEGATION_DEPTH.scope(current_depth + 1, async {
                                    child_agent_ref.run(p_ref, c_ref).await
                                }).await
                            });
                            let run_res_timeout = tokio::time::timeout(std::time::Duration::from_secs(300), run_res_fut);
                            run_res = match with_spinner(&spinner_msg, run_res_timeout).await {
                                Ok(r) => r,
                                Err(_) => Err(anyhow!("Subagent execution timed out after 5 minutes")),
                            };
                        }
                    }
                } else {
                    break;
                }
            }
        }

        if has_memory_mcp {
            if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                if run_res.is_ok() {
                    match client.call_tool("commit_database_branch", &serde_json::json!({})).await {
                        Ok(_) => crate::tui_println!("{}  ✓ Committed simulation space branch '{}'{}", EMERALD_GREEN, branch_id, COLOR_RESET),
                        Err(e) => tracing::warn!("Failed to commit database branch: {:?}", e),
                    }
                } else {
                    match client.call_tool("rollback_database_branch", &serde_json::json!({})).await {
                        Ok(_) => crate::tui_println!("{}  ✓ Rolled back simulation space branch '{}'{}", AURA_GOLD, branch_id, COLOR_RESET),
                        Err(e) => tracing::warn!("Failed to rollback database branch: {:?}", e),
                    }
                }
            }
        }

        if run_res.is_ok() && workspace_dir != parent_dir {
            if let Err(e) = sync_changes_back(&workspace_dir, &parent_dir) {
                crate::tui_println!("{}⚠️ Failed to sync changes back to active workspace: {}{}", AURA_GOLD, e, COLOR_RESET);
            } else {
                crate::tui_println!("{}  ✓ Synchronized changes back to active workspace{}", EMERALD_GREEN, COLOR_RESET);
            }
        }

        match run_res {
            Ok(res) => {
                crate::tui_println!("{}  ✓ Complete{}", EMERALD_GREEN, COLOR_RESET);
                
                // Run evolution review
                let _ = run_evolution_review(&self.parent_provider, "subagent", goal, context, &res.content).await;

                Ok(serde_json::json!({
                    "status": "success",
                    "session_id": child_session_id,
                    "summary": res.content
                }))
            }
            Err(e) => {
                crate::tui_println!("{}  ✕ Subagent execution failed: {}{}", ERROR_RED, e, COLOR_RESET);
                Ok(serde_json::json!({
                    "status": "error",
                    "error": format!("Subagent execution failed: {:?}", e)
                }))
            }
        }
    }
}

fn ensure_markdown_images(text: &str) -> String {
    let re = match regex::Regex::new(r"(?i)(file://[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif)|https?://[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif)|/[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif))") {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };
    
    let mut result = text.to_string();
    let mut matches: Vec<_> = re.find_iter(text).collect();
    matches.reverse();
    
    for mat in matches {
        let start = mat.start();
        let end = mat.end();
        let matched_str = mat.as_str();
        
        let mut already_formatted = false;
        if start > 0 {
            let before = &text[..start];
            if before.ends_with('(') || before.ends_with("](") {
                already_formatted = true;
            }
        }
        
        if !already_formatted {
            let replacement = format!("![]({})", matched_str);
            result.replace_range(start..end, &replacement);
        }
    }
    result
}

pub struct DelegateProfileTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub profile: SubagentProfile,
    pub parent_tools: Vec<Arc<dyn Tool>>,
}

#[async_trait::async_trait]
impl Tool for DelegateProfileTool {
    fn name(&self) -> &str {
        &self.profile.name
    }

    fn description(&self) -> &str {
        &self.profile.description
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
                }
            },
            "required": ["goal"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let current_depth = DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
        if current_depth >= 3 {
            crate::tui_println!("{}⚠️ Delegation depth limit reached ({}). Aborting nested subagent '{}'.{}", AURA_GOLD, current_depth, self.profile.name, COLOR_RESET);
            return Err(anyhow!("Delegation limit reached. Max nesting depth is 3."));
        }

        let goal = arguments.get("goal").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'goal' argument"))?;
        let context = arguments.get("context").and_then(|v| v.as_str()).unwrap_or("");
        let json_schema = arguments.get("json_schema").cloned();

        let clean_goal = ensure_markdown_images(goal);
        let clean_context = ensure_markdown_images(context);

        let mut models_to_try = Vec::new();
        if let Some(m) = &self.profile.model {
            models_to_try.push(m.clone());
        } else {
            models_to_try.push(self.config.agents.defaults.model.clone());
        }

        if let Some(fallbacks) = &self.profile.fallbacks {
            for fallback in fallbacks {
                if !fallback.trim().is_empty() {
                    models_to_try.push(fallback.trim().to_string());
                }
            }
        } else {
            let dynamic_fallbacks = self.config.get_dynamic_fallbacks(&self.profile.name);
            for fallback in dynamic_fallbacks {
                if !models_to_try.contains(&fallback) {
                    models_to_try.push(fallback);
                }
            }
        }

        let default_model = self.config.agents.defaults.model.clone();
        if !models_to_try.contains(&default_model) {
            models_to_try.push(default_model);
        }

        let child_session_id = format!("subagent:{}:{}", self.profile.name, &uuid::Uuid::new_v4().to_string()[..8]);
        let subagent_prompt = format!(
            "You are a specialized subagent operating under the following profile guidelines:\n\n\
            {}\n\n\
            TASK:\n{}\n\n\
            CONTEXT:\n{}\n\n\
            When finished, provide a clear, concise summary of what you did and found.",
            self.profile.system_prompt, clean_goal, clean_context
        );

        let is_reviewer = self.profile.name == "reviewer";
        let is_vision = self.profile.name == "vision_agent";
        let is_vision_profile = is_vision;
        let formatted_name = format_subagent_name(&self.profile.name);
        let mut last_error = None;

        let parent_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let workspace_dir = match create_isolated_workspace(&parent_dir) {
            Ok(dir) => {
                crate::tui_println!("{}  ✓ Isolated workspace worktree created at {:?}{}", EMERALD_GREEN, dir, COLOR_RESET);
                dir
            }
            Err(e) => {
                crate::tui_println!("{}⚠️ Failed to create isolated workspace ({:?}). Running in active workspace.{}", AURA_GOLD, e, COLOR_RESET);
                parent_dir.clone()
            }
        };

        let _worktree_guard = WorktreeGuard::new(parent_dir.clone(), workspace_dir.clone());

        for (idx, model_name) in models_to_try.iter().enumerate() {
            // For vision_agent, skip models that don't support vision to avoid wasting fallbacks
            if is_vision_profile && !crate::providers::model_supports_vision(model_name) {
                crate::tui_println!("{}▲ Skipping non-vision model '{}' for vision task{}", AURA_GOLD, model_name, COLOR_RESET);
                continue;
            }

            if idx > 0 {
                crate::tui_println!("{}▲ Primary model failed. Trying fallback model ({} of {}): {}{}", AURA_GOLD, idx, models_to_try.len() - 1, model_name, COLOR_RESET);
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

            let mut child_registry = ToolRegistry::new_with_context(
                self.config.clone(),
                provider.clone(),
                self.session_manager.clone(),
            );
            for tool in &filtered_parent_tools {
                child_registry.register(tool.clone());
            }

            // Only register delegate_task if allowed for this profile
            let allowed_delegate = match self.profile.name.as_str() {
                "planner" | "sop_designer" | "openz_coordinator" => true,
                "vision_agent" | "documentation_agent" | "self_improvement" | "skill_improvement" |
                "openz_maintainer" | "mcps_manager" | "git_ops_agent" | "ast_searcher" |
                "database_specialist" | "browser_operator" | "dependency_manager" |
                "frontend_architect" | "docs_lookup_agent" | "document_compiler" |
                "presentation_designer" | "code_synthesizer" | "summarizer_agent" |
                "media_designer" | "api_integrator" | "performance_tuner" |
                "communication_manager" | "reviewer" | "code_auditor" |
                "debugger" | "test_engineer" | "devops_agent" | "refactor_agent" |
                "memory_manager" | "automation_agent" | "coding_agent" => false,
                _ => true, // Custom subagents allow delegate_task by default
            };

            if allowed_delegate {
                child_registry.register(std::sync::Arc::new(DelegateTaskTool {
                    config: self.config.clone(),
                    parent_provider: provider.clone(),
                    session_manager: self.session_manager.clone(),
                    parent_tools: self.parent_tools.clone(),
                }));
            }

            let child_agent = AgentLoop::new(
                self.config.clone(),
                provider,
                child_registry,
                self.session_manager.clone(),
            );

            if is_reviewer {
                crate::tui_println!("{}◇ Reviewing...{}", AURA_PURPLE, COLOR_RESET);
            } else if is_vision {
                crate::tui_println!("{}◎ Vision Agent{}", AURA_PURPLE, COLOR_RESET);
            } else {
                crate::tui_println!("{}◎ {}{}", AURA_PURPLE, formatted_name, COLOR_RESET);
            }

            let spinner_msg = if is_reviewer {
                format!("{}  Reviewing...{}", AURA_SLATE, COLOR_RESET)
            } else if is_vision {
                format!("{}  Processing image...{}", AURA_SLATE, COLOR_RESET)
            } else {
                format!("{}  Running...{}", AURA_SLATE, COLOR_RESET)
            };

            let branch_id = format!("branch_{}", &uuid::Uuid::new_v4().to_string()[..8]);
            let mut has_memory_mcp = false;
            if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                match client.call_tool("create_database_branch", &serde_json::json!({ "branchId": branch_id })).await {
                    Ok(_) => {
                        crate::tui_println!("{}  ✓ Isolated simulation space branch '{}' created{}", EMERALD_GREEN, branch_id, COLOR_RESET);
                        has_memory_mcp = true;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create database branch: {:?}", e);
                    }
                }
            }

            let mut final_prompt = subagent_prompt.clone();
            if let Some(ref schema) = json_schema {
                final_prompt.push_str(&format!(
                    "\n\nCRITICAL REQUIREMENT: Your final response MUST be a raw JSON object strictly conforming to this JSON Schema:\n{}\nDo not wrap it in markdown code blocks, do not add any conversational text. Return only the raw valid JSON.",
                    serde_json::to_string_pretty(schema).unwrap_or_default()
                ));
            }

            let mut run_res = {
                let p_ref = &final_prompt;
                let c_ref = &child_session_id;
                let child_agent_ref = &child_agent;
                let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir.clone(), async {
                    DELEGATION_DEPTH.scope(current_depth + 1, async {
                        child_agent_ref.run(p_ref, c_ref).await
                    }).await
                });
                let run_res_timeout = tokio::time::timeout(std::time::Duration::from_secs(300), run_res_fut);
                match with_spinner(&spinner_msg, run_res_timeout).await {
                    Ok(res) => res,
                    Err(_) => Err(anyhow!("Subagent execution timed out after 5 minutes")),
                }
            };

            // Enforce schema validation on child agent success
            if let Some(ref schema) = json_schema {
                let mut attempts = 0;
                loop {
                    if let Ok(ref mut res) = run_res {
                        let text_output = res.content.trim();
                        let clean_json_str = if text_output.starts_with("```json") {
                            text_output.strip_prefix("```json").unwrap().strip_suffix("```").unwrap_or(text_output).trim()
                        } else if text_output.starts_with("```") {
                            text_output.strip_prefix("```").unwrap().strip_suffix("```").unwrap_or(text_output).trim()
                        } else {
                            text_output
                        };

                        let parsed_val: Result<Value, _> = serde_json::from_str(clean_json_str);
                        match parsed_val {
                            Ok(val) => {
                                match validate_schema(&val, schema) {
                                    Ok(_) => {
                                        res.content = clean_json_str.to_string();
                                        break;
                                    }
                                    Err(e) => {
                                        if attempts >= 2 {
                                            run_res = Err(anyhow!("Subagent output failed schema validation: {}", e));
                                            break;
                                        }
                                        attempts += 1;
                                        crate::tui_println!(
                                            "{}▲ [Reflection] Subagent JSON schema validation failed: {}. Retrying attempt {} of 2...{}",
                                            AURA_GOLD, e, attempts, COLOR_RESET
                                        );
                                        let retry_prompt = format!(
                                            "Your previous response did not conform to the JSON Schema. Validation Error: {}\n\n\
                                            Please correct your response. Return ONLY the raw valid JSON matching the schema.",
                                            e
                                        );
                                        let p_ref = &retry_prompt;
                                        let c_ref = &child_session_id;
                                        let child_agent_ref = &child_agent;
                                        let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir.clone(), async {
                                            DELEGATION_DEPTH.scope(current_depth + 1, async {
                                                child_agent_ref.run(p_ref, c_ref).await
                                            }).await
                                        });
                                        let run_res_timeout = tokio::time::timeout(std::time::Duration::from_secs(300), run_res_fut);
                                        run_res = match with_spinner(&spinner_msg, run_res_timeout).await {
                                            Ok(r) => r,
                                            Err(_) => Err(anyhow!("Subagent execution timed out after 5 minutes")),
                                        };
                                    }
                                }
                            }
                            Err(e) => {
                                if attempts >= 2 {
                                    run_res = Err(anyhow!("Subagent output failed to parse as JSON: {}. Parse Error: {}", e, text_output));
                                    break;
                                }
                                attempts += 1;
                                crate::tui_println!(
                                    "{}▲ [Reflection] Subagent output is not valid JSON: {}. Retrying attempt {} of 2...{}",
                                    AURA_GOLD, e, attempts, COLOR_RESET
                                );
                                let retry_prompt = format!(
                                    "Your previous response was not valid JSON. Parse Error: {}\n\n\
                                    Please correct your response. Return ONLY the raw valid JSON matching the schema.",
                                    e
                                );
                                let p_ref = &retry_prompt;
                                let c_ref = &child_session_id;
                                let child_agent_ref = &child_agent;
                                let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(workspace_dir.clone(), async {
                                    DELEGATION_DEPTH.scope(current_depth + 1, async {
                                        child_agent_ref.run(p_ref, c_ref).await
                                    }).await
                                });
                                let run_res_timeout = tokio::time::timeout(std::time::Duration::from_secs(300), run_res_fut);
                                run_res = match with_spinner(&spinner_msg, run_res_timeout).await {
                                    Ok(r) => r,
                                    Err(_) => Err(anyhow!("Subagent execution timed out after 5 minutes")),
                                };
                            }
                        }
                    } else {
                        break;
                    }
                }
            }

            match run_res {
                Ok(run_res) => {
                    if has_memory_mcp {
                        if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                            match client.call_tool("commit_database_branch", &serde_json::json!({})).await {
                                Ok(_) => crate::tui_println!("{}  ✓ Committed simulation space branch '{}'{}", EMERALD_GREEN, branch_id, COLOR_RESET),
                                Err(e) => tracing::warn!("Failed to commit database branch: {:?}", e),
                            }
                        }
                    }
                    crate::tui_println!("{}  ✓ Complete{}", EMERALD_GREEN, COLOR_RESET);
                    
                    if workspace_dir != parent_dir {
                        let _ = sync_changes_back(&workspace_dir, &parent_dir);
                    }

                    // Run evolution review
                    let _ = run_evolution_review(&self.parent_provider, &self.profile.name, &clean_goal, &clean_context, &run_res.content).await;

                    return Ok(serde_json::json!({
                        "status": "success",
                        "session_id": child_session_id,
                        "model_used": model_name,
                        "summary": run_res.content
                    }));
                }
                Err(e) => {
                    if has_memory_mcp {
                        if let Some(client) = crate::tools::mcp::get_memory_mcp_client() {
                            match client.call_tool("rollback_database_branch", &serde_json::json!({})).await {
                                Ok(_) => crate::tui_println!("{}  ✓ Rolled back simulation space branch '{}'{}", AURA_GOLD, branch_id, COLOR_RESET),
                                Err(e) => tracing::warn!("Failed to rollback database branch: {:?}", e),
                            }
                        }
                    }
                    crate::tui_println!("{}✕ Error: Model '{}' execution failed: {}{}", ERROR_RED, model_name, e, COLOR_RESET);
                    last_error = Some(e);
                }
            }
        }



        let err_msg = format!("All configured models/fallbacks failed for subagent '{}'. Last error: {:?}", self.profile.name, last_error);
        Ok(serde_json::json!({
            "status": "error",
            "error": err_msg
        }))
    }
}

pub fn build_provider_for_model(config: &Config, model: &str) -> Result<Arc<dyn LLMProvider>> {
    let defaults = &config.agents.defaults;
    let mut provider_name = defaults.provider.clone();
    let mut clean_model = model;

    let model_lower = model.to_lowercase();

    // 1. Check for explicit provider prefixes
    if model_lower.starts_with("openrouter/") {
        provider_name = "openrouter".to_string();
        clean_model = &model["openrouter/".len()..];
    } else if model_lower.starts_with("ollama/") {
        provider_name = "ollama".to_string();
        clean_model = &model["ollama/".len()..];
    } else if model_lower.starts_with("anthropic/") {
        provider_name = "anthropic".to_string();
        clean_model = &model["anthropic/".len()..];
    } else if model_lower.starts_with("openai/") {
        provider_name = "openai".to_string();
        clean_model = &model["openai/".len()..];
    } else if model_lower.starts_with("deepseek/") {
        provider_name = "deepseek".to_string();
        clean_model = &model["deepseek/".len()..];
    } else if model_lower.starts_with("groq/") {
        provider_name = "groq".to_string();
        clean_model = &model["groq/".len()..];
    } else if model_lower.starts_with("google_ai_studio/") {
        provider_name = "google_ai_studio".to_string();
        clean_model = &model["google_ai_studio/".len()..];
    } else if model_lower.starts_with("google-ai-studio/") {
        provider_name = "google_ai_studio".to_string();
        clean_model = &model["google-ai-studio/".len()..];
    } else if model_lower.starts_with("opencode_zen/") {
        provider_name = "opencode_zen".to_string();
        clean_model = &model["opencode_zen/".len()..];
    } else if model_lower.starts_with("opencode-zen/") {
        provider_name = "opencode_zen".to_string();
        clean_model = &model["opencode-zen/".len()..];
    } else if model_lower.starts_with("z.ai/") {
        provider_name = "z.ai".to_string();
        clean_model = &model["z.ai/".len()..];
    } else if model_lower.starts_with("z_ai/") {
        provider_name = "z.ai".to_string();
        clean_model = &model["z_ai/".len()..];
    } else if model_lower.starts_with("nvidia/") {
        provider_name = "nvidia".to_string();
        clean_model = &model["nvidia/".len()..];
    } else if model_lower.starts_with("minimax/") {
        provider_name = "minimax".to_string();
        clean_model = &model["minimax/".len()..];
    } else if model_lower.starts_with("mistral/") {
        provider_name = "mistral".to_string();
        clean_model = &model["mistral/".len()..];
    } else if model_lower.starts_with("cerebres/") {
        provider_name = "cerebres".to_string();
        clean_model = &model["cerebres/".len()..];
    } else if model_lower.starts_with("cerebras/") {
        provider_name = "cerebres".to_string();
        clean_model = &model["cerebras/".len()..];
    } else if provider_name == "auto" {
        // 2. Resolve based on model name keywords
        let has_key = |prov: &str| -> bool {
            match prov {
                "anthropic" => config.providers.anthropic.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("ANTHROPIC_API_KEY").is_ok(),
                "openai" => config.providers.openai.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENAI_API_KEY").is_ok(),
                "deepseek" => config.providers.deepseek.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("DEEPSEEK_API_KEY").is_ok(),
                "groq" => config.providers.groq.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("GROQ_API_KEY").is_ok(),
                "openrouter" => config.providers.openrouter.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENROUTER_API_KEY").is_ok(),
                "opencode_zen" => config.providers.opencode_zen.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENCODE_ZEN_API_KEY").is_ok(),
                "google_ai_studio" => config.providers.google_ai_studio.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("GOOGLE_AI_STUDIO_API_KEY").is_ok(),
                "z.ai" => config.providers.z_ai.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("Z_AI_API_KEY").is_ok(),
                "nvidia" => config.providers.nvidia.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("NVIDIA_API_KEY").is_ok(),
                "minimax" => config.providers.minimax.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("MINIMAX_API_KEY").is_ok(),
                "mistral" => config.providers.mistral.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("MISTRAL_API_KEY").is_ok(),
                "cerebres" => config.providers.cerebres.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("CEREBRES_API_KEY").is_ok() || std::env::var("CEBRAS_API_KEY").is_ok(),
                _ => false,
            }
        };

        if model_lower.contains("claude") {
            if has_key("anthropic") {
                provider_name = "anthropic".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "anthropic".to_string();
            }
        } else if model_lower.contains("gpt") {
            if has_key("openai") {
                provider_name = "openai".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "openai".to_string();
            }
        } else if model_lower.contains("deepseek") {
            if has_key("deepseek") {
                provider_name = "deepseek".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "deepseek".to_string();
            }
        } else if model_lower.contains("gemini") {
            if has_key("google_ai_studio") {
                provider_name = "google_ai_studio".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else {
                provider_name = "google_ai_studio".to_string();
            }
        } else if model_lower.contains("gemma") {
            if has_key("google_ai_studio") {
                provider_name = "google_ai_studio".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else {
                provider_name = "google_ai_studio".to_string();
            }
        } else if model_lower.contains("mistral") || model_lower.contains("codestral") {
            if has_key("mistral") {
                provider_name = "mistral".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else {
                provider_name = "mistral".to_string();
            }
        } else if model_lower.contains("ollama") {
            provider_name = "ollama".to_string();
        } else {
            if has_key("opencode_zen") {
                provider_name = "opencode_zen".to_string();
            } else if has_key("google_ai_studio") {
                provider_name = "google_ai_studio".to_string();
            } else if has_key("anthropic") {
                provider_name = "anthropic".to_string();
            } else if has_key("openai") {
                provider_name = "openai".to_string();
            } else if has_key("deepseek") {
                provider_name = "deepseek".to_string();
            } else if has_key("openrouter") {
                provider_name = "openrouter".to_string();
            } else if has_key("groq") {
                provider_name = "groq".to_string();
            } else if has_key("mistral") {
                provider_name = "mistral".to_string();
            } else if has_key("nvidia") {
                provider_name = "nvidia".to_string();
            } else if has_key("z.ai") {
                provider_name = "z.ai".to_string();
            } else {
                provider_name = "openai".to_string();
            }
        }
    }

    let (api_key, api_base) = match provider_name.as_str() {
        "anthropic" => {
            let p = config.providers.anthropic.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            (key, base)
        }
        "openai" => {
            let p = config.providers.openai.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            (key, base)
        }
        "openrouter" => {
            let p = config.providers.openrouter.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
            (key, base)
        }
        "deepseek" => {
            let p = config.providers.deepseek.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.deepseek.com/v1".to_string());
            (key, base)
        }
        "groq" => {
            let p = config.providers.groq.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("GROQ_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string());
            (key, base)
        }
        "ollama" => {
            let p = config.providers.ollama.as_ref();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
            (String::new(), base)
        }
        "minimax" => {
            let p = config.providers.minimax.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.minimax.io/v1".to_string());
            (key, base)
        }
        "mistral" => {
            let p = config.providers.mistral.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("MISTRAL_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string());
            (key, base)
        }
        "z.ai" => {
            let p = config.providers.z_ai.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("Z_AI_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.z.ai/api/paas/v4/".to_string());
            (key, base)
        }
        "nvidia" => {
            let p = config.providers.nvidia.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("NVIDIA_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://integrate.api.nvidia.com/v1".to_string());
            (key, base)
        }
        "opencode_zen" | "opencode zen" => {
            let p = config.providers.opencode_zen.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENCODE_ZEN_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://opencode.ai/zen/v1".to_string());
            (key, base)
        }
        "cerebres" => {
            let p = config.providers.cerebres.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("CEREBRES_API_KEY").ok())
                .or_else(|| std::env::var("CEBRAS_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://api.cerebras.ai/v1".to_string());
            (key, base)
        }
        "google_ai_studio" | "google ai studio" => {
            let p = config.providers.google_ai_studio.as_ref();
            let key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("GOOGLE_AI_STUDIO_API_KEY").ok())
                .unwrap_or_default();
            let base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta/openai/".to_string());
            (key, base)
        }
        _ => {
            return Err(anyhow!("Unsupported provider: {}", provider_name));
        }
    };

    let mut final_provider_name = provider_name.clone();
    let mut final_api_key = api_key;
    let mut final_api_base = api_base;
    let mut final_model = clean_model.to_string();

    if final_provider_name != "ollama" && final_api_key.is_empty() {
        let has_openrouter = config.providers.openrouter.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENROUTER_API_KEY").is_ok();
        let has_opencode_zen = config.providers.opencode_zen.as_ref().and_then(|p| p.api_key.as_ref()).is_some() || std::env::var("OPENCODE_ZEN_API_KEY").is_ok();

        if has_openrouter {
            let p = config.providers.openrouter.as_ref();
            final_api_key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .unwrap_or_default();
            final_api_base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
            final_provider_name = "openrouter".to_string();
            final_model = if clean_model.contains('/') {
                clean_model.to_string()
            } else {
                format!("{}/{}", provider_name, clean_model)
            };
        } else if has_opencode_zen {
            let p = config.providers.opencode_zen.as_ref();
            final_api_key = p.and_then(|x| x.api_key.clone())
                .or_else(|| std::env::var("OPENCODE_ZEN_API_KEY").ok())
                .unwrap_or_default();
            final_api_base = p.and_then(|x| x.api_base.clone())
                .unwrap_or_else(|| "https://opencode.ai/zen/v1".to_string());
            final_provider_name = "opencode_zen".to_string();
            final_model = if clean_model.contains('/') {
                clean_model.to_string()
            } else {
                format!("{}/{}", provider_name, clean_model)
            };
        } else {
            return Err(anyhow!("No API key configured for provider: {}", provider_name));
        }
    }

    let mut clean_model_str = final_model;
    if final_provider_name == "nvidia" {
        if clean_model_str.ends_with(":free") {
            clean_model_str = clean_model_str[..clean_model_str.len() - 5].to_string();
        }
        if !clean_model_str.contains('/') {
            clean_model_str = format!("nvidia/{}", clean_model_str);
        }
    } else if final_provider_name == "google_ai_studio" || final_provider_name == "google ai studio" {
        if clean_model_str.starts_with("google/") {
            clean_model_str = clean_model_str["google/".len()..].to_string();
        } else if clean_model_str.starts_with("models/") {
            clean_model_str = clean_model_str["models/".len()..].to_string();
        }
    }

    let provider: Arc<dyn LLMProvider> = if final_provider_name == "anthropic" {
        Arc::new(AnthropicProvider::new(final_api_key, final_api_base, clean_model_str))
    } else {
        Arc::new(OpenAIProvider::new(final_api_key, final_api_base, clean_model_str))
    };

    Ok(provider)
}

pub struct OptimizeSubagentTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
}

#[async_trait::async_trait]
impl Tool for OptimizeSubagentTool {
    fn name(&self) -> &str {
        "optimize_subagent"
    }

    fn description(&self) -> &str {
        "Optimize a specialized subagent's system prompt using AI based on feedback logs or execution errors."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subagent_name": {
                    "type": "string",
                    "description": "The name of the subagent to optimize (e.g. 'researcher', 'architect', 'reviewer')"
                },
                "feedback": {
                    "type": "string",
                    "description": "Details about the error, feedback, failed logs, or missing guidelines that occurred."
                }
            },
            "required": ["subagent_name", "feedback"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let subagent_name = arguments.get("subagent_name").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'subagent_name' argument"))?;
        let feedback = arguments.get("feedback").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'feedback' argument"))?;

        let mut profiles = crate::subagents::load_profiles()?;
        let pos = profiles.iter().position(|p| p.name == subagent_name)
            .ok_or_else(|| anyhow!("Subagent '{}' not found", subagent_name))?;

        let profile = &profiles[pos];


        let system_prompt_sum = "You are an expert prompt engineer. Optimize system prompts for specialized subagents. \
            Analyze the failed case feedback, and rewrite the subagent's system prompt to address the issue. \
            Ensure the prompt remains clear, structured, and focused. Return only the optimized system prompt, with no conversational text or markdown blocks.";

        let user_prompt = format!(
            "Subagent: {}\n\
            Current System Prompt:\n{}\n\n\
            Execution Feedback/Error:\n{}\n\n\
            Please return only the rewritten, optimized system prompt.",
            subagent_name, profile.system_prompt, feedback
        );

        let messages = vec![crate::session::Message {
            role: "user".to_string(),
            content: user_prompt,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            extra: serde_json::Map::new(),
        }];

        let settings = crate::providers::GenerationSettings {
            temperature: 0.2,
            max_tokens: 1024,
            reasoning_effort: None,
        };

        let spinner_msg = format!(
            "{}▸ [Prompt-Optimize] Asking OpenZ to optimize subagent prompt for '{}'...{}",
            AURA_PURPLE,
            subagent_name,
            COLOR_RESET
        );
        let chat_fut = self.parent_provider.chat(system_prompt_sum, &messages, &[], &settings);
        let resp = with_spinner(&spinner_msg, chat_fut).await?;
        let content = resp.content.ok_or_else(|| anyhow!("Failed to generate optimized prompt from AI"))?;

        let clean_prompt = content.trim().to_string();
        if clean_prompt.is_empty() {
            return Err(anyhow!("Received empty optimized prompt from AI"));
        }

        profiles[pos].system_prompt = clean_prompt.clone();
        crate::subagents::save_profiles(&profiles)?;

        crate::tui_println!("{}✓ [Prompt-Optimize] Optimized prompt for '{}' saved successfully.{}", EMERALD_GREEN, subagent_name, COLOR_RESET);

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Successfully optimized subagent '{}'", subagent_name),
            "new_system_prompt": clean_prompt
        }))
    }
}

pub struct EvaluatorOptimizerLoopTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
}

#[async_trait::async_trait]
impl Tool for EvaluatorOptimizerLoopTool {
    fn name(&self) -> &str {
        "evaluator_optimizer_loop"
    }

    fn description(&self) -> &str {
        "Run a stateful draft-and-review cycle (reflection loop) between an optimizer subagent (e.g. coding_agent) and an evaluator subagent (e.g. reviewer) to generate high-quality outputs."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "optimizer": {
                    "type": "string",
                    "description": "Name of the subagent to generate and refine the draft (e.g. 'coding_agent')."
                },
                "evaluator": {
                    "type": "string",
                    "description": "Name of the subagent to evaluate and review the draft (e.g. 'reviewer')."
                },
                "goal": {
                    "type": "string",
                    "description": "The specific goal or task to accomplish."
                },
                "context": {
                    "type": "string",
                    "description": "Additional context or background details required for the task."
                },
                "checklist": {
                    "type": "string",
                    "description": "Grading checklist or quality criteria for the evaluator to check against (e.g. 'Must include unit tests', 'No compilation warnings')."
                },
                "max_iterations": {
                    "type": "integer",
                    "description": "Maximum number of optimization iterations (default: 3)."
                }
            },
            "required": ["optimizer", "evaluator", "goal", "checklist"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let optimizer_name = arguments.get("optimizer").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'optimizer' argument"))?;
        let evaluator_name = arguments.get("evaluator").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'evaluator' argument"))?;
        let goal = arguments.get("goal").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'goal' argument"))?;
        let context = arguments.get("context").and_then(|v| v.as_str()).unwrap_or("");
        let checklist = arguments.get("checklist").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'checklist' argument"))?;
        let max_iterations = arguments.get("max_iterations").and_then(|v| v.as_i64()).unwrap_or(3) as usize;

        let profiles = crate::subagents::load_profiles()?;
        let optimizer_profile = profiles.iter().find(|p| p.name == optimizer_name)
            .ok_or_else(|| anyhow!("Optimizer subagent profile '{}' not found", optimizer_name))?;
        let evaluator_profile = profiles.iter().find(|p| p.name == evaluator_name)
            .ok_or_else(|| anyhow!("Evaluator subagent profile '{}' not found", evaluator_name))?;

        let optimizer_tool = DelegateProfileTool {
            config: self.config.clone(),
            parent_provider: self.parent_provider.clone(),
            session_manager: self.session_manager.clone(),
            profile: optimizer_profile.clone(),
            parent_tools: self.parent_tools.clone(),
        };

        let evaluator_tool = DelegateProfileTool {
            config: self.config.clone(),
            parent_provider: self.parent_provider.clone(),
            session_manager: self.session_manager.clone(),
            profile: evaluator_profile.clone(),
            parent_tools: self.parent_tools.clone(),
        };

        let mut optimizer_output = String::new();
        let mut feedback = String::new();
        let mut passed = false;
        let mut iterations_run = 0;

        let eval_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "passed": { "type": "boolean" },
                "feedback": {
                    "type": "string",
                    "description": "Detailed feedback describing precisely what checklist items failed, or empty if all passed."
                }
            },
            "required": ["passed", "feedback"]
        });

        for i in 1..=max_iterations {
            iterations_run = i;
            crate::tui_println!(
                "{}🔄 [Evaluator-Optimizer] Starting iteration {}/{} (Optimizer: '{}', Evaluator: '{}'){}",
                AURA_PURPLE, i, max_iterations, optimizer_name, evaluator_name, COLOR_RESET
            );

            // Invoke Optimizer
            let opt_goal = if i == 1 {
                goal.to_string()
            } else {
                format!(
                    "Your previous draft failed evaluation. Please refine it based on this feedback:\n\n\
                    FEEDBACK:\n{}\n\n\
                    CRITERIA CHECKLIST:\n{}\n\n\
                    ORIGINAL GOAL:\n{}",
                    feedback, checklist, goal
                )
            };

            let opt_context = if i == 1 {
                context.to_string()
            } else {
                format!(
                    "PREVIOUS DRAFT:\n{}\n\n\
                    {}",
                    optimizer_output, context
                )
            };

            let opt_res = optimizer_tool.call(&serde_json::json!({
                "goal": opt_goal,
                "context": opt_context
            })).await?;

            if opt_res.get("status").and_then(|v| v.as_str()) != Some("success") {
                let err = opt_res.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown optimizer error");
                return Err(anyhow!("Optimizer subagent '{}' failed: {}", optimizer_name, err));
            }

            optimizer_output = opt_res.get("summary").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Optimizer response missing summary"))?.to_string();

            // Invoke Evaluator
            let eval_goal = format!(
                "Review the draft produced by the optimizer against the following checklist criteria. Assess whether the draft passes all checklist items. If it fails any item, specify detailed feedback on how to fix it.\n\n\
                CHECKLIST CRITERIA:\n{}\n\n\
                OPTIMIZER DRAFT:\n{}",
                checklist, optimizer_output
            );

            let eval_context = format!(
                "Original task goal: {}\nOriginal context: {}",
                goal, context
            );

            let eval_res = evaluator_tool.call(&serde_json::json!({
                "goal": eval_goal,
                "context": eval_context,
                "json_schema": eval_schema
            })).await?;

            if eval_res.get("status").and_then(|v| v.as_str()) != Some("success") {
                let err = eval_res.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown evaluator error");
                return Err(anyhow!("Evaluator subagent '{}' failed: {}", evaluator_name, err));
            }

            let eval_summary = eval_res.get("summary").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Evaluator response missing summary"))?;

            let eval_json: Value = serde_json::from_str(eval_summary)
                .map_err(|e| anyhow!("Failed to parse evaluator JSON response ({}): {}", e, eval_summary))?;

            passed = eval_json.get("passed").and_then(|v| v.as_bool()).unwrap_or(false);
            feedback = eval_json.get("feedback").and_then(|v| v.as_str()).unwrap_or("").to_string();

            if passed {
                crate::tui_println!(
                    "{}✓ [Evaluator-Optimizer] Evaluation PASSED on iteration {}/{}!{}",
                    EMERALD_GREEN, i, max_iterations, COLOR_RESET
                );
                break;
            } else {
                crate::tui_println!(
                    "{}✕ [Evaluator-Optimizer] Evaluation FAILED on iteration {}/{}. Feedback: {}{}",
                    AURA_GOLD, i, max_iterations, feedback, COLOR_RESET
                );
            }
        }

        Ok(serde_json::json!({
            "status": if passed { "success" } else { "partial_success" },
            "iterations_run": iterations_run,
            "passed": passed,
            "final_output": optimizer_output,
            "final_feedback": feedback
        }))
    }
}

pub struct CreateSubagentTool {
    pub config: Config,
}

#[async_trait::async_trait]
impl Tool for CreateSubagentTool {
    fn name(&self) -> &str {
        "create_subagent"
    }

    fn description(&self) -> &str {
        "Create and save a new custom specialized subagent profile. The new subagent will be saved to the database and dynamically registered for future tasks."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Unique name for the subagent in lowercase alphanumeric/underscore format (e.g. 'twitter_researcher')"
                },
                "description": {
                    "type": "string",
                    "description": "A short summary of what this subagent is specialized in."
                },
                "system_prompt": {
                    "type": "string",
                    "description": "The detailed instructions and guidelines that define how this subagent operates."
                },
                "model": {
                    "type": "string",
                    "description": "Optional: The primary model to run (e.g. 'gpt-4o-mini', 'claude-3-5-sonnet', 'gpt-4o'). Default is 'gpt-4o-mini'."
                },
                "fallbacks": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional: Up to 3 fallback models to try if the primary model fails."
                }
            },
            "required": ["name", "description", "system_prompt"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let name = arguments.get("name").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'name' argument"))?.trim().to_string();
        let description = arguments.get("description").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'description' argument"))?.trim().to_string();
        let system_prompt = arguments.get("system_prompt").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'system_prompt' argument"))?.trim().to_string();
        let model = arguments.get("model").and_then(|v| v.as_str()).map(|s| s.trim().to_string());

        let mut fallbacks = Vec::new();
        if let Some(arr) = arguments.get("fallbacks").and_then(|v| v.as_array()) {
            for v in arr {
                if let Some(s) = v.as_str() {
                    let s_trimmed = s.trim().to_string();
                    if !s_trimmed.is_empty() {
                        fallbacks.push(s_trimmed);
                    }
                }
            }
        }
        let fallbacks_opt = if fallbacks.is_empty() {
            None
        } else {
            Some(fallbacks)
        };

        // Validate name format: starts with a letter, lowercase alphanumeric and underscore only
        if name.is_empty() || !name.chars().next().unwrap().is_ascii_alphabetic() || name.chars().any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '_') {
            return Err(anyhow!("Subagent name must start with a letter and contain only lowercase alphanumeric characters and underscores."));
        }

        // Do not allow overwriting default subagents
        let defaults = [
            "planner", "researcher", "architect", "skill_creator", "reviewer",
            "code_auditor", "debugger", "test_engineer", "devops_agent",
            "refactor_agent", "memory_manager", "vision_agent", "documentation_agent",
            "self_improvement", "skill_improvement", "openz_maintainer", "mcps_manager",
            "git_ops_agent", "ast_searcher", "database_specialist", "browser_operator",
            "dependency_manager", "frontend_architect", "docs_lookup_agent",
            "document_compiler", "presentation_designer", "code_synthesizer",
            "summarizer_agent", "media_designer", "openz_coordinator",
            "sop_designer", "api_integrator", "performance_tuner", "communication_manager",
            "automation_agent", "coding_agent"
        ];
        if defaults.contains(&name.as_str()) {
            return Err(anyhow!("Cannot overwrite default subagent '{}'", name));
        }

        let mut profiles = crate::subagents::load_profiles()?;
        let profile = SubagentProfile {
            name: name.clone(),
            description,
            system_prompt,
            model,
            fallbacks: fallbacks_opt,
            extra: serde_json::Map::new(),
        };

        if let Some(pos) = profiles.iter().position(|p| p.name == name) {
            profiles[pos] = profile;
        } else {
            profiles.push(profile);
        }

        crate::subagents::save_profiles(&profiles)?;

        crate::tui_println!("{}✓ Custom subagent '{}' created and saved.{}", EMERALD_GREEN, name, COLOR_RESET);

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Successfully created/updated subagent '{}'", name)
        }))
    }
}

pub struct DeleteSubagentTool;

#[async_trait::async_trait]
impl Tool for DeleteSubagentTool {
    fn name(&self) -> &str {
        "delete_subagent"
    }

    fn description(&self) -> &str {
        "Delete a custom subagent profile. Crucial: Default subagents cannot be deleted."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name of the custom subagent to delete (e.g. 'twitter_researcher')"
                }
            },
            "required": ["name"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let name = arguments.get("name").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'name' argument"))?.trim().to_string();

        let defaults = [
            "planner", "researcher", "architect", "skill_creator", "reviewer",
            "code_auditor", "debugger", "test_engineer", "devops_agent",
            "refactor_agent", "memory_manager", "vision_agent", "documentation_agent",
            "self_improvement", "skill_improvement", "openz_maintainer", "mcps_manager",
            "git_ops_agent", "ast_searcher", "database_specialist", "browser_operator",
            "dependency_manager", "frontend_architect", "docs_lookup_agent",
            "document_compiler", "presentation_designer", "code_synthesizer",
            "summarizer_agent", "media_designer", "openz_coordinator",
            "sop_designer", "api_integrator", "performance_tuner", "communication_manager",
            "automation_agent", "coding_agent"
        ];
        if defaults.contains(&name.as_str()) {
            return Err(anyhow!("Cannot delete default subagent '{}'", name));
        }

        let mut profiles = crate::subagents::load_profiles()?;
        let pos = profiles.iter().position(|p| p.name == name)
            .ok_or_else(|| anyhow!("Custom subagent '{}' not found", name))?;

        profiles.remove(pos);
        crate::subagents::save_profiles(&profiles)?;

        crate::tui_println!("{}✓ Custom subagent '{}' deleted.{}", EMERALD_GREEN, name, COLOR_RESET);

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Successfully deleted custom subagent '{}'", name)
        }))
    }
}

fn format_subagent_name(name: &str) -> String {
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

pub struct ParallelResearchTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
}

const READ_ONLY_TOOLS: &[&str] = &[
    "read_file",
    "find_files",
    "doc_reader",
    "semantic_search",
    "rust_docs",
    "list_dir",
    "web_fetch",
    "grep_search",
    "code_outline",
    "web_search",
    "db_inspector",
    "system_info",
    "check_port",
    "ast_grep",
    "crawl",
    "obscura",
    "recall_memory",
];

#[async_trait::async_trait]
impl Tool for ParallelResearchTool {
    fn name(&self) -> &str {
        "parallel_research"
    }

    fn description(&self) -> &str {
        "Run multiple independent research or analysis tasks concurrently in parallel using focused read-only subagents, and return their combined summaries. Use this when you have independent files/topics/searches to analyze simultaneously."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "goal": {
                                "type": "string",
                                "description": "The specific research task/question for the subagent to answer. Be precise."
                            },
                            "context": {
                                "type": "string",
                                "description": "Any additional context needed specifically for this task."
                            },
                            "model": {
                                "type": "string",
                                "description": "Optional model override name (e.g., 'gpt-4o-mini', 'claude-3-5-haiku') for the subagent."
                            }
                        },
                        "required": ["goal"]
                    },
                    "description": "List of independent research/analysis tasks to execute concurrently."
                }
            },
            "required": ["tasks"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let tasks_val = arguments.get("tasks").and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("Missing or invalid 'tasks' argument"))?;

        if tasks_val.is_empty() {
            return Err(anyhow!("The 'tasks' array cannot be empty"));
        }

        crate::tui_println!("{}◎ Parallel Research: Spawning {} subagents concurrently...{}", AURA_PURPLE, tasks_val.len(), COLOR_RESET);

        let mut join_handles = Vec::new();

        for task_val in tasks_val {
            let goal = match task_val.get("goal").and_then(|v| v.as_str()) {
                Some(g) => g.to_string(),
                None => continue,
            };
            let context = task_val.get("context").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let model_override = task_val.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());

            let config = self.config.clone();
            let parent_provider = self.parent_provider.clone();
            let session_manager = self.session_manager.clone();

            let mut read_only_parent_tools = Vec::new();
            for tool in &self.parent_tools {
                if READ_ONLY_TOOLS.contains(&tool.name()) {
                    read_only_parent_tools.push(tool.clone());
                }
            }

            let handle = tokio::spawn(crate::agent::style::spinner::IS_SILENT.scope(true, async move {
                let provider = if let Some(ref m) = model_override {
                    match build_provider_for_model(&config, m) {
                        Ok(p) => p,
                        Err(_) => parent_provider.clone()
                    }
                } else {
                    parent_provider.clone()
                };

                let mut child_registry = ToolRegistry::new_with_context(
                    config.clone(),
                    provider.clone(),
                    session_manager.clone(),
                );
                for tool in read_only_parent_tools {
                    child_registry.register(tool);
                }

                let child_session_id = format!("subagent:parallel:{}", &uuid::Uuid::new_v4().to_string()[..8]);
                let child_agent = AgentLoop::new(
                    config,
                    provider,
                    child_registry,
                    session_manager,
                );

                let subagent_prompt = format!(
                    "You are a focused research subagent. Complete the following task using the read-only tools available.\n\n\
                    TASK:\n{}\n\n\
                    CONTEXT:\n{}\n\n\
                    When finished, provide a clear, concise summary of what you did and found.",
                    goal, context
                );

                let run_res = child_agent.run(&subagent_prompt, &child_session_id).await;
                (goal, run_res)
            }));
            join_handles.push(handle);
        }

        let join_results = futures_util::future::join_all(join_handles).await;
        let mut results = Vec::new();

        for res in join_results {
            match res {
                Ok((goal, Ok(run_res))) => {
                    results.push(serde_json::json!({
                        "task": goal,
                        "status": "success",
                        "summary": run_res.content
                    }));
                }
                Ok((goal, Err(e))) => {
                    results.push(serde_json::json!({
                        "task": goal,
                        "status": "error",
                        "error": format!("{:?}", e)
                    }));
                }
                Err(e) => {
                    results.push(serde_json::json!({
                        "status": "error",
                        "error": format!("Task join failed: {:?}", e)
                    }));
                }
            }
        }

        Ok(serde_json::json!({
            "status": "success",
            "results": results
        }))
    }
}

pub struct WorktreeGuard {
    pub parent_dir: std::path::PathBuf,
    pub worktree_dir: std::path::PathBuf,
    pub active: bool,
}

impl WorktreeGuard {
    pub fn new(parent_dir: std::path::PathBuf, worktree_dir: std::path::PathBuf) -> Self {
        Self {
            parent_dir,
            worktree_dir,
            active: true,
        }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        if self.active && self.worktree_dir != self.parent_dir {
            cleanup_isolated_workspace(&self.parent_dir, &self.worktree_dir);
        }
    }
}

fn dir_size(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            size += dir_size(&entry.path());
        }
    }
    size
}

fn enforce_disk_quota() {
    let worktrees_dir = crate::config::resolve_path("~/.openz/worktrees");
    if !worktrees_dir.exists() || !worktrees_dir.is_dir() {
        return;
    }

    let quota: u64 = 5 * 1024 * 1024 * 1024; // 5 GB
    let mut total_size = dir_size(&worktrees_dir);
    if total_size <= quota {
        return;
    }

    let mut worktrees = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&worktrees_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("openz_worktree_") {
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            worktrees.push((path, modified));
                        }
                    }
                }
            }
        }
    }

    worktrees.sort_by(|a, b| a.1.cmp(&b.1));

    for (path, _) in worktrees {
        if total_size <= quota {
            break;
        }
        let size = dir_size(&path);
        if std::fs::remove_dir_all(&path).is_ok() {
            total_size = total_size.saturating_sub(size);
        }
    }
}

pub fn cleanup_stale_resources() {
    // 1. Run git worktree prune in current directory if it's a git repo
    let parent_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&parent_dir)
        .output();
    if let Ok(out) = git_check {
        if out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true" {
            let _ = std::process::Command::new("git")
                .args(["worktree", "prune"])
                .current_dir(&parent_dir)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }

    let ttl_seconds = 3600; // 1 hour TTL for active worktrees

    // 2. Clean dedicated directory (~/.openz/worktrees)
    let worktrees_dir = crate::config::resolve_path("~/.openz/worktrees");
    if worktrees_dir.exists() && worktrees_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&worktrees_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.starts_with("openz_worktree_") {
                        if is_older_than(&path, ttl_seconds) {
                            let _ = std::fs::remove_dir_all(&path);
                        }
                    }
                }
            }
        }
    }

    // 3. Clean legacy /tmp/openz_worktree_* directories
    let tmp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("openz_worktree_") {
                    if is_older_than(&path, ttl_seconds) {
                        let _ = std::fs::remove_dir_all(&path);
                    }
                }
            }
        }
    }

    let seven_days_in_seconds = 7 * 24 * 3600;

    // 4. Clean tool_outputs (~/.openz/tool_outputs)
    let tool_outputs_dir = crate::config::resolve_path("~/.openz/tool_outputs");
    if tool_outputs_dir.exists() && tool_outputs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&tool_outputs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if is_older_than(&path, seven_days_in_seconds) {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }

    // 5. Clean traces (~/.openz/traces)
    let traces_dir = crate::config::resolve_path("~/.openz/traces");
    if traces_dir.exists() && traces_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&traces_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if is_older_than(&path, seven_days_in_seconds) {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }

    // 6. Clean cron_logs (~/.openz/cron_logs)
    let cron_logs_dir = crate::config::resolve_path("~/.openz/cron_logs");
    if cron_logs_dir.exists() && cron_logs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&cron_logs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if is_older_than(&path, seven_days_in_seconds) {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }
}

fn is_older_than(path: &std::path::Path, seconds: u64) -> bool {
    if let Ok(metadata) = std::fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                return elapsed.as_secs() > seconds;
            }
        }
    }
    false
}

fn create_isolated_workspace(parent_dir: &std::path::Path) -> Result<std::path::PathBuf> {
    enforce_disk_quota();

    let worktrees_dir = crate::config::resolve_path("~/.openz/worktrees");
    if !worktrees_dir.exists() {
        let _ = std::fs::create_dir_all(&worktrees_dir);
    }
    let temp_dir = worktrees_dir.join(format!("openz_worktree_{}", &uuid::Uuid::new_v4().to_string()[..8]));

    // 1. Check if parent_dir is a git repository
    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(parent_dir)
        .output();

    let is_git = match git_check {
        Ok(out) => out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true",
        Err(_) => false,
    };

    if is_git {
        // 2. Create git worktree
        let worktree_add = std::process::Command::new("git")
            .args(["worktree", "add", "--detach", temp_dir.to_str().unwrap()])
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match worktree_add {
            Ok(status) if status.success() => {
                // 3. Sync uncommitted changes (modified, added, deleted, untracked files)
                if let Ok(status_out) = std::process::Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(parent_dir)
                    .output()
                {
                    let stdout = String::from_utf8_lossy(&status_out.stdout);
                    for line in stdout.lines() {
                        if line.len() < 4 {
                            continue;
                        }
                        let status_code = &line[..2];
                        let file_path_str = &line[3..];
                        
                        let file_path = if status_code.starts_with('R') {
                            if let Some(pos) = file_path_str.find(" -> ") {
                                &file_path_str[pos + 4..]
                            } else {
                                file_path_str
                            }
                        } else {
                            file_path_str
                        };

                        let src = parent_dir.join(file_path);
                        let dst = temp_dir.join(file_path);

                        if status_code.contains('D') {
                            let _ = std::fs::remove_file(&dst);
                        } else {
                            if src.exists() {
                                if let Some(parent) = dst.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                let _ = std::fs::copy(&src, &dst);
                            }
                        }
                    }
                }
                return Ok(temp_dir);
            }
            _ => {
                // If git worktree add fails, fallback to recursive copy
            }
        }
    }

    // Fallback: Copy workspace files recursively (skipping heavy dirs)
    std::fs::create_dir_all(&temp_dir)?;
    copy_dir_recursive_filtered(parent_dir, &temp_dir)?;
    Ok(temp_dir)
}

fn copy_dir_recursive_filtered(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }

    if src.is_dir() {
        let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "target" || name == "node_modules" || name == ".git" || name == ".fastembed_cache" || name == ".sediment" || name == "logs" {
            return Ok(());
        }

        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let entry_path = entry.path();
            let entry_name = entry_path.file_name().unwrap();
            copy_dir_recursive_filtered(&entry_path, &dst.join(entry_name))?;
        }
    } else {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

fn cleanup_isolated_workspace(parent_dir: &std::path::Path, worktree_dir: &std::path::Path) {
    let git_check = std::process::Command::new("git")
        .args(["worktree", "list"])
        .current_dir(parent_dir)
        .output();

    let is_worktree = match git_check {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains(worktree_dir.to_str().unwrap_or("____invalid____"))
        }
        Err(_) => false,
    };

    if is_worktree {
        let _ = std::process::Command::new("git")
            .args(["worktree", "remove", "--force", worktree_dir.to_str().unwrap()])
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        
        let _ = std::process::Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    } else {
        let _ = std::fs::remove_dir_all(worktree_dir);
    }
}

fn sync_changes_back(src_dir: &std::path::Path, dst_dir: &std::path::Path) -> Result<()> {
    let git_check = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(src_dir)
        .output();

    if let Ok(status_out) = git_check {
        let stdout = String::from_utf8_lossy(&status_out.stdout);
        for line in stdout.lines() {
            if line.len() < 4 {
                continue;
            }
            let status_code = &line[..2];
            let file_path_str = &line[3..];
            
            let file_path = if status_code.starts_with('R') {
                if let Some(pos) = file_path_str.find(" -> ") {
                    &file_path_str[pos + 4..]
                } else {
                    file_path_str
                }
            } else {
                file_path_str
            };

            let src = src_dir.join(file_path);
            let dst = dst_dir.join(file_path);

            if status_code.contains('D') {
                let _ = std::fs::remove_file(&dst);
            } else {
                if src.exists() {
                    if let Some(parent) = dst.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::copy(&src, &dst);
                }
            }
        }
    } else {
        copy_dir_recursive_filtered(src_dir, dst_dir)?;
    }
    Ok(())
}

async fn run_evolution_review(
    provider: &std::sync::Arc<dyn LLMProvider>,
    profile_name: &str,
    goal: &str,
    context: &str,
    summary: &str,
) -> Result<()> {
    let system_prompt = "You are a specialized Subagent Reviewer. Your task is to evaluate if a subagent successfully completed its task, and if so, extract any procedural skills or guidelines discovered during execution.\n\n\
        Review the subagent's goal, the context, and the summary of what it did and found.\n\n\
        Perform two tasks:\n\
        1. SUCCESS EVALUATION: Decide if the subagent succeeded in accomplishing the goal (true or false).\n\
        2. SKILL EXTRACTION: If the subagent succeeded, extract any reusable procedural guidelines, rules, tool usage lessons, or coding patterns it discovered. Avoid general descriptions; make them actionable instructions for future runs. Format the extracted guidelines in Markdown with a clear title (# Skill: ...), a description of when to use it, specific guidelines, and examples.\n\n\
        Provide your response as a raw JSON object with the following structure:\n\n\
        JSON Format:\n\
        {\n\
          \"success\": true,\n\
          \"skill_name\": \"cargo_check_workaround\",\n\
          \"skill_content\": \"# Skill: Cargo Check Workaround\\n\\nWhen cargo check fails with X, do Y...\"\n\
        }\n\n\
        Do not output any introductory or conversational text, only the raw JSON.";

    let user_prompt = format!(
        "Subagent Profile: {}\n\
         Goal: {}\n\
         Context: {}\n\
         Subagent Summary of Work:\n{}\n\n\
         Please review the above execution, evaluate success, and extract any reusable skills.",
         profile_name, goal, context, summary
    );

    let messages = vec![crate::session::Message {
        role: "user".to_string(),
        content: user_prompt,
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra: serde_json::Map::new(),
    }];

    let settings = crate::providers::GenerationSettings {
        temperature: 0.1,
        max_tokens: 1536,
        reasoning_effort: None,
    };

    let spinner_msg = format!("{}◇ [Evolution] Evaluating subagent success & extracting skills...{}", AURA_PURPLE, COLOR_RESET);
    let resp = with_spinner(&spinner_msg, provider.chat(system_prompt, &messages, &[], &settings)).await?;
    let content = resp.content.ok_or_else(|| anyhow!("No content returned from AI"))?;

    // Parse JSON
    let mut clean_json = content.trim();
    if clean_json.starts_with("```json") {
        clean_json = clean_json.strip_prefix("```json").unwrap();
    } else if clean_json.starts_with("```") {
        clean_json = clean_json.strip_prefix("```").unwrap();
    }
    if clean_json.ends_with("```") {
        clean_json = clean_suffix_ticks(clean_json);
    }
    let clean_json = clean_json.trim();

    #[derive(serde::Deserialize)]
    struct ReviewRes {
        success: bool,
        skill_name: String,
        skill_content: String,
    }

    if let Ok(review) = serde_json::from_str::<ReviewRes>(clean_json) {
        if review.success {
            let s_name = review.skill_name.trim().to_lowercase().replace(' ', "_");
            let s_content = review.skill_content.trim();
            if !s_name.is_empty() && !s_content.is_empty() {
                crate::agent::skills::save_subagent_skill(profile_name, &s_name, s_content)?;
                crate::tui_println!(
                    "{}✓ [Evolution] Extracted and saved skill '{}' for subagent '{}'{}",
                    EMERALD_GREEN, s_name, profile_name, COLOR_RESET
                );
            }
        } else {
            crate::tui_println!(
                "{}▲ [Evolution] Subagent task evaluation: Unsuccessful. No skill files updated.{}",
                AURA_GOLD, COLOR_RESET
            );
        }
    }

    Ok(())
}

fn validate_schema(value: &serde_json::Value, schema: &serde_json::Value) -> Result<(), String> {
    if let Some(schema_obj) = schema.as_object() {
        if let Some(enum_vals) = schema_obj.get("enum").and_then(|e| e.as_array()) {
            if !enum_vals.contains(value) {
                return Err(format!("Value {:?} is not one of the allowed enum values: {:?}", value, enum_vals));
            }
        }

        if let Some(any_of) = schema_obj.get("anyOf").and_then(|a| a.as_array()) {
            let mut matched = false;
            let mut errs = Vec::new();
            for sub_schema in any_of {
                match validate_schema(value, sub_schema) {
                    Ok(_) => {
                        matched = true;
                        break;
                    }
                    Err(e) => {
                        errs.push(e);
                    }
                }
            }
            if !matched {
                return Err(format!("Value does not match anyOf schemas: {:?}", errs));
            }
        }

        if let Some(types) = schema_obj.get("type") {
            match types {
                serde_json::Value::String(t) => {
                    match t.as_str() {
                        "object" => {
                            if !value.is_object() {
                                return Err(format!("Expected object, found {:?}", value));
                            }
                            let val_obj = value.as_object().unwrap();
                            if let Some(properties) = schema_obj.get("properties").and_then(|p| p.as_object()) {
                                for (prop_name, prop_schema) in properties {
                                    if let Some(prop_val) = val_obj.get(prop_name) {
                                        validate_schema(prop_val, prop_schema)
                                            .map_err(|e| format!("Property '{}': {}", prop_name, e))?;
                                    }
                                }
                            }
                            if let Some(required) = schema_obj.get("required").and_then(|r| r.as_array()) {
                                for req in required {
                                    if let Some(req_str) = req.as_str() {
                                        if !val_obj.contains_key(req_str) {
                                            return Err(format!("Missing required property '{}'", req_str));
                                        }
                                    }
                                }
                            }
                        }
                        "array" => {
                            if !value.is_array() {
                                return Err(format!("Expected array, found {:?}", value));
                            }
                            let val_arr = value.as_array().unwrap();
                            if let Some(items) = schema_obj.get("items") {
                                for (idx, item_val) in val_arr.iter().enumerate() {
                                    validate_schema(item_val, items)
                                        .map_err(|e| format!("Item at index {}: {}", idx, e))?;
                                }
                            }
                        }
                        "string" => {
                            if !value.is_string() {
                                return Err(format!("Expected string, found {:?}", value));
                            }
                        }
                        "number" => {
                            if !value.is_number() {
                                return Err(format!("Expected number, found {:?}", value));
                            }
                        }
                        "integer" => {
                            if !value.is_number() || (value.is_f64() && value.as_f64().unwrap().fract() != 0.0) {
                                return Err(format!("Expected integer, found {:?}", value));
                            }
                        }
                        "boolean" => {
                            if !value.is_boolean() {
                                return Err(format!("Expected boolean, found {:?}", value));
                            }
                        }
                        _ => {}
                    }
                }
                serde_json::Value::Array(arr) => {
                    let mut matched = false;
                    let mut errs = Vec::new();
                    for t_val in arr {
                        if let Some(t_str) = t_val.as_str() {
                            let mut dummy_schema = schema_obj.clone();
                            dummy_schema.insert("type".to_string(), serde_json::Value::String(t_str.to_string()));
                            match validate_schema(value, &serde_json::Value::Object(dummy_schema)) {
                                Ok(_) => {
                                    matched = true;
                                    break;
                                }
                                Err(e) => {
                                    errs.push(e);
                                }
                            }
                        }
                    }
                    if !matched {
                        return Err(format!("Value does not match any of the allowed types: {:?}", errs));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn clean_suffix_ticks(s: &str) -> &str {
    if s.ends_with("```") {
        &s[..s.len() - 3]
    } else {
        s
    }
}

fn filter_tools_for_subagent(subagent_name: &str, all_tools: &[Arc<dyn Tool>]) -> Vec<Arc<dyn Tool>> {
    let allowed_names: Option<&[&str]> = match subagent_name {
        "planner" => Some(&[
            "read_file", "list_dir", "find_files", "code_outline", 
            "parallel_research", "evaluator_optimizer_loop"
        ]),
        "researcher" => Some(&[
            "read_file", "list_dir", "find_files", "web_fetch", 
            "web_search", "doc_reader", "semantic_search", "crawl", 
            "obscura"
        ]),
        "architect" => Some(&[
            "read_file", "write_file", "list_dir", "find_files", 
            "code_outline", "ast_grep", "db_inspector"
        ]),
        "git_ops_agent" => Some(&[
            "read_file", "list_dir", "git_manager"
        ]),
        "ast_searcher" => Some(&[
            "read_file", "list_dir", "find_files", "ast_grep", 
            "code_outline", "grep_search"
        ]),
        "database_specialist" => Some(&[
            "read_file", "list_dir", "db_inspector"
        ]),
        "browser_operator" => Some(&[
            "read_file", "list_dir", "web_fetch", "crawl", "obscura"
        ]),
        "dependency_manager" => Some(&[
            "read_file", "write_file", "list_dir", "cargo_manager", "onpkg"
        ]),
        "frontend_architect" => Some(&[
            "read_file", "write_file", "list_dir", "generate_image"
        ]),
        "docs_lookup_agent" => Some(&[
            "read_file", "list_dir", "web_fetch", "web_search", "rust_docs"
        ]),
        "media_designer" => Some(&[
            "read_file", "write_file", "list_dir", "generate_image"
        ]),
        "sop_designer" => Some(&[
            "read_file", "write_file", "list_dir"
        ]),
        "api_integrator" => Some(&[
            "read_file", "write_file", "list_dir", "web_fetch", "web_search", "exec_command"
        ]),
        "performance_tuner" => Some(&[
            "read_file", "list_dir", "system_info", "exec_command"
        ]),
        "communication_manager" => Some(&[
            "read_file", "list_dir", "check_port"
        ]),
        "document_compiler" => Some(&[
            "read_file", "write_file", "list_dir", "find_files", "doc_reader", "exec_command", "compile_template"
        ]),
        "presentation_designer" => Some(&[
            "read_file", "write_file", "list_dir", "find_files", "exec_command", "generate_image", "compile_template"
        ]),
        "code_synthesizer" => Some(&[
            "read_file", "write_file", "list_dir", "find_files", "onpkg", "code_outline", "cargo_manager"
        ]),
        "summarizer_agent" => Some(&[
            "read_file", "write_file", "list_dir", "grep_search"
        ]),
        "automation_agent" => Some(&[
            "read_file", "write_file", "list_dir", "find_files", "gsd_browser", "obscura",
            "crawl", "web_fetch", "schedule_job", "list_jobs", "remove_job", "exec_command", "manage_mcp"
        ]),
        "coding_agent" => Some(&[
            "read_file", "write_file", "list_dir", "find_files", "code_outline", "ast_grep",
            "grep_search", "exec_command", "cargo_manager"
        ]),
        "reviewer" | "code_auditor" => Some(&[
            "read_file", "list_dir", "code_outline", "ast_grep", "grep_search"
        ]),
        "debugger" => Some(&[
            "read_file", "write_file", "list_dir", "code_outline", "grep_search", 
            "exec_command", "cargo_manager"
        ]),
        "test_engineer" => Some(&[
            "read_file", "write_file", "list_dir", "exec_command", "cargo_manager"
        ]),
        "devops_agent" => Some(&[
            "read_file", "write_file", "list_dir", "exec_command"
        ]),
        "refactor_agent" => Some(&[
            "read_file", "write_file", "list_dir", "code_outline", "ast_grep", "grep_search"
        ]),
        "memory_manager" | "self_improvement" | "skill_improvement" => Some(&[
            "read_file", "write_file", "list_dir", "find_files"
        ]),
        "openz_maintainer" => Some(&[
            "read_file", "write_file", "list_dir", "exec_command", "cargo_manager"
        ]),
        "mcps_manager" => Some(&[
            "read_file", "write_file", "list_dir", "manage_mcp", "exec_command"
        ]),
        // Coordinator and custom profiles inherit all tools
        _ => None,
    };

    if let Some(allowed) = allowed_names {
        all_tools.iter()
            .filter(|t| allowed.contains(&t.name()))
            .cloned()
            .collect()
    } else {
        all_tools.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::Config;
    use crate::session::SessionManager;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_delegation_depth_limit() {
        let tool = DelegateTaskTool {
            config: Config::default(),
            parent_provider: Arc::new(crate::providers::openai::OpenAIProvider::new(
                "mock_key".to_string(),
                "mock_base".to_string(),
                "gpt-4o-mini".to_string(),
            )),
            session_manager: SessionManager::new(std::env::temp_dir()),
            parent_tools: Vec::new(),
        };

        // If DELEGATION_DEPTH is 3, calling the tool should return an error immediately
        let res = DELEGATION_DEPTH.scope(3, async {
            tool.call(&serde_json::json!({
                "goal": "Test nested delegation safety"
            })).await
        }).await;

        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Delegation limit reached"));
    }

    #[test]
    fn test_validate_schema_success() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "inactive"]
                }
            },
            "required": ["name", "age"]
        });

        let value = serde_json::json!({
            "name": "Aswin",
            "age": 25,
            "tags": ["rust", "ai"],
            "status": "active"
        });

        assert!(validate_schema(&value, &schema).is_ok());
    }

    #[test]
    fn test_validate_schema_failure() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["name", "age"]
        });

        // Missing required field
        let val_missing = serde_json::json!({
            "name": "Aswin"
        });
        assert!(validate_schema(&val_missing, &schema).is_err());

        // Incorrect type
        let val_bad_type = serde_json::json!({
            "name": "Aswin",
            "age": "twenty-five"
        });
        assert!(validate_schema(&val_bad_type, &schema).is_err());
    }

    struct MockTool {
        name: String,
    }

    #[async_trait::async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "mock"
        }
        fn parameters(&self) -> Value {
            serde_json::json!({})
        }
        async fn call(&self, _args: &Value) -> Result<Value> {
            Ok(serde_json::json!({}))
        }
    }

    #[test]
    fn test_filter_tools_for_new_default_subagents() {
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(MockTool { name: "read_file".to_string() }),
            Arc::new(MockTool { name: "write_file".to_string() }),
            Arc::new(MockTool { name: "list_dir".to_string() }),
            Arc::new(MockTool { name: "find_files".to_string() }),
            Arc::new(MockTool { name: "doc_reader".to_string() }),
            Arc::new(MockTool { name: "exec_command".to_string() }),
            Arc::new(MockTool { name: "generate_image".to_string() }),
            Arc::new(MockTool { name: "onpkg".to_string() }),
            Arc::new(MockTool { name: "code_outline".to_string() }),
            Arc::new(MockTool { name: "cargo_manager".to_string() }),
            Arc::new(MockTool { name: "grep_search".to_string() }),
            Arc::new(MockTool { name: "compile_template".to_string() }),
            Arc::new(MockTool { name: "some_other_tool".to_string() }),
        ];

        // Test document_compiler
        let filtered = filter_tools_for_subagent("document_compiler", &tools);
        assert_eq!(filtered.len(), 7);
        assert!(filtered.iter().any(|t| t.name() == "compile_template"));
        assert!(filtered.iter().any(|t| t.name() == "doc_reader"));
        assert!(!filtered.iter().any(|t| t.name() == "onpkg"));

        // Test presentation_designer
        let filtered = filter_tools_for_subagent("presentation_designer", &tools);
        assert_eq!(filtered.len(), 7);
        assert!(filtered.iter().any(|t| t.name() == "compile_template"));
        assert!(filtered.iter().any(|t| t.name() == "generate_image"));
        assert!(!filtered.iter().any(|t| t.name() == "doc_reader"));

        // Test code_synthesizer
        let filtered = filter_tools_for_subagent("code_synthesizer", &tools);
        assert_eq!(filtered.len(), 7);
        assert!(filtered.iter().any(|t| t.name() == "onpkg"));
        assert!(!filtered.iter().any(|t| t.name() == "generate_image"));

        // Test summarizer_agent
        let filtered = filter_tools_for_subagent("summarizer_agent", &tools);
        assert_eq!(filtered.len(), 4);
        assert!(filtered.iter().any(|t| t.name() == "grep_search"));
        assert!(!filtered.iter().any(|t| t.name() == "onpkg"));
    }

    struct LoopMockProvider {
        call_count: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl crate::providers::LLMProvider for LoopMockProvider {
        async fn chat(
            &self,
            system_prompt: &str,
            messages: &[crate::session::Message],
            _tools: &[serde_json::Value],
            _settings: &crate::providers::GenerationSettings,
        ) -> Result<crate::providers::LLMResponse> {
            let count = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            println!("MOCK PROVIDER CHAT: count={}, system_prompt_len={}", count, system_prompt.len());
            for (idx, msg) in messages.iter().enumerate() {
                println!("  Message {}: role={}, content={}", idx, msg.role, msg.content);
            }
            
            // Check if it's the evaluator call by looking at system prompt or content
            let is_evaluator = system_prompt.contains("Review the draft produced by the optimizer") 
                || messages.iter().any(|m| m.content.contains("Review the draft produced by the optimizer"));

            if is_evaluator {
                // Check if optimizer draft has "Draft version 0" (meaning iteration 1)
                let has_v0 = system_prompt.contains("Draft version 0") 
                    || messages.iter().any(|m| m.content.contains("Draft version 0"));
                if has_v0 {
                    Ok(crate::providers::LLMResponse {
                        content: Some(r#"{"passed": false, "feedback": "Draft needs more detail and standard hello function"}"#.to_string()),
                        tool_calls: Vec::new(),
                        finish_reason: "stop".to_string(),
                        reasoning_content: None,
                    })
                } else {
                    Ok(crate::providers::LLMResponse {
                        content: Some(r#"{"passed": true, "feedback": ""}"#.to_string()),
                        tool_calls: Vec::new(),
                        finish_reason: "stop".to_string(),
                        reasoning_content: None,
                    })
                }
            } else {
                // Optimizer call
                Ok(crate::providers::LLMResponse {
                    content: Some(format!("Draft version {}", count)),
                    tool_calls: Vec::new(),
                    finish_reason: "stop".to_string(),
                    reasoning_content: None,
                })
            }
        }
    }

    struct TestLock;

    impl TestLock {
        fn acquire() -> Self {
            let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
            loop {
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&lock_path)
                {
                    Ok(_) => break,
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }
            TestLock
        }
    }

    impl Drop for TestLock {
        fn drop(&mut self) {
            let lock_path = std::env::temp_dir().join("openz_test_config_dir.lock");
            let _ = std::fs::remove_file(lock_path);
        }
    }

    #[tokio::test]
    async fn test_evaluator_optimizer_loop_success() -> Result<()> {
        let _lock = TestLock::acquire();
        let temp_dir = std::env::temp_dir().join(format!("openz_eval_opt_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        std::env::set_var("OPENZ_CONFIG_DIR", &temp_dir);
        std::env::set_var("ANTHROPIC_API_KEY", "dummy");
        std::env::set_var("OPENAI_API_KEY", "dummy");
        std::env::set_var("OPENZ_USE_MOCK_PROVIDER", "true");

        let provider = Arc::new(LoopMockProvider {
            call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });

        let tool = EvaluatorOptimizerLoopTool {
            config: Config::default(),
            parent_provider: provider.clone(),
            session_manager: SessionManager::new(temp_dir.clone()),
            parent_tools: Vec::new(),
        };

        let res = tool.call(&serde_json::json!({
            "optimizer": "coding_agent",
            "evaluator": "reviewer",
            "goal": "Write a hello world program in Rust",
            "checklist": "Must have a main function and print hello",
            "max_iterations": 3
        })).await?;

        assert_eq!(res.get("status").and_then(|v| v.as_str()), Some("success"));
        assert_eq!(res.get("passed").and_then(|v| v.as_bool()), Some(true));
        assert!(res.get("iterations_run").and_then(|v| v.as_i64()).unwrap() > 1);
        assert!(res.get("final_output").and_then(|v| v.as_str()).unwrap().contains("Draft version"));

        // Cleanup config dir
        std::env::remove_var("OPENZ_CONFIG_DIR");
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENZ_USE_MOCK_PROVIDER");
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}


