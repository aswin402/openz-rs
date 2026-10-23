use super::{build_provider_for_model, CancellationToken, DELEGATION_DEPTH};
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
use tokio::task::JoinSet;

pub struct ParallelResearchTool {
    pub config: Config,
    pub parent_provider: Arc<dyn LLMProvider>,
    pub session_manager: SessionManager,
    pub parent_tools: Vec<Arc<dyn Tool>>,
    pub cancellation_token: CancellationToken,
    pub capability_policy: Option<CapabilityPolicy>,
}

const PARALLEL_RESEARCH_RESULT_FLUSH_GRACE_SECS: u64 = 5;

const READ_ONLY_TOOLS: &[&str] = &[
    "read_file",
    "find_files",
    "read_doc",
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
    "crawl_website",
    "obscura_browser",
    "recall_memory",
];

#[cfg(test)]
pub(crate) fn read_only_tool_names() -> &'static [&'static str] {
    READ_ONLY_TOOLS
}

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

        match tasks_val.is_empty() {
            true => return Err(anyhow!("The 'tasks' array cannot be empty")),
            false => {}
        }

        let is_parent_silent = crate::agent::style::is_silent();
        match !is_parent_silent {
            true => {
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
            false => {}
        }

        let current_depth = DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
        let current_workspace = crate::config::loader::ACTIVE_WORKSPACE
            .try_with(|w| w.clone())
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

        let mut join_set = JoinSet::new();
        let mut task_goals: Vec<String> = Vec::new();
        let mut max_child_timeout_secs = 1;

        for (idx, task_val) in tasks_val.iter().enumerate() {
            let goal = match super::extract_string_arg(
                task_val,
                &["goal", "task", "prompt", "instruction", "description"],
            ) {
                Some(g) => g,
                None => continue,
            };
            let context = super::extract_string_arg(
                task_val,
                &["context", "background", "details"],
            )
            .unwrap_or_default();
            let model_override = super::extract_string_arg(
                task_val,
                &["model", "model_override"],
            );
            let timeout_secs = super::resolve_subagent_timeout_secs(
                super::extract_u64_arg(task_val, &["timeout_secs", "timeout"]),
                self.config.agents.defaults.tool_timeout_secs,
            );
            max_child_timeout_secs = max_child_timeout_secs.max(timeout_secs);
            task_goals.push(goal.clone());

            let role = get_heuristic_role(idx, &goal);

            match !is_parent_silent {
                true => {
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
                false => {}
            }

            let config = self.config.clone();
            let parent_provider = self.parent_provider.clone();
            let session_manager = self.session_manager.clone();
            let cancellation_token = self.cancellation_token.clone();
            let capability_policy = self.capability_policy.clone();

            let read_only_parent_tools: Vec<Arc<dyn Tool>> = self
                .parent_tools
                .iter()
                .filter(|tool| {
                    READ_ONLY_TOOLS.contains(&tool.name())
                        && capability_policy
                            .as_ref()
                            .map(|policy| {
                                crate::tools::orchestrator::tool_allowed_by_policy(
                                    tool.name(),
                                    policy,
                                )
                            })
                            .unwrap_or(true)
                })
                .cloned()
                .collect();

            let current_workspace = current_workspace.clone();
            let role_clone = role.clone();
            let goal_clone = goal.clone();

            join_set.spawn(async move {
                match !is_parent_silent {
                    true => {
                        let leaf_prefix =
                            crate::agent::style::get_tree_prefix_for_depth(true, current_depth);
                        crate::tui_println!(
                            "{}{}{}• {}{}\u{2014} running...{}",
                            AURA_SLATE,
                            leaf_prefix,
                            RED_ORANGE,
                            COLOR_BOLD,
                            role_clone,
                            COLOR_RESET
                        );
                    }
                    false => {}
                }

                let run_res_fut = crate::config::loader::ACTIVE_WORKSPACE.scope(current_workspace, async {
                    DELEGATION_DEPTH.scope(current_depth + 1, async {
                        let provider = model_override
                            .as_deref()
                            .and_then(|m| build_provider_for_model(&config, m).ok())
                            .unwrap_or_else(|| parent_provider.clone());

                        let child_registry = ToolRegistry::new_with_context(
                            config.clone(),
                            provider.clone(),
                            session_manager.clone(),
                        );
                        child_registry.set_capability_policy(capability_policy.clone());
                        for tool in read_only_parent_tools {
                            child_registry.register(tool);
                        }

                        let child_session_id = format!("subagent:parallel:{}", crate::core::short_uuid());
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

                match !is_parent_silent {
                    true => {
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
                    false => {}
                }
                (idx, goal_clone, run_res)
            });
        }

        let total_tasks = task_goals.len();
        let mut completed = vec![false; total_tasks];
        let mut completed_count = 0;
        let mut results = Vec::new();
        let aggregate_deadline_secs = parallel_research_flush_deadline_secs(max_child_timeout_secs);
        let aggregate_deadline =
            tokio::time::sleep(std::time::Duration::from_secs(aggregate_deadline_secs));
        tokio::pin!(aggregate_deadline);

        while completed_count < total_tasks {
            tokio::select! {
                biased;
                _ = self.cancellation_token.wait_for_cancellation() => {
                    join_set.abort_all();
                    return Err(anyhow!("Parallel research cancelled by user"));
                }
                _ = &mut aggregate_deadline => {
                    join_set.abort_all();
                    for (idx, goal) in task_goals.iter().enumerate() {
                        match !completed[idx] {
                            true => {
                                results.push(parallel_task_error_json(
                                    goal,
                                    "timeout",
                                    &format!(
                                        "Parallel research aggregate deadline reached after {aggregate_deadline_secs}s; unfinished task aborted so completed partial results can be returned before the outer tool timeout."
                                    ),
                                ));
                            }
                            false => {}
                        }
                    }
                    break;
                }
                joined = join_set.join_next() => {
                    match joined {
                        Some(Ok((idx, goal, Ok(run_res)))) => {
                            match idx < completed.len() && !completed[idx] {
                                true => {
                                    completed[idx] = true;
                                    completed_count += 1;
                                }
                                false => {}
                            }
                            results.push(parallel_task_success_json(&goal, &run_res.content));
                        }
                        Some(Ok((idx, goal, Err(e)))) => {
                            match idx < completed.len() && !completed[idx] {
                                true => {
                                    completed[idx] = true;
                                    completed_count += 1;
                                }
                                false => {}
                            }
                            let error = e.to_string();
                            let status = match error.to_lowercase().contains("timed out") {
                                true => "timeout",
                                false => "error",
                            };
                            results.push(parallel_task_error_json(&goal, status, &error));
                        }
                        Some(Err(e)) => {
                            results.push(parallel_task_error_json(
                                "unknown task",
                                "error",
                                &format!("Task join failed: {e:?}"),
                            ));
                        }
                        None => break,
                    }
                }
            }
        }

        Ok(parallel_research_response(results))
    }
}

pub fn parallel_research_flush_deadline_secs(max_child_timeout_secs: u64) -> u64 {
    max_child_timeout_secs
        .saturating_sub(PARALLEL_RESEARCH_RESULT_FLUSH_GRACE_SECS)
        .max(1)
}

fn parallel_task_success_json(task: &str, summary: &str) -> Value {
    serde_json::json!({
        "task": task,
        "status": "success",
        "summary": summary,
    })
}

fn parallel_task_error_json(task: &str, status: &str, error: &str) -> Value {
    serde_json::json!({
        "task": task,
        "status": status,
        "error": error,
    })
}

pub fn parallel_research_response(results: Vec<Value>) -> Value {
    let succeeded = results
        .iter()
        .filter(|result| result.get("status").and_then(|v| v.as_str()) == Some("success"))
        .count();
    let failed = results.len().saturating_sub(succeeded);
    let status = match (failed == 0, succeeded > 0) {
        (true, _) => "success",
        (false, true) => "partial_success",
        (false, false) => "error",
    };

    serde_json::json!({
        "status": status,
        "succeeded": succeeded,
        "failed": failed,
        "results": results,
    })
}

const GOAL_STATUS_PREFIXES: &[(&str, &str)] = &[
    ("review ", "reviewing"),
    ("analyze ", "analyzing"),
    ("debug ", "debugging"),
    ("scaffold ", "scaffolding"),
];

pub fn get_status_from_goal(goal: &str) -> String {
    let raw = goal.trim();
    let lower = raw.to_lowercase();
    GOAL_STATUS_PREFIXES
        .iter()
        .find_map(|(prefix, verb)| {
            lower
                .starts_with(prefix)
                .then(|| format!("{} {}...", verb, raw[prefix.len()..].trim_end_matches('.')))
        })
        .unwrap_or_else(|| format!("running {}...", raw.trim_end_matches('.')))
}

const HEURISTIC_ROLE_RULES: &[(&[&str], &str)] = &[
    (&["test"], "Tester"),
    (&["debug", "error", "fail"], "Debugger"),
    (&["architect", "design", "structure"], "Architect"),
    (&["write", "code", "implement"], "Developer"),
    (&["review", "audit"], "Reviewer"),
];

pub fn get_heuristic_role(index: usize, goal: &str) -> String {
    let goal_lower = goal.to_lowercase();
    HEURISTIC_ROLE_RULES
        .iter()
        .find_map(|(keywords, role)| {
            keywords
                .iter()
                .any(|kw| goal_lower.contains(kw))
                .then(|| (*role).to_string())
        })
        .unwrap_or_else(|| match index % 4 {
            0 => "Researcher".to_string(),
            1 => "Debugger".to_string(),
            2 => "Architect".to_string(),
            _ => "Developer".to_string(),
        })
}

#[cfg(test)]
#[path = "parallel_research_tests.rs"]
mod tests;
