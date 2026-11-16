//! Agent turn execution state machine and orchestration helpers.

use super::{AgentLoop, TurnContext, TurnState};
use crate::agent::style::*;
use crate::providers::GenerationSettings;
use crate::session::Message;
use anyhow::Result;
use futures_util::StreamExt;
use std::io::Write;

mod artifacts;
mod events;
mod research;
mod response;
mod shell_http_rewrite;
#[cfg(test)]
mod tests;
mod tool_pipeline;
mod turn;

use self::artifacts::{
    auto_adjust_tool_call_with_availability,
    auto_device_inventory_suggest_call_after_open_failure, auto_open_artifact_call_after_tool,
    auto_scope_context_before_edit, should_record_auto_scope_result,
};
use self::events::{
    compact_reasoning_summary, normalize_tui_thought_display, publish_auto_capture_notice,
    should_send_public_reasoning_progress, should_show_tui_thoughts, stream_content_chunk,
};
use self::research::{
    direct_research_fetch_key, direct_research_url, fresh_research_brief_blocks_lookup,
    is_research_lookup_tool,
};
use self::response::normalize_response;
use self::tool_pipeline::{
    execute_approved_tool, execute_auto_tool_call, prefetch_parallel_safe_batch, ApprovedToolExec,
};
use self::turn::{acquire_provider_turn_lock, build_iteration_routing_prompt, RunContext};

fn print_stream_reasoning(
    full_reasoning: &str,
    in_reasoning_phase: &mut bool,
    reasoning_printed: &mut bool,
    start_time: std::time::Instant,
    display_mode: &str,
    silent: bool,
) {
    let should_print = !*reasoning_printed
        && !full_reasoning.is_empty()
        && should_show_tui_thoughts(display_mode);
    let _ = should_print.then(|| {
        let elapsed = start_time.elapsed().as_secs_f64();
        crate::channels::ratatui::send_ratatui_turn_event(
            crate::channels::ratatui::TurnEvent::SingleMessage(
                crate::channels::ratatui::app::ChatMessage::thought(
                    full_reasoning.to_string(),
                    Some(elapsed),
                ),
            ),
        );
        let depth = crate::tools::subagent::DELEGATION_DEPTH
            .try_with(|d| *d)
            .unwrap_or(0);
        let _ = (!silent).then(|| {
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
            let elapsed_f32 = start_time.elapsed().as_secs_f32();
            let prefix = (depth > 0)
                .then(|| crate::agent::style::get_tree_prefix(false))
                .unwrap_or_default();
            crate::tui_println!(
                "{}{}• {}{}{}Thought for {:.1}s{}",
                prefix,
                RED_ORANGE,
                COLOR_RESET,
                COLOR_BOLD,
                RED_ORANGE,
                elapsed_f32,
                COLOR_RESET
            );
            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
            let visible_reasoning = match display_mode == "compact" {
                true => compact_reasoning_summary(full_reasoning),
                false => full_reasoning.to_string(),
            };
            crate::agent::style::print_tree_monologue(&leaf_prefix, &visible_reasoning);
            crate::tui_println!("");
        });
        *reasoning_printed = true;
        *in_reasoning_phase = false;
    });
}

