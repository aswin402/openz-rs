use super::{build_provider_for_model, CancellationToken, DELEGATION_DEPTH};
use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::session::SessionManager;
use crate::tools::Tool;
use crate::tools::ToolRegistry;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::Arc;

pub struct ParallelResearchTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
    pub cancellation_token: CancellationToken,
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

    fn metadata(&self) -> crate::tools::ToolMetadata {
        super::subagent_tool_metadata("parallel_research")
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
                            },
                            "timeout_secs": {
                                "type": "integer",
                                "description": "Optional timeout in seconds for this subagent task. Overrides the default tool timeout."
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
        let tasks_val = arguments
            .get("tasks")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("Missing or invalid 'tasks' argument"))?;

        if tasks_val.is_empty() {
            return Err(anyhow!("The 'tasks' array cannot be empty"));
        }

        let is_parent_silent = crate::agent::style::is_silent();
        if !is_parent_silent {
            let prefix = crate::agent::style::get_tree_prefix(false);
            crate::tui_println!(
                "{}{}{}◎ {}{}ParallelResearch spawning {} subagents{}",
                AURA_SLATE,
                prefix,
                COLOR_RESET,
                AURA_PURPLE,
                COLOR_BOLD,
                tasks_val.len(),
                COLOR_RESET
            );
        }

        let current_depth = DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
        let current_workspace = crate::config::loader::ACTIVE_WORKSPACE
            .try_with(|w| w.clone())
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

        let mut join_handles = Vec::new();

        for (idx, task_val) in tasks_val.iter().enumerate() {
            let goal = match task_val.get("goal").and_then(|v| v.as_str()) {
                Some(g) => g.to_string(),
                None => continue,
            };
            let context = task_val
                .get("context")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let model_override = task_val
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let timeout_secs = super::resolve_subagent_timeout_secs(
                task_val.get("timeout_secs").and_then(|v| v.as_u64()),
                self.config.agents.defaults.tool_timeout_secs,
            );

            let role = get_heuristic_role(idx, &goal);

            if !is_parent_silent {
                let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                crate::tui_println!(
                    "{}{}{}◌ {}{}\u{2014} queued{}",
                    AURA_SLATE,
                    leaf_prefix,
                    AURA_SLATE,
                    COLOR_BOLD,
                    role,
                    COLOR_RESET
                );
            }

            let config = self.config.clone();
            let parent_provider = self.parent_provider.clone();
            let session_manager = self.session_manager.clone();
            let cancellation_token = self.cancellation_token.clone();

            let mut read_only_parent_tools = Vec::new();
            for tool in &self.parent_tools {
                if READ_ONLY_TOOLS.contains(&tool.name()) {
                    read_only_parent_tools.push(tool.clone());
                }
            }

            let current_workspace = current_workspace.clone();
            let role_clone = role.clone();
            let goal_clone = goal.clone();

            let handle = tokio::spawn(async move {
                if !is_parent_silent {
                    let leaf_prefix =
                        crate::agent::style::get_tree_prefix_for_depth(true, current_depth);
                    crate::tui_println!(
                        "{}{}{}● {}{}\u{2014} running...{}",
                        AURA_SLATE,
                        leaf_prefix,
                        RED_ORANGE,
                        COLOR_BOLD,
                        role_clone,
                        COLOR_RESET
                    );
                }

                let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(current_workspace, async {
                    DELEGATION_DEPTH.scope(current_depth + 1, async {
                        let provider = if let Some(ref m) = model_override {
                            match build_provider_for_model(&config, m) {
                                Ok(p) => p,
                                Err(_) => parent_provider.clone()
                            }
                        } else {
                            parent_provider.clone()
                        };

                        let child_registry = ToolRegistry::new_with_context(
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
                            goal_clone, context
                        );

                        crate::agent::style::spinner::IS_SILENT.scope(crate::agent::style::is_silent(), async {
                            tokio::select! {
                                biased;
                                _ = cancellation_token.wait_for_cancellation() => {
                                    Err(anyhow::anyhow!("Task cancelled"))
                                }
                                res = child_agent.run(&subagent_prompt, &child_session_id) => res,
                            }
                        }).await
                    }).await
                });
                let run_res = match tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    run_res_fut,
                )
                .await
                {
                    Ok(res) => res,
                    Err(_) => Err(anyhow::anyhow!(
                        "Parallel research task timed out after {timeout_secs}s"
                    )),
                };

                if !is_parent_silent {
                    let leaf_prefix =
                        crate::agent::style::get_tree_prefix_for_depth(true, current_depth);
                    match run_res {
                        Ok(ref run_res) => {
                            let summary =
                                crate::agent::style::format_subagent_summary(&run_res.content);
                            crate::tui_println!(
                                "{}{}{}{}✓ {}{} \u{2014} {}{}{}",
                                AURA_SLATE,
                                leaf_prefix,
                                AURA_GREEN,
                                COLOR_BOLD,
                                role_clone,
                                COLOR_RESET,
                                AURA_GREEN,
                                summary,
                                COLOR_RESET
                            );
                        }
                        Err(ref e) => {
                            crate::tui_println!(
                                "{}{}{}{}✕ {}{} \u{2014} failed: {}{}{}",
                                AURA_SLATE,
                                leaf_prefix,
                                AURA_ROSE,
                                COLOR_BOLD,
                                role_clone,
                                COLOR_RESET,
                                AURA_ROSE,
                                e,
                                COLOR_RESET
                            );
                        }
                    }
                }
                (goal_clone, run_res)
            });
            join_handles.push(handle);
        }

        // Race join_all against cancellation — spawned tasks self-terminate
        // via their own tokio::select! on the shared CancellationToken.
        let join_results = {
            let all_fut = futures_util::future::join_all(join_handles);
            tokio::select! {
                biased;
                _ = self.cancellation_token.wait_for_cancellation() => {
                    return Err(anyhow!("Parallel research cancelled by user"));
                }
                results = all_fut => results,
            }
        };
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

pub fn get_status_from_goal(goal: &str) -> String {
    let trimmed = goal.trim().to_lowercase();
    if trimmed.starts_with("review ") {
        format!("reviewing {}...", goal.trim()[7..].trim_end_matches('.'))
    } else if trimmed.starts_with("analyze ") {
        format!("analyzing {}...", goal.trim()[8..].trim_end_matches('.'))
    } else if trimmed.starts_with("debug ") {
        format!("debugging {}...", goal.trim()[6..].trim_end_matches('.'))
    } else if trimmed.starts_with("scaffold ") {
        format!("scaffolding {}...", goal.trim()[9..].trim_end_matches('.'))
    } else {
        format!("running {}...", goal.trim().trim_end_matches('.'))
    }
}

pub fn get_heuristic_role(index: usize, goal: &str) -> String {
    let goal_lower = goal.to_lowercase();
    if goal_lower.contains("test") {
        "Tester".to_string()
    } else if goal_lower.contains("debug")
        || goal_lower.contains("error")
        || goal_lower.contains("fail")
    {
        "Debugger".to_string()
    } else if goal_lower.contains("architect")
        || goal_lower.contains("design")
        || goal_lower.contains("structure")
    {
        "Architect".to_string()
    } else if goal_lower.contains("write")
        || goal_lower.contains("code")
        || goal_lower.contains("implement")
    {
        "Developer".to_string()
    } else if goal_lower.contains("review") || goal_lower.contains("audit") {
        "Reviewer".to_string()
    } else {
        match index % 4 {
            0 => "Researcher".to_string(),
            1 => "Debugger".to_string(),
            2 => "Architect".to_string(),
            _ => "Developer".to_string(),
        }
    }
}
