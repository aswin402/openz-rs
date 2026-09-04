//! Resource limits, timeout handling, retries, and result rendering for tools.

use crate::agent::style::*;
use anyhow::Result;

pub(super) fn should_cancel_turn_after_tool_error(error_str: &str) -> bool {
    let lower = error_str.to_lowercase();
    lower.contains("cancelled by user")
        || lower.contains("canceled by user")
        || lower.contains("subagent task cancelled")
}

fn timeout_arg(arguments: &serde_json::Value, key: &str) -> Option<u64> {
    arguments.get(key).and_then(|value| value.as_u64())
}

fn max_parallel_task_timeout(arguments: &serde_json::Value) -> Option<u64> {
    arguments
        .get("tasks")
        .and_then(|value| value.as_array())
        .and_then(|tasks| {
            tasks
                .iter()
                .filter_map(|task| timeout_arg(task, "timeout_secs"))
                .max()
        })
}

pub(super) fn resolve_tool_timeout_secs(
    tool_name: &str,
    arguments: &serde_json::Value,
    recommended_timeout_secs: Option<u64>,
    default_timeout_secs: u64,
) -> u64 {
    if let Some(explicit) = timeout_arg(arguments, "_timeout_secs") {
        return crate::tools::clamp_tool_timeout_secs(explicit);
    }

    let mut timeout_secs = recommended_timeout_secs.unwrap_or(default_timeout_secs);
    if let Some(argument_timeout) = timeout_arg(arguments, "timeout_secs") {
        timeout_secs = timeout_secs.max(argument_timeout);
    }
    if tool_name == "parallel_research" {
        if let Some(task_timeout) = max_parallel_task_timeout(arguments) {
            timeout_secs = timeout_secs.max(task_timeout);
        }
    }

    crate::tools::clamp_tool_timeout_secs(timeout_secs)
}

pub(super) struct ApprovedToolExec<'a> {
    pub(super) tool: std::sync::Arc<dyn crate::tools::Tool>,
    pub(super) call: &'a crate::providers::ToolCallRequest,
    pub(super) metadata: &'a crate::tools::ToolMetadata,
    pub(super) config: &'a crate::config::schema::Config,
    pub(super) formatted_args: &'a str,
    pub(super) session_key: &'a str,
    pub(super) silent: bool,
    pub(super) tool_spinner_msg: &'a str,
    pub(super) turn_cancel: &'a crate::tools::subagent::CancellationToken,
    pub(super) turn_errors: &'a mut Vec<String>,
}

struct ToolExecutionPipeline<'a> {
    params: ApprovedToolExec<'a>,
}

impl<'a> ToolExecutionPipeline<'a> {
    fn new(params: ApprovedToolExec<'a>) -> Self {
        Self { params }
    }

    async fn execute(&mut self) -> serde_json::Value {
        let _process_guard = match self.acquire_process_guard() {
            Ok(guard) => guard,
            Err(reason) => return self.render_process_policy_block(&reason).await,
        };

        let tool_timeout_secs = self.resolve_timeout_secs();
        let mut attempts = 0;
        let max_attempts = 3;
        let mut delay = std::time::Duration::from_secs(1);

        loop {
            attempts += 1;
            match self.run_with_spinner(tool_timeout_secs).await {
                Ok(res) => return self.render_success(res).await,
                Err(err) => {
                    if attempts < max_attempts && is_transient_error(&err) {
                        tracing::warn!(
                            tool = %self.params.call.name,
                            attempt = attempts,
                            error = %err,
                            "Transient tool error encountered. Retrying in {:?}",
                            delay
                        );
                        tokio::time::sleep(delay).await;
                        delay *= 2;
                    } else {
                        return self.render_error(err).await;
                    }
                }
            }
        }
    }

    fn acquire_process_guard(
        &self,
    ) -> Result<Option<crate::tools::resource_policy::ProcessToolGuard>, String> {
        if !self.params.metadata.spawns_process {
            return Ok(None);
        }

        crate::tools::resource_policy::try_acquire_process_tool(
            self.params
                .config
                .agents
                .defaults
                .max_concurrent_process_tools,
        )
        .map(Some)
    }

    fn resolve_timeout_secs(&self) -> u64 {
        let normalized = crate::tools::normalize_tool_args(&self.params.call.arguments);
        resolve_tool_timeout_secs(
            &self.params.call.name,
            &normalized,
            self.params.metadata.recommended_timeout_secs,
            self.params.config.agents.defaults.tool_timeout_secs,
        )
    }