#[allow(clippy::too_many_arguments)]
async fn stream_provider_response(
    loop_ref: &AgentLoop,
    ctx: &mut TurnContext<'_>,
    config: &crate::config::schema::Config,
    tools_openai: &[serde_json::Value],
    settings: &GenerationSettings,
    activity_msg: &str,
    turn_cancel_clone: &crate::tools::subagent::CancellationToken,
    start_time: std::time::Instant,
    content_streaming_started: &mut bool,
    reasoning_printed: &mut bool,
    markdown_streamer: &mut crate::agent::style::StreamingMarkdownRenderer,
) -> Result<crate::providers::LLMResponse> {
    let mut stream = loop_ref
        .chat_stream_with_fallback(
            &mut ctx.active_provider,
            &ctx.system_prompt,
            &ctx.messages,
            tools_openai,
            settings,
            activity_msg,
        )
        .await?;
    let silent = crate::agent::style::spinner::is_silent();
    let tui_thought_display =
        normalize_tui_thought_display(&config.agents.defaults.tui_thought_display);

    let mut full_content = String::new();
    let mut full_reasoning = String::new();
    let mut in_reasoning_phase = false;
    let mut tool_call_stream_started = false;
    let mut streaming_assembly = super::streaming::StreamingAssembly::new();

    let stream_cancel_tx = crate::shutdown::cli_cancel_tx();
    let mut stream_cancel_rx = stream_cancel_tx.subscribe();
    let stream_cancel_initial = *stream_cancel_rx.borrow();
    let estimated_tokens = crate::providers::DynamicContextRegistry::estimate_request_tokens(
        &ctx.messages,
        &ctx.system_prompt,
        tools_openai,
    );
    let prefill_timeout = std::time::Duration::from_secs(
        loop_ref.provider_prefill_timeout_secs(estimated_tokens),
    );
    let stream_idle_timeout = loop_ref.provider_attempt_timeout_duration();
    let mut received_any_chunk = false;

    loop {
        let active_timeout = match received_any_chunk {
            true => stream_idle_timeout,
            false => prefill_timeout,
        };
        let chunk = tokio::select! {
            biased;
            _ = turn_cancel_clone.wait_for_cancellation() => {
                return Err(anyhow::anyhow!("Cancelled by user"));
            }
            _ = async {
                while *stream_cancel_rx.borrow() == stream_cancel_initial
                    && stream_cancel_rx.changed().await.is_ok()
                {}
            } => {
                return Err(anyhow::anyhow!("Cancelled by user"));
            }
            next = tokio::time::timeout(active_timeout, stream.next()) => {
                match next {
                    Ok(Some(c)) => {
                        received_any_chunk = true;
                        c
                    }
                    Ok(None) => break,
                    Err(_) => {
                        return Err(anyhow::anyhow!(
                            "Provider stream timed out after {}s without output",
                            active_timeout.as_secs()
                        ));
                    }
                }
            }
        };
        match chunk? {
            crate::providers::ChatStreamChunk::Content(text) => {
                print_stream_reasoning(
                    &full_reasoning,
                    &mut in_reasoning_phase,
                    reasoning_printed,
                    start_time,
                    tui_thought_display,
                    silent,
                );

                let _ = (in_reasoning_phase && !silent).then(|| {
                    print!("\r\x1b[2K");
                    let _ = std::io::stdout().flush();
                    in_reasoning_phase = false;
                });
                let _ = (!*content_streaming_started).then(|| {
                    crate::channels::ratatui::send_ratatui_turn_event(
                        crate::channels::ratatui::TurnEvent::Activity(
                            crate::channels::ratatui::animation::AgentActivity::Generating,
                        ),
                    );
                });
                full_content.push_str(&text);
                stream_content_chunk(&text, markdown_streamer, content_streaming_started);
                let _ = crate::channels::websocket::ws_chat_id(ctx.session_key).map(|chat_id| {
                    crate::channels::websocket::publish_ws_event(
                        crate::channels::websocket::protocol::delta(
                            chat_id,
                            super::current_turn_id(),
                            text.clone(),
                        ),
                    );
                });
                streaming_assembly.push_chunk(crate::providers::ChatStreamChunk::Content(text));
            }
            crate::providers::ChatStreamChunk::Reasoning(text) => {
                let _ = (!in_reasoning_phase).then(|| {
                    crate::channels::ratatui::send_ratatui_turn_event(
                        crate::channels::ratatui::TurnEvent::Activity(
                            crate::channels::ratatui::animation::AgentActivity::Planning,
                        ),
                    );
                });
                full_reasoning.push_str(&text);
                let _ = crate::channels::websocket::ws_chat_id(ctx.session_key).map(|chat_id| {
                    crate::channels::websocket::publish_ws_event(
                        crate::channels::websocket::protocol::reasoning_delta(
                            chat_id,
                            super::current_turn_id(),
                            text.clone(),
                        ),
                    );
                });
                in_reasoning_phase = true;
                let _ = (!silent && should_show_tui_thoughts(tui_thought_display)).then(|| {
                    let elapsed = start_time.elapsed().as_secs_f32();
                    print!(
                        "\r\x1b[2K{}{}▶ Thinking... {:.1}s{}",
                        COLOR_BOLD, RED_ORANGE, elapsed, COLOR_RESET
                    );
                    let _ = std::io::stdout().flush();
                });
                streaming_assembly.push_chunk(crate::providers::ChatStreamChunk::Reasoning(text));
            }
            crate::providers::ChatStreamChunk::ToolCall {
                index,
                id,
                name,
                arguments,
            } => {
                tool_call_stream_started = true;
                print_stream_reasoning(
                    &full_reasoning,
                    &mut in_reasoning_phase,
                    reasoning_printed,
                    start_time,
                    tui_thought_display,
                    silent,
                );
                let _ = (in_reasoning_phase && !silent).then(|| {
                    print!("\r\x1b[2K");
                    let _ = std::io::stdout().flush();
                    in_reasoning_phase = false;
                });
                streaming_assembly.push_chunk(crate::providers::ChatStreamChunk::ToolCall {
                    index,
                    id,
                    name,
                    arguments,
                });
            }
            crate::providers::ChatStreamChunk::Done {
                finish_reason: reason,
            } => {
                streaming_assembly.push_chunk(crate::providers::ChatStreamChunk::Done {
                    finish_reason: reason,
                });
            }
        }
    }

    let _ = (in_reasoning_phase && !silent).then(|| {
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
        in_reasoning_phase = false;
    });

    let should_finalize_thoughts = should_show_tui_thoughts(tui_thought_display)
        && (!full_content.trim().is_empty() || tool_call_stream_started);
    let _ = should_finalize_thoughts.then(|| {
        print_stream_reasoning(
            &full_reasoning,
            &mut in_reasoning_phase,
            reasoning_printed,
            start_time,
            tui_thought_display,
            silent,
        );
    });

    markdown_streamer.finish();
    ctx.streamed = !full_content.is_empty();

    let mut assembled = streaming_assembly.into_response();
    let _ = assembled.tool_calls.is_empty().then(|| {
        assembled.content = (!full_content.is_empty()).then_some(full_content);
    });
    assembled.reasoning_content = (!full_reasoning.is_empty()).then_some(full_reasoning);
    Ok(assembled)
}

fn select_reasoning_source(
    resp: &crate::providers::LLMResponse,
    has_reasoning: bool,
    allow_content_fallback: bool,
) -> Option<String> {
    has_reasoning
        .then(|| resp.reasoning_content.clone())
        .flatten()
        .or_else(|| allow_content_fallback.then(|| resp.content.clone()).flatten())
}

async fn emit_reasoning_and_thoughts(
    ctx: &TurnContext<'_>,
    config: &crate::config::schema::Config,
    resp: &crate::providers::LLMResponse,
    duration_secs: f32,
    content_streaming_started: bool,
    reasoning_printed: bool,
) {
    let has_reasoning = resp
        .reasoning_content
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_content = resp.content.as_ref().is_some_and(|s| !s.trim().is_empty());
    let has_tool_calls = !resp.tool_calls.is_empty();

    match has_reasoning || (has_content && has_tool_calls) {
        false => {}
        true => {
            let output_visibility = crate::agent::events::OutputVisibility::from_tui_trace_mode(
                &config.agents.defaults.tui_thought_display,
            );
            let public_reasoning_progress =
                should_send_public_reasoning_progress(&config.agents.defaults.tui_thought_display);

            let candidate_reasoning = public_reasoning_progress
                .then(|| select_reasoning_source(resp, has_reasoning, has_content && has_tool_calls))
                .flatten();
            match candidate_reasoning.and_then(|text| {
                crate::agent::events::AgentEvent::PrivateReasoning(text)
                    .public_text(&output_visibility)
            }) {
                Some(msg) => {
                    super::tool_execution::send_progress_update(ctx.session_key, &msg).await;
                }
                None => {}
            }

            let silent = crate::agent::style::spinner::is_silent();
            let depth = crate::tools::subagent::DELEGATION_DEPTH
                .try_with(|d| *d)
                .unwrap_or(0);
            let _ = (!silent
                && should_show_tui_thoughts(&config.agents.defaults.tui_thought_display))
            .then(|| {
                let prefix = (depth > 0)
                    .then(|| crate::agent::style::get_tree_prefix(false))
                    .unwrap_or_default();
                match ctx.streamed {
                    true => {
                        let _ = (!content_streaming_started && !reasoning_printed).then(|| {
                            print!("\r\x1b[2K");
                            let _ = std::io::stdout().flush();
                            crate::tui_println!(
                                "{}{}• {}{}{}Thought for {:.1}s{}",
                                prefix,
                                RED_ORANGE,
                                COLOR_RESET,
                                COLOR_BOLD,
                                RED_ORANGE,
                                duration_secs,
                                COLOR_RESET
                            );
                        });
                    }
                    false => {
                        crate::tui_println!(
                            "{}{}• {}{}{}Thought for {:.1}s{}",
                            prefix,
                            RED_ORANGE,
                            COLOR_RESET,
                            COLOR_BOLD,
                            RED_ORANGE,
                            duration_secs,
                            COLOR_RESET
                        );
                        let full_reasoning = select_reasoning_source(
                            resp,
                            has_reasoning,
                            public_reasoning_progress && has_content && has_tool_calls,
                        )
                        .unwrap_or_default();
                        let _ = (!full_reasoning.is_empty()).then(|| {
                            crate::channels::ratatui::send_ratatui_turn_event(
                                crate::channels::ratatui::TurnEvent::SingleMessage(
                                    crate::channels::ratatui::app::ChatMessage::thought(
                                        full_reasoning.clone(),
                                        Some(duration_secs as f64),
                                    ),
                                ),
                            );
                            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                            crate::agent::style::print_tree_monologue(
                                &leaf_prefix,
                                &full_reasoning,
                            );
                            print!("\r\n");
                        });
                        let _ = std::io::stdout().flush();
                    }
                }
                print!("\r\n");
                let _ = std::io::stdout().flush();
            });
        }
    }
}

