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

async fn continue_truncated_response(
    loop_ref: &AgentLoop,
    ctx: &mut TurnContext<'_>,
    resp: &mut crate::providers::LLMResponse,
    settings: &GenerationSettings,
    turn_cancel: &crate::tools::subagent::CancellationToken,
) {
    let mut accumulated_content = resp.content.clone();
    let mut finish_reason = resp.finish_reason.clone();
    let mut continue_attempts = 0;

    while finish_reason == "length" && continue_attempts < 3 && !turn_cancel.is_cancelled() {
        continue_attempts += 1;

        let mut temp_messages = ctx.messages.clone();
        let _ = accumulated_content.as_ref().map(|current_acc| {
            temp_messages.push(Message {
                role: "assistant".to_string(),
                content: current_acc.clone(),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
        });

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
        match loop_ref
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
            Ok(cont_resp) => {
                finish_reason = cont_resp.finish_reason.clone();
                let _ = cont_resp.content.as_ref().map(|cont_content| {
                    match accumulated_content.as_mut() {
                        Some(acc) => acc.push_str(cont_content),
                        None => accumulated_content = Some(cont_content.clone()),
                    }
                });
                let _ = (!cont_resp.tool_calls.is_empty())
                    .then(|| resp.tool_calls.extend(cont_resp.tool_calls));
            }
            Err(_) => break,
        }
    }

    resp.content = accumulated_content;
    resp.finish_reason = finish_reason;
}

fn build_recovery_prompt(original_reasoning: Option<&str>, has_tools: bool) -> String {
    let trimmed = original_reasoning.map(str::trim).filter(|s| !s.is_empty());
    match (trimmed, has_tools) {
        (Some(reasoning), true) => format!(
            "Your previous streamed response contained reasoning only and stopped before executing a tool call or providing a final answer. Here is that reasoning:\n\n{}\n\nNow execute the required tool call(s) to fulfill the request, or if no tool is needed, provide only the final user-facing answer to my last message without reasoning labels.",
            reasoning
        ),
        (None, true) => "Your previous streamed response contained reasoning only. Now execute the required tool call(s) to fulfill the request, or provide only the final user-facing answer to my last message.".to_string(),
        (Some(reasoning), false) => format!(
            "Your previous streamed response contained reasoning only and no user-facing answer. Here is that reasoning:\n\n{}\n\nNow provide only the final user-facing answer to my last message. Do not include analysis, reasoning labels, or tool calls.",
            reasoning
        ),
        (None, false) => "Your previous streamed response contained reasoning only and no user-facing answer. Provide only the final answer to my last message now. Do not include reasoning, analysis, or tool calls.".to_string(),
    }
}

fn maybe_print_recovery_reasoning(
    reasoning_printed: &mut bool,
    mode: &str,
    start_time: std::time::Instant,
    original_reasoning: Option<&String>,
) {
    let should_print = !*reasoning_printed
        && should_show_tui_thoughts(mode)
        && !crate::agent::style::spinner::is_silent();
    let _ = should_print.then(|| {
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
        let duration_secs = start_time.elapsed().as_secs_f32();
        let depth = crate::tools::subagent::DELEGATION_DEPTH
            .try_with(|d| *d)
            .unwrap_or(0);
        let prefix = (depth > 0)
            .then(|| crate::agent::style::get_tree_prefix(false))
            .unwrap_or_default();
        crate::tui_println!(
            "{}{}• {}{}{}Thought for {:.1}s{}",
            prefix,
            crate::agent::style::RED_ORANGE,
            crate::agent::style::COLOR_RESET,
            crate::agent::style::COLOR_BOLD,
            crate::agent::style::RED_ORANGE,
            duration_secs,
            crate::agent::style::COLOR_RESET
        );
        let _ = original_reasoning.map(|reasoning| {
            let visible_reasoning = match mode == "compact" {
                true => compact_reasoning_summary(reasoning),
                false => reasoning.clone(),
            };
            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
            crate::agent::style::print_tree_monologue(&leaf_prefix, &visible_reasoning);
            crate::tui_println!("");
        });
        *reasoning_printed = true;
    });
}

#[allow(clippy::too_many_arguments)]
async fn recover_from_reasoning_only_response(
    loop_ref: &AgentLoop,
    ctx: &mut TurnContext<'_>,
    resp: &mut crate::providers::LLMResponse,
    config: &crate::config::schema::Config,
    tools_openai: &[serde_json::Value],
    settings: &GenerationSettings,
    start_time: std::time::Instant,
    reasoning_printed: &mut bool,
    content_streaming_started: &mut bool,
    markdown_streamer: &mut crate::channels::cli::render::StreamingMarkdownRenderer,
) -> Result<()> {
    let _ = (!crate::agent::style::spinner::is_silent()).then(|| {
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
    });

    match config.agents.defaults.streaming {
        false => {
            resp.content = resp.reasoning_content.take();
            ctx.streamed = false;
            Ok(())
        }
        true => {
            let original_reasoning = resp.reasoning_content.take();
            let mut recovery_messages = ctx.messages.clone();
            let recovery_prompt =
                build_recovery_prompt(original_reasoning.as_deref(), !tools_openai.is_empty());
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
                    tools_openai,
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
                    let mut recovery_assembly =
                        crate::agent::agent_loop::streaming::StreamingAssembly::new();

                    let recovery_stream_idle_timeout = loop_ref.provider_attempt_timeout_duration();
                    while let Some(chunk) = tokio::time::timeout(
                        recovery_stream_idle_timeout,
                        recovery_stream.next(),
                    )
                    .await
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "Provider recovery stream timed out after {}s without output",
                            recovery_stream_idle_timeout.as_secs()
                        )
                    })? {
                        match chunk? {
                            crate::providers::ChatStreamChunk::Content(text) => {
                                maybe_print_recovery_reasoning(
                                    reasoning_printed,
                                    mode,
                                    start_time,
                                    original_reasoning.as_ref(),
                                );
                                recovery_content.push_str(&text);
                                stream_content_chunk(
                                    &text,
                                    markdown_streamer,
                                    content_streaming_started,
                                );
                                crate::agent::agent_loop::tool_execution::send_progress_update(
                                    ctx.session_key,
                                    &text,
                                )
                                .await;
                                recovery_assembly.push_chunk(
                                    crate::providers::ChatStreamChunk::Content(text),
                                );
                            }
                            crate::providers::ChatStreamChunk::Reasoning(text) => {
                                recovery_reasoning.push_str(&text);
                                recovery_assembly.push_chunk(
                                    crate::providers::ChatStreamChunk::Reasoning(text),
                                );
                            }
                            crate::providers::ChatStreamChunk::ToolCall {
                                index,
                                id,
                                name,
                                arguments,
                            } => {
                                recovery_assembly.push_chunk(
                                    crate::providers::ChatStreamChunk::ToolCall {
                                        index,
                                        id,
                                        name,
                                        arguments,
                                    },
                                );
                            }
                            crate::providers::ChatStreamChunk::Done { finish_reason } => {
                                let _ = finish_reason.as_ref().map(|reason| {
                                    recovery_finish_reason = reason.clone();
                                });
                                recovery_assembly.push_chunk(
                                    crate::providers::ChatStreamChunk::Done { finish_reason },
                                );
                            }
                        }
                    }

                    markdown_streamer.finish();
                    let assembled_recovery = recovery_assembly.into_response();

                    match assembled_recovery.tool_calls.is_empty() {
                        false => {
                            resp.tool_calls = assembled_recovery.tool_calls;
                            resp.content = None;
                            resp.reasoning_content = crate::providers::openai::merge_reasoning(
                                original_reasoning,
                                (!recovery_reasoning.trim().is_empty())
                                    .then_some(recovery_reasoning),
                            );
                            resp.finish_reason = "tool_calls".to_string();
                            ctx.streamed = false;
                        }
                        true => {
                            let recovery_reasoning_visible = recovery_reasoning.trim().to_string();
                            let recovered_content = (!recovery_content.trim().is_empty())
                                .then(|| recovery_content.clone())
                                .or_else(|| {
                                    (!recovery_reasoning_visible.is_empty())
                                        .then(|| recovery_reasoning_visible.clone())
                                })
                                .or_else(|| {
                                    Some(
                                        "I did not receive a final answer from the model for this turn."
                                            .to_string(),
                                    )
                                });
                            let hide_reasoning_for_memory = recovery_reasoning.trim().is_empty()
                                || recovered_content
                                    .as_ref()
                                    .is_some_and(|content| content == recovery_reasoning.trim());
                            let recovery_reasoning_for_memory =
                                (!hide_reasoning_for_memory).then_some(recovery_reasoning);
                            resp.content = recovered_content;
                            resp.reasoning_content = crate::providers::openai::merge_reasoning(
                                original_reasoning,
                                recovery_reasoning_for_memory,
                            );
                            resp.finish_reason = recovery_finish_reason;
                            resp.tool_calls.clear();
                            ctx.streamed = *content_streaming_started;
                        }
                    }
                    Ok(())
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
                    Ok(())
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn normalize_response(
    loop_ref: &AgentLoop,
    ctx: &mut TurnContext<'_>,
    mut resp: crate::providers::LLMResponse,
    config: &crate::config::schema::Config,
    tools_openai: &[serde_json::Value],
    settings: &GenerationSettings,
    turn_cancel: &crate::tools::subagent::CancellationToken,
    start_time: std::time::Instant,
    reasoning_printed: &mut bool,
    content_streaming_started: &mut bool,
    markdown_streamer: &mut crate::channels::cli::render::StreamingMarkdownRenderer,
) -> Result<crate::providers::LLMResponse> {
    match resp.finish_reason == "length" {
        true => {
            continue_truncated_response(loop_ref, ctx, &mut resp, settings, turn_cancel).await;
        }
        false => {}
    }

    let _ = resp.content.take().map(|text| {
        let (clean_content, extracted_reasoning) =
            crate::providers::openai::split_think_blocks(&text);
        resp.content = clean_content;
        resp.reasoning_content = crate::providers::openai::merge_reasoning(
            resp.reasoning_content.take(),
            extracted_reasoning,
        );
    });

    let _ = resp
        .tool_calls
        .is_empty()
        .then(|| resp.content.as_deref())
        .flatten()
        .map(crate::providers::openai::parse_fallback_tool_calls)
        .filter(|parsed| !parsed.is_empty())
        .map(|parsed| {
            resp.tool_calls = parsed;
            resp.content = None;
        });

    let _ = (resp.tool_calls.is_empty() && resp.content.is_none())
        .then(|| resp.reasoning_content.as_deref())
        .flatten()
        .filter(|reasoning| {
            reasoning.contains("[TOOL_CALL]")
                || reasoning.contains("<tool_call>")
                || reasoning.contains("```tool_call")
        })
        .map(crate::providers::openai::parse_fallback_tool_calls)
        .filter(|parsed| !parsed.is_empty())
        .map(|parsed| {
            resp.tool_calls = parsed;
        });

    match resp.content.is_none() && resp.reasoning_content.is_some() && resp.tool_calls.is_empty() {
        true => {
            recover_from_reasoning_only_response(
                loop_ref,
                ctx,
                &mut resp,
                config,
                tools_openai,
                settings,
                start_time,
                reasoning_printed,
                content_streaming_started,
                markdown_streamer,
            )
            .await?;
        }
        false => {}
    }

    let _ = (resp.content.is_none() && resp.reasoning_content.is_none() && resp.tool_calls.is_empty())
        .then(|| {
            let finish = resp.finish_reason.to_lowercase();
            (finish.contains("filter") || finish.contains("malformed")).then(|| {
                resp.content = Some(format!(
                    "The model call ended without output due to provider safety/filter reason: '{}'. Check tool call arguments or system prompt constraints.",
                    resp.finish_reason
                ));
            })
        });

    Ok(resp)
}