    async fn run_with_spinner(&self, timeout_secs: u64) -> anyhow::Result<serde_json::Value> {
        let tool_timeout = std::time::Duration::from_secs(timeout_secs);
        let normalized = crate::tools::normalize_tool_args(&self.params.call.arguments);
        let fut = self.params.tool.call(&normalized);
        let timed_fut = tokio::time::timeout(tool_timeout, fut);
        let tool_cancel_tx = crate::shutdown::cli_cancel_tx();
        let mut tool_cancel_rx = tool_cancel_tx.subscribe();
        let tool_cancel_initial = *tool_cancel_rx.borrow();

        let cancel_aware_fut = async {
            tokio::select! {
                biased;
                _ = self.params.turn_cancel.wait_for_cancellation() => {
                    Err(anyhow::anyhow!("Cancelled by user"))
                }
                _ = async {
                    while *tool_cancel_rx.borrow() == tool_cancel_initial {
                        if tool_cancel_rx.changed().await.is_err() { break; }
                    }
                } => {
                    Err(anyhow::anyhow!("Cancelled by user"))
                }
                res = timed_fut => {
                    match res {
                        Ok(r) => r,
                        Err(_) => Err(anyhow::anyhow!(
                            "Tool execution timed out after {}s",
                            timeout_secs
                        )),
                    }
                }
            }
        };

        with_spinner(self.params.tool_spinner_msg, cancel_aware_fut).await
    }

    async fn render_process_policy_block(&mut self, reason: &str) -> serde_json::Value {
        let error_str = format!(
            "Tool blocked by resource policy: {}. {}",
            self.params.call.name, reason
        );
        self.params.turn_errors.push(format!(
            "Tool {} blocked by process resource policy: {}",
            self.params.call.name, reason
        ));
        self.render_failure(&error_str).await
    }

    async fn render_success(&self, res: serde_json::Value) -> serde_json::Value {
        crate::agent::agent_loop::tool_execution::render_tool_success(
            self.params.call,
            self.params.formatted_args,
            self.params.session_key,
            self.params.silent,
            res,
        )
        .await
    }

    async fn render_error(&mut self, err: anyhow::Error) -> serde_json::Value {
        let error_str = err.to_string();
        if should_cancel_turn_after_tool_error(&error_str) {
            self.params.turn_cancel.cancel();
        }
        self.params.turn_errors.push(format!(
            "Tool {} failed: {}",
            self.params.call.name, error_str
        ));
        self.render_failure(&error_str).await
    }

    async fn render_failure(&self, error_str: &str) -> serde_json::Value {
        crate::agent::agent_loop::tool_execution::render_tool_failure(
            self.params.call,
            self.params.formatted_args,
            self.params.session_key,
            self.params.silent,
            error_str,
        )
        .await
    }
}

fn is_transient_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    if msg.contains("cancelled by user") || msg.contains("tool execution timed out") {
        return false;
    }
    msg.contains("rate limit")
        || msg.contains("429")
        || msg.contains("too many requests")
        || msg.contains("timeout")
        || msg.contains("timed out")
        || msg.contains("connection")
        || msg.contains("connect")
        || msg.contains("network")
        || msg.contains("dns")
        || msg.contains("host unreachable")
        || msg.contains("temporary failure")
        || msg.contains("502")
        || msg.contains("503")
        || msg.contains("504")
        || msg.contains("bad gateway")
        || msg.contains("service unavailable")
}

pub(super) async fn execute_approved_tool(params: ApprovedToolExec<'_>) -> serde_json::Value {
    ToolExecutionPipeline::new(params).execute().await
}

/// Execute a tool call generated by an automatic follow-up policy.
///
/// These calls are created by the run loop after a user-facing tool completes
/// (for example, opening a generated artifact or suggesting a device). They
/// intentionally bypass the model-facing approval decision because the parent
/// call already established the user intent, but still use the same resource,
/// timeout, retry, cancellation, and rendering pipeline as every approved
/// tool call.
pub(super) async fn execute_auto_tool_call(
    tools: &crate::tools::ToolRegistry,
    call: &crate::providers::ToolCallRequest,
    config: &crate::config::schema::Config,
    session_key: &str,
    silent: bool,
    turn_cancel: &crate::tools::subagent::CancellationToken,
    turn_errors: &mut Vec<String>,
) -> Option<serde_json::Value> {
    let tool = tools.get(&call.name)?;
    let metadata = tool.metadata();
    let formatted_args = crate::agent::agent_loop::tool_execution::format_tool_args(
        &call.name,
        &call.arguments,
    );
    let tool_spinner_msg = crate::agent::style::get_tree_spinner_msg(&call.name, &formatted_args);

    Some(
        execute_approved_tool(ApprovedToolExec {
            tool,
            call,
            metadata: &metadata,
            config,
            formatted_args: &formatted_args,
            session_key,
            silent,
            tool_spinner_msg: &tool_spinner_msg,
            turn_cancel,
            turn_errors,
        })
        .await,
    )
}