async fn emit_skipped_tool_notice(session_key: &str, silent: bool, skip_msg: &str) -> serde_json::Value {
    super::tool_execution::send_progress_update(session_key, skip_msg).await;
    let _ = (!silent).then(|| {
        let leaf_prefix = crate::agent::style::get_tree_prefix(true);
        crate::tui_println!(
            "{}{}{}↳ {}{}",
            AURA_SLATE,
            leaf_prefix,
            AURA_BLUE,
            skip_msg,
            COLOR_RESET
        );
    });
    serde_json::json!({ "status": "skipped", "reason": skip_msg })
}

fn research_lookup_skip_reason(
    out_of_scope_direct_page_lookup: bool,
    duplicate_direct_url: bool,
    fresh_brief_blocks_lookup: bool,
) -> Option<&'static str> {
    [
        (
            out_of_scope_direct_page_lookup,
            "Skipped broader research: this request supplied one direct URL. Use the fetched page, or ask for deeper/broader research to inspect additional sources.",
        ),
        (
            duplicate_direct_url,
            "Skipped duplicate URL lookup: this page was already fetched successfully in this turn. Use the previous result or continue with a different source.",
        ),
        (
            fresh_brief_blocks_lookup,
            "Skipped web/search lookup: a fresh saved research brief already matches this non-latest query. Answer from [Relevant Research Briefs] unless the user asks for latest/current data.",
        ),
    ]
    .into_iter()
    .find_map(|(blocked, reason)| blocked.then_some(reason))
}

#[derive(Clone, Copy)]
enum ApprovalGateOutcome {
    LoopBlocked,
    Forbidden,
    UserDenied,
    Approved,
}

fn classify_approval_gate(
    approval: &super::security_approval::ToolApprovalDecision,
) -> ApprovalGateOutcome {
    [
        (approval.is_loop, ApprovalGateOutcome::LoopBlocked),
        (approval.forbidden, ApprovalGateOutcome::Forbidden),
        (!approval.approved, ApprovalGateOutcome::UserDenied),
    ]
    .into_iter()
    .find_map(|(matched, outcome)| matched.then_some(outcome))
    .unwrap_or(ApprovalGateOutcome::Approved)
}

async fn execute_scoped_approved_tool(
    loop_ref: &AgentLoop,
    exec: ApprovedToolExec<'_>,
    scope_sticky: bool,
) -> serde_json::Value {
    match scope_sticky {
        true => {
            crate::tools::subagent::ACTIVE_PARENT_STICKY_TOOLS
                .scope(
                    loop_ref.tools.current_sticky_tools(),
                    execute_approved_tool(exec),
                )
                .await
        }
        false => execute_approved_tool(exec).await,
    }
}

