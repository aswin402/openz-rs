//! Provider response continuation, cleanup, and reasoning-only recovery.

use super::events::{
    compact_reasoning_summary, normalize_tui_thought_display, should_show_tui_thoughts,
    stream_content_chunk,
};
use super::{AgentLoop, TurnContext};
use crate::providers::GenerationSettings;
use crate::session::Message;
use anyhow::Result;
use futures_util::StreamExt;
use std::io::Write;

pub(super) async fn normalize_response(
    loop_ref: &AgentLoop,
    ctx: &mut TurnContext<'_>,
    mut resp: crate::providers::LLMResponse,
    config: &crate::config::schema::Config,
    settings: &GenerationSettings,
    turn_cancel: &crate::tools::subagent::CancellationToken,
    start_time: std::time::Instant,
    reasoning_printed: &mut bool,
    content_streaming_started: &mut bool,
    current_line_buffer: &mut String,
) -> Result<crate::providers::LLMResponse> {
    if resp.finish_reason == "length" {
        let mut accumulated_content = resp.content.clone();
        let mut finish_reason = resp.finish_reason.clone();
        let mut continue_attempts = 0;

        while finish_reason == "length" && continue_attempts < 3 {
            if turn_cancel.is_cancelled() {
                break;
            }
            continue_attempts += 1;

            let mut temp_messages = ctx.messages.clone();
            if let Some(ref current_acc) = accumulated_content {
                temp_messages.push(Message {
                    role: "assistant".to_string(),
                    content: current_acc.clone(),
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra: serde_json::Map::new(),
                });
            }

            temp_messages.push(Message {
                role: "user".to_string(),
                content: "Continue generating the rest of your previous message exactly from where you left off. Do not repeat the beginning.".to_string(),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });

            let cont_activity_msg = format!(
                "{}▶ Continuing response... (attempt {}){}",
                crate::agent::style::RED_ORANGE,
                continue_attempts,
                crate::agent::style::COLOR_RESET
            );
            // Continuations are text-only so the model cannot start a new tool call.
            if let Ok(cont_resp) = loop_ref
                .chat_with_fallback(
                    &mut ctx.active_provider,
                    &ctx.system_prompt,
                    &temp_messages,
                    &[],
                    settings,
                    &cont_activity_msg,
                )
                .await
            {
                finish_reason = cont_resp.finish_reason.clone();
                if let Some(ref cont_content) = cont_resp.content {
                    if let Some(ref mut acc) = accumulated_content {
                        acc.push_str(cont_content);
                    } else {
                        accumulated_content = Some(cont_content.clone());
                    }
                }
                if !cont_resp.tool_calls.is_empty() {
                    resp.tool_calls.extend(cont_resp.tool_calls);
                }
            } else {
                break;
            }
        }

        resp.content = accumulated_content;
        resp.finish_reason = finish_reason;
    }

    if let Some(text) = resp.content.take() {
        let (clean_content, extracted_reasoning) =
            crate::providers::openai::split_think_blocks(&text);
        resp.content = clean_content;
        resp.reasoning_content = crate::providers::openai::merge_reasoning(
            resp.reasoning_content.take(),
            extracted_reasoning,
        );
    }

    if resp.tool_calls.is_empty() {
        if let Some(ref text) = resp.content {
            let parsed = crate::providers::openai::parse_fallback_tool_calls(text);
            if !parsed.is_empty() {
                resp.tool_calls = parsed;
                resp.content = None;
            }
        }
    }

    // Some providers return reasoning_content without a user-facing answer.
    // Ask once for a final answer instead of exposing internal reasoning as-is.
    if resp.content.is_none() && resp.reasoning_content.is_some() && resp.tool_calls.is_empty() {
        if !crate::agent::style::spinner::is_silent() {
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
        }

        if config.agents.defaults.streaming {
            let original_reasoning = resp.reasoning_content.take();
            let mut recovery_messages = ctx.messages.clone();
            let recovery_prompt = match original_reasoning.as_deref() {
                Some(reasoning) if !reasoning.trim().is_empty() => format!(
                    "Your previous streamed response contained reasoning only and no user-facing answer. Here is that reasoning:\n\n{}\n\nNow provide only the final user-facing answer to my last message. Do not include analysis, reasoning labels, or tool calls.",
                    reasoning.trim()
                ),
                _ => "Your previous streamed response contained reasoning only and no user-facing answer. Provide only the final answer to my last message now. Do not include reasoning, analysis, or tool calls.".to_string(),
            };
            recovery_messages.push(Message {
                role: "user".to_string(),
                content: recovery_prompt,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
            let recovery_activity_msg = String::new();

            match loop_ref
                .chat_stream_with_fallback(
                    &mut ctx.active_provider,
                    &ctx.system_prompt,
                    &recovery_messages,
                    &[],
                    settings,
                    &recovery_activity_msg,
                )
                .await
            {
                Ok(mut recovery_stream) => {
                    let mode = normalize_tui_thought_display(
                        &config.agents.defaults.tui_thought_display,
                    );
                    let mut recovery_content = String::new();
                    let mut recovery_reasoning = String::new();
                    let mut recovery_finish_reason = "stop".to_string();

                    let recovery_stream_idle_timeout = loop_ref.provider_attempt_timeout_duration();
                    loop {
                        let Some(chunk) = tokio::time::timeout(
                            recovery_stream_idle_timeout,
                            recovery_stream.next(),
                        )
                        .await
                        .map_err(|_| {
                            anyhow::anyhow!(
                                "Provider recovery stream timed out after {}s without output",
                                recovery_stream_idle_timeout.as_secs()
                            )
                        })?
                        else {
                            break;
                        };
                        match chunk? {
                            crate::providers::ChatStreamChunk::Content(text) => {
                                if !*reasoning_printed
                                    && should_show_tui_thoughts(mode)
                                    && !crate::agent::style::spinner::is_silent()
                                {
                                    let duration_secs = start_time.elapsed().as_secs_f32();
                                    let depth = crate::tools::subagent::DELEGATION_DEPTH
                                        .try_with(|d| *d)
                                        .unwrap_or(0);
                                    let prefix = if depth > 0 {
                                        crate::agent::style::get_tree_prefix(false)
                                    } else {
                                        String::new()
                                    };
                                    crate::tui_println!(
                                        "{}{}● {}{}{}Thought for {:.1}s{}",
                                        prefix,
                                        crate::agent::style::RED_ORANGE,
                                        crate::agent::style::COLOR_RESET,
                                        crate::agent::style::COLOR_BOLD,
                                        crate::agent::style::RED_ORANGE,
                                        duration_secs,
                                        crate::agent::style::COLOR_RESET
                                    );
                                    if let Some(ref reasoning) = original_reasoning {
                                        let visible_reasoning = if mode == "compact" {
                                            compact_reasoning_summary(reasoning)
                                        } else {
                                            reasoning.clone()
                                        };
                                        let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                                        crate::agent::style::print_tree_monologue(
                                            &leaf_prefix,
                                            &visible_reasoning,
                                        );
                                        crate::tui_println!("");
                                    }
                                    *reasoning_printed = true;
                                }

                                recovery_content.push_str(&text);
                                stream_content_chunk(
                                    &text,
                                    current_line_buffer,
                                    crate::agent::style::spinner::is_silent(),
                                    content_streaming_started,
                                );
                                crate::agent::agent_loop::tool_execution::send_progress_update(
                                    ctx.session_key,
                                    &text,
                                )
                                .await;
                            }
                            crate::providers::ChatStreamChunk::Reasoning(text) => {
                                recovery_reasoning.push_str(&text);
                            }
                            crate::providers::ChatStreamChunk::ToolCall { .. } => {}
                            crate::providers::ChatStreamChunk::Done { finish_reason } => {
                                if let Some(reason) = finish_reason {
                                    recovery_finish_reason = reason;
                                }
                            }
                        }
                    }

                    if !current_line_buffer.is_empty()
                        && !crate::agent::style::spinner::is_silent()
                    {
                        print!("\r\x1b[2K");
                        print!("{}", super::events::format_markdown_line(current_line_buffer));
                        let _ = std::io::stdout().flush();
                    }

                    let recovery_reasoning_visible = recovery_reasoning.trim().to_string();
                    let recovered_content = if !recovery_content.trim().is_empty() {
                        Some(recovery_content)
                    } else if !recovery_reasoning_visible.is_empty() {
                        Some(recovery_reasoning_visible)
                    } else {
                        Some("I did not receive a final answer from the model for this turn.".to_string())
                    };
                    let recovery_reasoning_for_memory = if recovery_reasoning.trim().is_empty()
                        || recovered_content
                            .as_ref()
                            .is_some_and(|content| content == recovery_reasoning.trim())
                    {
                        None
                    } else {
                        Some(recovery_reasoning)
                    };
                    resp.content = recovered_content;
                    resp.reasoning_content = crate::providers::openai::merge_reasoning(
                        original_reasoning,
                        recovery_reasoning_for_memory,
                    );
                    resp.finish_reason = recovery_finish_reason;
                    resp.tool_calls.clear();
                    ctx.streamed = *content_streaming_started;
                }
                Err(e) => {
                    tracing::warn!(
                        session = %ctx.session_key,
                        error = %e,
                        "Failed to recover final answer from reasoning-only stream"
                    );
                    resp.content = Some(
                        original_reasoning
                            .as_deref()
                            .filter(|reasoning| !reasoning.trim().is_empty())
                            .unwrap_or("I did not receive a final answer from the model for this turn.")
                            .to_string(),
                    );
                    resp.reasoning_content = None;
                    ctx.streamed = false;
                    *content_streaming_started = false;
                }
            }
        } else {
            resp.content = resp.reasoning_content.take();
            ctx.streamed = false;
        }
    }

    Ok(resp)
}