#[allow(clippy::too_many_arguments)]
async fn dispatch_tool_call(
    loop_ref: &AgentLoop,
    ctx: &mut TurnContext<'_>,
    run: &mut RunContext,
    config: &crate::config::schema::Config,
    call: &crate::providers::ToolCallRequest,
    formatted_args: &str,
    tool_spinner_msg: &str,
    silent: bool,
    turn_cancel: &crate::tools::subagent::CancellationToken,
    prefetched_outputs: &mut std::collections::HashMap<String, serde_json::Value>,
    approval: &super::security_approval::ToolApprovalDecision,
    out_of_scope_direct_page_lookup: bool,
    duplicate_direct_url: bool,
    fresh_brief_blocks_lookup: bool,
) -> serde_json::Value {
    match research_lookup_skip_reason(
        out_of_scope_direct_page_lookup,
        duplicate_direct_url,
        fresh_brief_blocks_lookup,
    ) {
        Some(skip_msg) => {
            return emit_skipped_tool_notice(ctx.session_key, silent, skip_msg).await;
        }
        None => {}
    }

    match approval.parse_error.as_deref() {
        Some(err_msg) => {
            let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, err_msg);
            super::tool_execution::send_progress_update(ctx.session_key, &fail_msg).await;
            let _ = (!silent).then(|| {
                let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                crate::tui_println!(
                    "{}{}{}✕ {} - Failed: {}{}",
                    AURA_SLATE,
                    leaf_prefix,
                    AURA_ROSE,
                    formatted_args,
                    err_msg,
                    COLOR_RESET
                );
            });
            ctx.turn_errors.push(format!(
                "Tool {} arguments parse error: {}",
                call.name, err_msg
            ));
            return serde_json::json!({ "error": err_msg });
        }
        None => {}
    }

    match classify_approval_gate(approval) {
        ApprovalGateOutcome::LoopBlocked => {
            let warning_str = approval.loop_message.clone().unwrap_or_else(|| {
                format!(
                    "Loop detected: You have already executed the tool '{}' with these exact arguments {} times in this turn. To prevent infinite loops, execution was blocked. Do NOT call this tool again. Analyze previous tool outputs and use a different strategy, or finish your response.",
                    call.name, approval.repeat_count
                )
            });
            let _ = (!silent).then(|| {
                let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                crate::tui_println!(
                    "{}{}{}↶ Loop detected for tool '{}'! Steering away. (Streak: {}, Total: {}){}",
                    AURA_SLATE,
                    leaf_prefix,
                    AURA_GOLD,
                    call.name,
                    run.loop_state.consecutive_blocks,
                    run.loop_state.total_blocks,
                    COLOR_RESET
                );
            });
            tracing::warn!(
                session = %ctx.session_key,
                tool = %call.name,
                streak = run.loop_state.consecutive_blocks,
                total_blocks = run.loop_state.total_blocks,
                "Tool execution blocked (repetition/cycle detected)"
            );
            let cycle_period = approval
                .loop_cycle
                .as_ref()
                .map(|c| c.period)
                .unwrap_or(1);
            let repeated_steps = approval
                .loop_cycle
                .as_ref()
                .map(|c| c.cycle_tools.clone())
                .unwrap_or_else(|| vec![call.name.clone()]);
            serde_json::json!({
                "error": warning_str,
                "stage": "steer",
                "cycle_period": cycle_period,
                "repeated_steps": repeated_steps,
                "consecutive_stalls": run.loop_state.consecutive_blocks,
                "required_action": "change_strategy_or_parameters"
            })
        }
        ApprovalGateOutcome::Forbidden => {
            let reason_str = approval.forbidden_reason.unwrap_or(
                "Execution denied by host: This command is forbidden by security rules.",
            );
            let reject_msg = format!(
                "✕ *{}* - Rejected: {}",
                formatted_args, reason_str
            );
            super::tool_execution::send_progress_update(ctx.session_key, &reject_msg).await;
            let _ = (!silent).then(|| {
                let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                crate::tui_println!(
                    "{}{}{}✕ {} - Rejected: {}{}",
                    AURA_SLATE,
                    leaf_prefix,
                    ERROR_RED,
                    formatted_args,
                    reason_str,
                    COLOR_RESET
                );
            });
            tracing::warn!(
                session = %ctx.session_key,
                tool = %call.name,
                reason = reason_str,
                "Tool execution forbidden by security guard"
            );
            serde_json::json!({ "error": reason_str })
        }
        ApprovalGateOutcome::UserDenied => {
            let error_msg = match approval.unattended_denied {
                true => format!(
                    "Execution denied by autonomous background policy: sensitive action '{}' requires an interactive session; pivot to a workspace-scoped or read-only approach.",
                    call.name
                ),
                false => "Execution denied by user.".to_string(),
            };
            let deny_msg = format!("✕ *{}* - {}", formatted_args, error_msg);
            super::tool_execution::send_progress_update(ctx.session_key, &deny_msg).await;
            let _ = (!silent).then(|| {
                let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                crate::tui_println!(
                    "{}{}{}✕ {} - {}{}",
                    AURA_SLATE,
                    leaf_prefix,
                    ERROR_RED,
                    formatted_args,
                    error_msg,
                    COLOR_RESET
                );
            });
            tracing::warn!(
                session = %ctx.session_key,
                tool = %call.name,
                unattended = approval.unattended_denied,
                "Tool execution denied by approval gate"
            );
            serde_json::json!({ "error": error_msg })
        }
        ApprovalGateOutcome::Approved => match loop_ref.tools.get(&call.name) {
            None => {
                ctx.turn_errors
                    .push(format!("Tool {} not found", call.name));
                super::tool_execution::render_tool_not_found(
                    call,
                    formatted_args,
                    ctx.session_key,
                    silent,
                )
                .await
            }
            Some(t) => {
                let metadata = t.metadata();
                let _ = metadata.spawns_process.then_some(());
                match metadata.spawns_process {
                    true => {
                        crate::tools::resource_policy::wait_for_process_slot(
                            config.agents.defaults.max_concurrent_process_tools,
                            std::time::Duration::from_secs(30),
                            turn_cancel,
                        )
                        .await;
                    }
                    false => {}
                }
                let runtime = crate::tools::resource_policy::RuntimeResourceSnapshot::current();
                let policy_decision = crate::tools::resource_policy::ToolResourcePolicy::evaluate(
                    &metadata,
                    &config.agents.defaults,
                    &runtime,
                );

                match policy_decision {
                    crate::tools::resource_policy::ToolResourceDecision::Block { reason } => {
                        let is_low_disk_recovery =
                            crate::tools::resource_policy::is_free_disk_block(&reason)
                                && crate::tools::resource_policy::is_low_disk_recovery_tool(
                                    &call.name,
                                    &call.arguments,
                                );
                        match is_low_disk_recovery {
                            true => {
                                execute_scoped_approved_tool(
                                    loop_ref,
                                    ApprovedToolExec {
                                        tool: t,
                                        call,
                                        metadata: &metadata,
                                        config,
                                        formatted_args,
                                        session_key: ctx.session_key,
                                        silent,
                                        tool_spinner_msg,
                                        turn_cancel,
                                        turn_errors: &mut ctx.turn_errors,
                                        prefetched_output: prefetched_outputs.remove(&call.id),
                                    },
                                    false,
                                )
                                .await
                            }
                            false => {
                                let error_str = format!(
                                    "Tool blocked by resource policy: {}. {}",
                                    call.name, reason
                                );
                                ctx.turn_errors.push(format!(
                                    "Tool {} blocked by resource policy: {}",
                                    call.name, reason
                                ));
                                super::tool_execution::render_tool_failure(
                                    call,
                                    formatted_args,
                                    ctx.session_key,
                                    silent,
                                    &error_str,
                                )
                                .await
                            }
                        }
                    }
                    crate::tools::resource_policy::ToolResourceDecision::RequireApproval {
                        reason,
                    } => {
                        let already_cleared_by_guard = approval.approval_requested
                            || (reason == "high risk tool requires approval"
                                && crate::agent::security::SecurityGuard::has_granular_argument_policy(
                                    &call.name,
                                ));
                        let approved = match already_cleared_by_guard {
                            true => true,
                            false => crate::agent::security::ask_approval(
                                ctx.session_key,
                                &call.name,
                                &serde_json::json!({
                                    "resource_policy_reason": reason,
                                    "arguments": call.arguments,
                                }),
                            )
                            .await
                            .unwrap_or(false),
                        };
                        match approved {
                            false => {
                                let error_str = format!(
                                    "Tool denied by resource policy approval: {}. {}",
                                    call.name, reason
                                );
                                ctx.turn_errors.push(format!(
                                    "Tool {} denied by resource policy approval: {}",
                                    call.name, reason
                                ));
                                super::tool_execution::render_tool_failure(
                                    call,
                                    formatted_args,
                                    ctx.session_key,
                                    silent,
                                    &error_str,
                                )
                                .await
                            }
                            true => {
                                execute_scoped_approved_tool(
                                    loop_ref,
                                    ApprovedToolExec {
                                        tool: t.clone(),
                                        call,
                                        metadata: &metadata,
                                        config,
                                        formatted_args,
                                        session_key: ctx.session_key,
                                        silent,
                                        tool_spinner_msg,
                                        turn_cancel,
                                        turn_errors: &mut ctx.turn_errors,
                                        prefetched_output: prefetched_outputs.remove(&call.id),
                                    },
                                    true,
                                )
                                .await
                            }
                        }
                    }
                    crate::tools::resource_policy::ToolResourceDecision::Allow => {
                        execute_scoped_approved_tool(
                            loop_ref,
                            ApprovedToolExec {
                                tool: t.clone(),
                                call,
                                metadata: &metadata,
                                config,
                                formatted_args,
                                session_key: ctx.session_key,
                                silent,
                                tool_spinner_msg,
                                turn_cancel,
                                turn_errors: &mut ctx.turn_errors,
                                prefetched_output: prefetched_outputs.remove(&call.id),
                            },
                            true,
                        )
                        .await
                    }
                }
            }
        },
    }
}

async fn emit_turn_halt_message(ctx: &mut TurnContext<'_>, icon: &str, msg: &str) {
    ctx.final_content = msg.to_string();
    ctx.streamed = false;
    super::tool_execution::send_progress_update(ctx.session_key, msg).await;
    let _ = (!crate::agent::style::spinner::is_silent()).then(|| {
        crate::tui_println!("{}{} {}{}", AURA_GOLD, icon, msg, COLOR_RESET);
    });
    ctx.messages.push(Message {
        role: "assistant".to_string(),
        content: msg.to_string(),
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra: serde_json::Map::new(),
    });
}

/// Perform a final tool-free LLM synthesis pass (`tools = &[]`) when a long-horizon
/// turn reaches `max_iterations` or loop-escalation saturation so all completed work,
/// findings, and next steps are delivered to the user instead of a bare warning stub.
async fn synthesize_turn_checkpoint(
    loop_ref: &AgentLoop,
    ctx: &mut TurnContext<'_>,
    settings: &GenerationSettings,
    turn_cancel: &crate::tools::subagent::CancellationToken,
    notice: &str,
) {
    match turn_cancel.is_cancelled() {
        true => {
            emit_turn_halt_message(ctx, "⚠️", notice).await;
            return;
        }
        false => {}
    }

    let mut synthesis_messages = ctx.messages.clone();
    synthesis_messages.push(Message {
        role: "user".to_string(),
        content: format!(
            "[System Checkpoint: {notice}] Do NOT call any tools. Synthesize a concise, structured summary of: (1) what was completed and verified so far, (2) files/resources inspected or modified, and (3) any remaining steps or blockers."
        ),
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra: serde_json::Map::new(),
    });

    let activity_msg = format!("{}▶ Synthesizing checkpoint summary{}", RED_ORANGE, COLOR_RESET);
    let synthesized = loop_ref
        .chat_with_fallback(
            &mut ctx.active_provider,
            &ctx.system_prompt,
            &synthesis_messages,
            &[],
            settings,
            &activity_msg,
        )
        .await
        .ok()
        .and_then(|resp| {
            let (clean, _) = crate::providers::openai::split_think_blocks(
                resp.content.as_deref().unwrap_or(""),
            );
            clean.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
        });

    match synthesized {
        Some(summary) => {
            let combined = format!("{notice}\n\n{summary}");
            ctx.final_content = combined.clone();
            ctx.streamed = false;
            super::tool_execution::send_progress_update(ctx.session_key, notice).await;
            let _ = (!crate::agent::style::spinner::is_silent()).then(|| {
                crate::tui_println!("{}⚠️ {}{}", AURA_GOLD, notice, COLOR_RESET);
            });
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: combined,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
        }
        None => {
            emit_turn_halt_message(ctx, "⚠️", notice).await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_and_record_optional_auto_tool(
    tools: &crate::tools::ToolRegistry,
    maybe_call: Option<crate::providers::ToolCallRequest>,
    config: &crate::config::schema::Config,
    session_key: &str,
    silent: bool,
    turn_cancel: &crate::tools::subagent::CancellationToken,
    turn_errors: &mut Vec<String>,
    tool_results: &mut Vec<super::transcript::ToolTranscriptResult>,
    assistant_tool_calls_json: &mut Vec<serde_json::Value>,
) {
    match maybe_call {
        Some(auto_call) => {
            let _ = execute_auto_tool_call(
                tools,
                &auto_call,
                config,
                session_key,
                silent,
                turn_cancel,
                turn_errors,
            )
            .await
            .map(|result| {
                super::transcript::append_auto_tool_result(
                    tool_results,
                    assistant_tool_calls_json,
                    &auto_call,
                    result,
                );
            });
        }
        None => {}
    }
}

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    match crate::agent::marketplace_intent::clarification_question_for_marketplace_intent(
        ctx.user_content,
    ) {
        Some(question) => {
            ctx.final_content = question.to_string();
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: question.to_string(),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
            return Ok(TurnState::Save);
        }
        None => {}
    }

    let mut run = RunContext::new(ctx);
    let _provider_turn_lock = acquire_provider_turn_lock(
        ctx.session_key,
        &ctx.config.agents.defaults.provider,
        &ctx.config.agents.defaults.model,
    )
    .await?;

    let turn_cancel = run.turn_cancel.clone();
    let turn_cancel_clone = turn_cancel.clone();
    let mut overflow_retries_used: usize = 0;

    loop {
        match run.turn_cancel.is_cancelled() {
            true => {
                emit_turn_halt_message(ctx, "▲", "Turn cancelled by user.").await;
                break;
            }
            false => {}
        }

        let config = ctx.config.clone();
        let settings = GenerationSettings {
            temperature: config.agents.defaults.temperature,
            max_tokens: config.agents.defaults.max_tokens,
            reasoning_effort: None,
        };

        tracing::info!(
            session = %ctx.session_key,
            iteration = run.iterations,
            "Sending completion request to LLM (model: {})",
            config.agents.defaults.model
        );
        match run.iterations >= run.max_iterations {
            true => {
                let msg = format!(
                    "⚠️ Reached tool iteration limit ({}). Summarizing work so far.",
                    run.max_iterations
                );
                synthesize_turn_checkpoint(loop_ref, ctx, &settings, &turn_cancel, &msg).await;
                break;
            }
            false => {}
        }

        let routing_prompt = build_iteration_routing_prompt(
            ctx.user_content,
            &ctx.messages,
            &ctx.tools_used,
            &ctx.turn_errors,
            run.iterations,
        );
        let routing_budgets = config
            .agents
            .defaults
            .layered_tool_routing
            .resolve_budgets(&config.agents.defaults.model);
        let (pending_tools, _) = loop_ref.tools.current_turn_scope();
        let mut explicit_mount_set: std::collections::HashSet<String> =
            pending_tools.into_iter().collect();
        let _ = (run.iterations > 0).then(|| {
            explicit_mount_set.extend(
                loop_ref
                    .tools
                    .explicitly_requested_for_prompt(&routing_prompt),
            );
        });
        let fresh_tools_openai = loop_ref
            .tools
            .to_openai_format_for_prompt_with_config(&routing_prompt, Some(&config));
        let tools_openai = run.tool_window.resolve_iteration_tools_for_step(
            run.iterations,
            fresh_tools_openai,
            &explicit_mount_set,
            routing_budgets.preserve_prefix_cache,
            routing_budgets.max_expanded_tools,
            routing_budgets.max_provider_tools,
        );
        let _ = (config.agents.defaults.show_tool_router_status
            && !crate::agent::style::spinner::is_silent())
        .then(|| {
            let summary = loop_ref
                .tools
                .tool_router_status_line_with_config(&routing_prompt, Some(&config));
            crate::tui_println!("{}◎ {}{}", AURA_PURPLE, summary, COLOR_RESET);
        });

        // Mid-turn proactive context pressure compaction across long multi-tool loops
        match run.iterations > 0 && ctx.messages.len() > 6 {
            true => {
                let req_tokens =
                    crate::providers::DynamicContextRegistry::estimate_request_tokens(
                        &ctx.messages,
                        &ctx.system_prompt,
                        &tools_openai,
                    );
                let pressured =
                    crate::providers::DynamicContextRegistry::should_compact_context(
                        &config.agents.defaults.model,
                        req_tokens,
                        &config,
                    );
                match pressured {
                    true => {
                        ctx.session.messages = ctx.messages.clone();
                        let _ = super::compact::compact_session_history(loop_ref, ctx, false).await;
                    }
                    false => {}
                }
            }
            false => {}
        }

        let activity_msg = format!("{}▶ Thinking{}", RED_ORANGE, COLOR_RESET);
        let start_time = std::time::Instant::now();
        let mut content_streaming_started = false;
        let mut reasoning_printed = false;
        let mut markdown_streamer =
            crate::agent::style::StreamingMarkdownRenderer::new(
                crate::agent::style::spinner::is_silent(),
            );

        let llm_outcome = match config.agents.defaults.streaming {
            true => {
                stream_provider_response(
                    loop_ref,
                    ctx,
                    &config,
                    &tools_openai,
                    &settings,
                    &activity_msg,
                    &turn_cancel_clone,
                    start_time,
                    &mut content_streaming_started,
                    &mut reasoning_printed,
                    &mut markdown_streamer,
                )
                .await
            }
            false => {
                let ns_cancel_tx = crate::shutdown::cli_cancel_tx();
                let mut ns_cancel_rx = ns_cancel_tx.subscribe();
                let ns_cancel_initial = *ns_cancel_rx.borrow();
                let chat_fut = loop_ref.chat_with_fallback(
                    &mut ctx.active_provider,
                    &ctx.system_prompt,
                    &ctx.messages,
                    &tools_openai,
                    &settings,
                    &activity_msg,
                );
                tokio::select! {
                    biased;
                    _ = turn_cancel_clone.wait_for_cancellation() => {
                        turn_cancel_clone.cancel();
                        let msg = "LLM request cancelled by user.".to_string();
                        ctx.final_content = msg;
                        return Ok(TurnState::Save);
                    }
                    _ = async {
                        while *ns_cancel_rx.borrow() == ns_cancel_initial
                            && ns_cancel_rx.changed().await.is_ok()
                        {}
                    } => {
                        turn_cancel_clone.cancel();
                        let msg = "LLM request cancelled by user.".to_string();
                        ctx.final_content = msg;
                        return Ok(TurnState::Save);
                    }
                    res = chat_fut => res,
                }
            }
        };

        let mut resp = match llm_outcome {
            Ok(r) => r,
            Err(err) => {
                let err_str = err.to_string();
                let can_recover = overflow_retries_used == 0
                    && ctx.messages.len() > 4
                    && crate::providers::DynamicContextRegistry::is_context_overflow_error(
                        &err_str,
                    );
                match can_recover {
                    false => return Err(err),
                    true => {
                        overflow_retries_used += 1;
                        let learned = crate::providers::DynamicContextRegistry::learn_context_limit_from_error(
                            &config.agents.defaults.model,
                            &err_str,
                        );
                        tracing::warn!(
                            session = %ctx.session_key,
                            model = %config.agents.defaults.model,
                            learned_limit = ?learned,
                            "Provider reported context overflow; self-healing via emergency compaction"
                        );
                        ctx.session.messages = ctx.messages.clone();
                        let _ = super::compact::compact_session_history(loop_ref, ctx, true).await;
                        continue;
                    }
                }
            }
        };

        resp = normalize_response(
            loop_ref,
            ctx,
            resp,
            &config,
            &tools_openai,
            &settings,
            &turn_cancel_clone,
            start_time,
            &mut reasoning_printed,
            &mut content_streaming_started,
            &mut markdown_streamer,
        )
        .await?;

        let duration = start_time.elapsed();
        tracing::info!(
            session = %ctx.session_key,
            duration_ms = duration.as_millis(),
            has_content = resp.content.is_some(),
            has_reasoning = resp.reasoning_content.is_some(),
            tool_calls = resp.tool_calls.len(),
            "Received LLM response (finish_reason: {})",
            resp.finish_reason
        );
        let _ = resp.reasoning_content.as_ref().map(|reasoning| {
            tracing::debug!(session = %ctx.session_key, "LLM reasoning content: {:?}", reasoning);
        });
        let _ = resp.content.as_ref().map(|content| {
            tracing::debug!(session = %ctx.session_key, "LLM text content: {:?}", content);
        });

        emit_reasoning_and_thoughts(
            ctx,
            &config,
            &resp,
            duration.as_secs_f32(),
            content_streaming_started,
            reasoning_printed,
        )
        .await;

        match resp.content {
            None => {}
            Some(content) => {
                let text_repeat =
                    super::loop_control::count_previous_text_responses(&ctx.messages, &content);
                match text_repeat >= 2 {
                    true => {
                        emit_turn_halt_message(
                            ctx,
                            "⚠️",
                            "⚠️ Halted execution: Detected repetitive text responses.",
                        )
                        .await;
                        break;
                    }
                    false => {}
                }

                let should_append_caveat = config.research.require_sources_for_current_claims
                    && super::research_policy::has_live_research_intent(ctx.user_content);
                let pre_caveat_len = content.len();
                let content = match should_append_caveat {
                    true => run
                        .turn_source_ledger
                        .append_live_research_caveat_if_needed(content, true),
                    false => content,
                };
                let _ = (ctx.streamed && content.len() > pre_caveat_len).then(|| {
                    let appended_suffix = content[pre_caveat_len..].to_string();
                    let _ = crate::channels::websocket::ws_chat_id(ctx.session_key).map(|chat_id| {
                        crate::channels::websocket::publish_ws_event(
                            crate::channels::websocket::protocol::delta(
                                chat_id,
                                super::current_turn_id(),
                                appended_suffix,
                            ),
                        );
                    });
                });

                ctx.final_content = content.clone();
                let mut extra = serde_json::Map::new();
                let _ = resp.reasoning_content.as_ref().map(|reasoning| {
                    extra.insert(
                        "reasoning_content".to_string(),
                        serde_json::Value::String(reasoning.clone()),
                    );
                });
                ctx.messages.push(Message {
                    role: "assistant".to_string(),
                    content,
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra,
                });
            }
        }

        match resp.tool_calls.is_empty() {
            true => break,
            false => {}
        }

        let mut should_halt = false;
        let mut tool_results = Vec::new();
        let mut assistant_tool_calls_json = Vec::new();
        let mut loop_detection_messages = ctx.messages.clone();
        let retrieve_original_available = loop_ref.tools.get("retrieve_original").is_some();
        let normalized_calls: Vec<crate::providers::ToolCallRequest> = resp
            .tool_calls
            .into_iter()
            .map(|mut call| {
                call.name = loop_ref.tools.resolve_canonical_name(&call.name);
                auto_adjust_tool_call_with_availability(
                    call,
                    ctx.user_content,
                    retrieve_original_available,
                )
            })
            .collect();
        let mut prefetched_outputs = prefetch_parallel_safe_batch(
            &loop_ref.tools,
            &normalized_calls,
            &config,
            &turn_cancel,
        )
        .await;

        for call in normalized_calls {
            let orig_name = call.name.clone();
            let scoped_candidate =
                auto_scope_context_before_edit(call.clone(), &mut run.auto_scoped_edit_paths);
            match orig_name != "scope_context" && scoped_candidate.name == "scope_context" {
                true => {
                    let pre_scope_call = crate::providers::ToolCallRequest {
                        id: format!("auto_scope_{}", call.id),
                        name: scoped_candidate.name,
                        arguments: scoped_candidate.arguments,
                    };
                    let pre_silent = crate::agent::style::spinner::is_silent();
                    let pre_cancel = turn_cancel.clone();
                    match execute_auto_tool_call(
                        &loop_ref.tools,
                        &pre_scope_call,
                        &config,
                        ctx.session_key,
                        pre_silent,
                        &pre_cancel,
                        &mut ctx.turn_errors,
                    )
                    .await
                    {
                        Some(scope_result) => {
                            let _ = should_record_auto_scope_result(&scope_result).then(|| {
                                ctx.tools_used.push(pre_scope_call.name.clone());
                                super::transcript::append_auto_tool_result(
                                    &mut tool_results,
                                    &mut assistant_tool_calls_json,
                                    &pre_scope_call,
                                    scope_result,
                                );
                            });
                        }
                        None => {}
                    }
                }
                false => {}
            }
            match turn_cancel.is_cancelled() {
                true => break,
                false => {}
            }
            ctx.tools_used.push(call.name.clone());
            loop_ref.tools.record_tool_used(&call.name);

            crate::agent::activity::update_activity(
                ctx.session_key,
                "Executing tool",
                Some(&call.name),
            );
            let silent = crate::agent::style::spinner::is_silent();
            let formatted_args =
                super::tool_execution::format_tool_args(&call.name, &call.arguments);
            let tool_spinner_msg =
                crate::agent::style::get_tree_spinner_msg(&call.name, &formatted_args);
            let direct_url = direct_research_url(&call.name, &call.arguments);
            let direct_fetch_key = direct_research_fetch_key(&call.name, &call.arguments);
            let duplicate_direct_url = direct_fetch_key
                .as_ref()
                .is_some_and(|key| run.completed_direct_research_urls.contains(key));
            let is_same_user_direct_url = direct_url.as_ref().is_some_and(|url| {
                super::research_policy::extract_distinct_http_urls(ctx.user_content)
                    .iter()
                    .any(|u| u == url)
            });
            let out_of_scope_direct_page_lookup = run.direct_page_only
                && run.direct_page_fetched
                && is_research_lookup_tool(&call.name)
                && !is_same_user_direct_url;

            let tool_msg = format!("▸ Running *{}*...", formatted_args);
            super::tool_execution::send_progress_update(ctx.session_key, &tool_msg).await;

            let _ = crate::channels::websocket::ws_chat_id(ctx.session_key).map(|chat_id| {
                crate::channels::websocket::publish_ws_event(
                    crate::channels::websocket::protocol::tool_start(
                        chat_id,
                        super::current_turn_id(),
                        call.id.clone(),
                        call.name.clone(),
                        call.arguments.clone(),
                    ),
                );
            });

            let clean_name = crate::agent::style::get_tool_clean_name(&call.name);
            let details = crate::agent::style::clean_tool_args_msg(&call.name, &formatted_args);
            crate::channels::ratatui::send_ratatui_turn_event(
                crate::channels::ratatui::TurnEvent::ToolStart {
                    name: clean_name.clone(),
                    details: details.clone(),
                },
            );
            let tool_start_instant = std::time::Instant::now();

            let _ = (!silent).then(|| {
                crate::agent::style::print_tree_tool_start(&call.name, &formatted_args);
            });

            tracing::info!(
                session = %ctx.session_key,
                tool = %call.name,
                arguments = %call.arguments,
                "Executing tool call"
            );
            let fresh_brief_blocks_lookup =
                fresh_research_brief_blocks_lookup(ctx.user_content, &call.name, &call.arguments)
                    .await;
            let approval = super::security_approval::evaluate_tool_approval(
                &call,
                &loop_detection_messages,
                ctx.session_key,
                &config.agents.defaults.security_mode,
                silent,
                &mut run.loop_state,
            )
            .await;
            let _ = approval.should_halt.then(|| {
                should_halt = true;
            });

            let mut result_val = dispatch_tool_call(
                loop_ref,
                ctx,
                &mut run,
                &config,
                &call,
                &formatted_args,
                &tool_spinner_msg,
                silent,
                &turn_cancel_clone,
                &mut prefetched_outputs,
                &approval,
                out_of_scope_direct_page_lookup,
                duplicate_direct_url,
                fresh_brief_blocks_lookup,
            )
            .await;

            let is_repeat_error = (!approval.is_loop)
                && approval.repeat_count >= 1
                && (result_val.get("error").is_some()
                    || result_val.get("status_code").is_some_and(|c| c != 0)
                    || result_val.get("exit_code").is_some_and(|c| c != 0));
            let _ = is_repeat_error.then(|| {
                let _ = result_val.as_object_mut().map(|obj| {
                    obj.insert(
                        "z_guard_diagnostic".to_string(),
                        serde_json::json!({
                            "stage": "diagnostic_tag",
                            "repeat_attempt": approval.repeat_count + 1,
                            "required_action": "inspect_state_and_vary_parameters_before_retry"
                        }),
                    );
                });
            });

            let _ = crate::channels::websocket::ws_chat_id(ctx.session_key).map(|chat_id| {
                let status = result_val
                    .get("error")
                    .is_some()
                    .then_some("error")
                    .unwrap_or("success");
                let mut output = result_val.to_string();
                let _ = output.char_indices().nth(2000).map(|(idx, _)| {
                    output.truncate(idx);
                    output.push_str("...");
                });
                crate::channels::websocket::publish_ws_event(
                    crate::channels::websocket::protocol::tool_end(
                        chat_id,
                        super::current_turn_id(),
                        call.id.clone(),
                        call.name.clone(),
                        status,
                        output,
                    ),
                );
            });

            let tool_duration_ms = tool_start_instant.elapsed().as_millis() as u64;
            let outcome_summary = crate::agent::style::format_tool_outcome_summary(
                &call.name,
                &call.arguments,
                &result_val,
            );
            let tool_success = result_val.get("error").is_none()
                && result_val.get("status_code").is_none_or(|c| c == 0)
                && !outcome_summary.contains("Failed")
                && !outcome_summary.contains('\u{2715}');

            crate::channels::ratatui::send_ratatui_turn_event(
                crate::channels::ratatui::TurnEvent::ToolEnd {
                    name: clean_name,
                    output: result_val.to_string(),
                    summary: Some(outcome_summary),
                    success: tool_success,
                    duration_ms: Some(tool_duration_ms),
                },
            );
            let _ = is_research_lookup_tool(&call.name).then(|| {
                run.turn_source_ledger
                    .record_tool_result(&call.name, &call.arguments, &result_val);
            });
            let _ = direct_fetch_key
                .filter(|_| result_val.get("error").is_none())
                .map(|key| {
                    run.completed_direct_research_urls.insert(key);
                    run.direct_page_fetched = true;
                });

            match result_val.get("error").and_then(|v| v.as_str()) {
                Some(err_val) => {
                    ctx.turn_errors
                        .push(format!("Tool {} returned error: {}", call.name, err_val));
                }
                None => {
                    let _ = crate::tools::shared_memory::auto_capture_research_memory(
                        &call.name,
                        &call.arguments,
                        &result_val,
                        ctx.user_content,
                    )
                    .await
                    .ok()
                    .flatten()
                    .map(|capture| run.turn_capture_summaries.push(capture));
                }
            }
            crate::agent::activity::update_activity(
                ctx.session_key,
                "Processing user prompt",
                None,
            );

            let maybe_auto_open_call = auto_open_artifact_call_after_tool(
                ctx.user_content,
                &call,
                &result_val,
                &mut run.auto_opened_artifact_paths,
            );
            let maybe_device_suggest_call = auto_device_inventory_suggest_call_after_open_failure(
                &call,
                &result_val,
                &mut run.auto_suggested_open_targets,
            );

            let transcript_result = super::transcript::ToolTranscriptResult {
                id: call.id.clone(),
                name: call.name.clone(),
                result: result_val,
            };
            tool_results.push(transcript_result.clone());

            let mut assistant_tool_call = serde_json::json!({
                "id": call.id,
                "type": "function",
                "function": {
                    "name": call.name,
                    "arguments": call.arguments.to_string()
                }
            });
            let _ = super::loop_control::tool_arg_fingerprint(&call.arguments)
                .zip(assistant_tool_call.as_object_mut())
                .map(|(fingerprint, obj)| {
                    obj.insert(
                        "_openz_arg_fingerprint".to_string(),
                        serde_json::Value::String(fingerprint),
                    );
                });
            let loop_assistant_tool_call = assistant_tool_call.clone();
            assistant_tool_calls_json.push(assistant_tool_call);
            super::transcript::append_assistant_tool_calls(
                &mut loop_detection_messages,
                vec![loop_assistant_tool_call],
                None,
            );
            let mut loop_tool_extra = serde_json::Map::new();
            loop_tool_extra.insert(
                "tool_call_id".to_string(),
                serde_json::Value::String(transcript_result.id),
            );
            loop_tool_extra.insert(
                "name".to_string(),
                serde_json::Value::String(transcript_result.name),
            );
            loop_detection_messages.push(crate::session::Message {
                role: "tool".to_string(),
                content: transcript_result.result.to_string(),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: loop_tool_extra,
            });

            run_and_record_optional_auto_tool(
                &loop_ref.tools,
                maybe_device_suggest_call,
                &config,
                ctx.session_key,
                silent,
                &turn_cancel_clone,
                &mut ctx.turn_errors,
                &mut tool_results,
                &mut assistant_tool_calls_json,
            )
            .await;

            match maybe_auto_open_call {
                Some(open_call) => {
                    match execute_auto_tool_call(
                        &loop_ref.tools,
                        &open_call,
                        &config,
                        ctx.session_key,
                        silent,
                        &turn_cancel_clone,
                        &mut ctx.turn_errors,
                    )
                    .await
                    {
                        Some(open_result) => {
                            let maybe_open_suggest_call =
                                auto_device_inventory_suggest_call_after_open_failure(
                                    &open_call,
                                    &open_result,
                                    &mut run.auto_suggested_open_targets,
                                );

                            super::transcript::append_auto_tool_result(
                                &mut tool_results,
                                &mut assistant_tool_calls_json,
                                &open_call,
                                open_result,
                            );

                            run_and_record_optional_auto_tool(
                                &loop_ref.tools,
                                maybe_open_suggest_call,
                                &config,
                                ctx.session_key,
                                silent,
                                &turn_cancel_clone,
                                &mut ctx.turn_errors,
                                &mut tool_results,
                                &mut assistant_tool_calls_json,
                            )
                            .await;
                        }
                        None => {}
                    }
                }
                None => {}
            }
        }

        super::transcript::append_assistant_tool_calls(
            &mut ctx.messages,
            assistant_tool_calls_json,
            resp.reasoning_content.as_deref(),
        );

        super::transcript::append_tool_results(&mut ctx.messages, &config, tool_results).await;

        ctx.session.messages = ctx.messages.clone();
        match run.iterations.is_multiple_of(5) {
            true => {
                let _ = loop_ref.session_manager.save(&ctx.session).await.map_err(|e| {
                    tracing::warn!("Failed to save session incrementally in Run loop: {}", e);
                });
            }
            false => {}
        }

        run.iterations += 1;
        match should_halt {
            true => {
                synthesize_turn_checkpoint(
                    loop_ref,
                    ctx,
                    &settings,
                    &turn_cancel,
                    "⚠️ Halted execution: Too many repeating tool calls blocked by loop detection. Summarizing progress so far.",
                )
                .await;
                break;
            }
            false => {}
        }
    }

    publish_auto_capture_notice(
        ctx.session_key,
        ctx.config.agents.defaults.show_auto_capture_notices,
        &run.turn_capture_summaries,
    );

    ctx.session.messages = ctx.messages.clone();
    let _ = loop_ref.session_manager.save(&ctx.session).await.map_err(|e| {
        tracing::warn!(
            "Failed to save session unconditionally on final iteration in Run loop: {}",
            e
        );
    });
    match (ctx.interaction_id.as_deref(), ctx.turn_errors.is_empty()) {
        (Some(inter_id), false) => {
            let errors_str = ctx.turn_errors.join("\n");
            let _ =
                crate::tools::shared_memory::update_interaction_errors(inter_id, &errors_str).await;
        }
        _ => {}
    }

    Ok(TurnState::Save)
}
