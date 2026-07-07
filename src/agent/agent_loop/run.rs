use super::{AgentLoop, TurnContext, TurnState};
use crate::agent::style::*;
use crate::providers::GenerationSettings;
use crate::session::Message;
use anyhow::Result;
use futures_util::StreamExt;
use std::io::Write;

fn should_cancel_turn_after_tool_error(error_str: &str) -> bool {
    let lower = error_str.to_lowercase();
    lower.contains("cancelled by user")
        || lower.contains("canceled by user")
        || lower.contains("subagent task cancelled")
}

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let mut iterations = 0;
    let mut loop_blocked_count = 0;
    let max_iterations = ctx.config.agents.defaults.max_tool_iterations;

    // Build a turn-level cancellation token from the current CLI context.
    // This provides early cancellation detection even before the CLI select! drops run_fut.
    let turn_cancel = crate::tools::subagent::CancellationToken::new();
    let turn_cancel_clone = turn_cancel.clone();

    loop {
        // Check for turn-level cancellation at the start of each iteration.
        // Without this, a subagent cancellation error is fed back to the LLM
        // which may continue iterating instead of stopping.
        if turn_cancel.is_cancelled() {
            let msg = "Turn cancelled by user.".to_string();
            ctx.final_content = msg.clone();
            super::tool_execution::send_progress_update(ctx.session_key, &msg).await;
            if !crate::agent::style::spinner::is_silent() {
                crate::tui_println!("{}▲ {}{}", AURA_GOLD, msg, COLOR_RESET);
            }
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: msg,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
            break;
        }
        let config = ctx.config.clone();
        let settings = GenerationSettings {
            temperature: config.agents.defaults.temperature,
            max_tokens: config.agents.defaults.max_tokens,
            reasoning_effort: None,
        };

        tracing::info!(
            session = %ctx.session_key,
            iteration = iterations,
            "Sending completion request to LLM (model: {})",
            config.agents.defaults.model
        );
        if iterations >= max_iterations {
            let msg = format!(
                "⚠️ Reached tool iteration limit ({}). Summarizing work so far.",
                max_iterations
            );
            ctx.final_content = msg.clone();
            super::tool_execution::send_progress_update(ctx.session_key, &msg).await;
            if !crate::agent::style::spinner::is_silent() {
                crate::tui_println!("{}⚠️ {}{}", AURA_GOLD, msg, COLOR_RESET);
            }
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: msg,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
            break;
        }

        let tools_openai = loop_ref.tools.to_openai_format();

        let activity_msg = format!("{}▶ Thinking{}", RED_ORANGE, COLOR_RESET);
        let start_time = std::time::Instant::now();
        // Track if content was already streamed to terminal (to avoid duplicate display)
        let mut content_streaming_started = false;
        let mut reasoning_printed = false;
        let mut current_line_buffer = String::new();
        let mut resp = if config.agents.defaults.streaming {
            let mut stream = loop_ref
                .chat_stream_with_fallback(
                    &mut ctx.active_provider,
                    &ctx.system_prompt,
                    &ctx.messages,
                    &tools_openai,
                    &settings,
                    &activity_msg,
                )
                .await?;
            let silent = crate::agent::style::spinner::is_silent();

            let mut full_content = String::new();
            let mut full_reasoning = String::new();
            // Track whether we're currently in reasoning phase (for live spinner)
            let mut in_reasoning_phase = false;

            let print_reasoning =
                |full_reasoning: &str,
                 in_reasoning_phase: &mut bool,
                 reasoning_printed: &mut bool,
                 start_time: std::time::Instant| {
                    if !*reasoning_printed && !full_reasoning.is_empty() {
                        let depth = crate::tools::subagent::DELEGATION_DEPTH
                            .try_with(|d| *d)
                            .unwrap_or(0);
                        if !silent {
                            let elapsed = start_time.elapsed().as_secs_f32();
                            print!("\r\x1b[2K");
                            let prefix = if depth > 0 {
                                crate::agent::style::get_tree_prefix(false)
                            } else {
                                "".to_string()
                            };
                            print!(
                                "{}{}● {}{}{}Thought for {:.1}s{}\r\n",
                                prefix,
                                RED_ORANGE,
                                COLOR_RESET,
                                COLOR_BOLD,
                                RED_ORANGE,
                                elapsed,
                                COLOR_RESET
                            );
                            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                            crate::agent::style::print_tree_monologue(&leaf_prefix, full_reasoning);
                            print!("\r\n");
                            let _ = std::io::stdout().flush();
                        }
                        *reasoning_printed = true;
                        *in_reasoning_phase = false;
                    }
                };

            let mut streaming_assembly = super::streaming::StreamingAssembly::new();

            // Create a cancel receiver for the streaming loop.
            // watch::Receiver::changed() is cancel-safe, unlike Notify.
            let stream_cancel_tx = crate::shutdown::cli_cancel_tx();
            let mut stream_cancel_rx = stream_cancel_tx.subscribe();
            let stream_cancel_initial = *stream_cancel_rx.borrow();

            loop {
                // Race: next stream chunk vs cancellation signal
                let chunk = tokio::select! {
                    biased;
                    _ = async {
                        while *stream_cancel_rx.borrow() == stream_cancel_initial {
                            if stream_cancel_rx.changed().await.is_err() { break; }
                        }
                    } => {
                        break; // cancelled
                    }
                    next = stream.next() => {
                        match next {
                            Some(c) => c,
                            None => break, // stream ended
                        }
                    }
                };
                match chunk? {
                    crate::providers::ChatStreamChunk::Content(text) => {
                        // If we have reasoning content that hasn't been printed yet, print it now
                        print_reasoning(
                            &full_reasoning,
                            &mut in_reasoning_phase,
                            &mut reasoning_printed,
                            start_time,
                        );

                        // If we were in reasoning phase but reasoning was empty/already printed, clear the spinner
                        if in_reasoning_phase && !silent {
                            print!("\r\x1b[2K");
                            let _ = std::io::stdout().flush();
                            in_reasoning_phase = false;
                        }
                        full_content.push_str(&text);
                        for c in text.chars() {
                            if c == '\r' {
                                continue;
                            }
                            if c == '\n' {
                                if !silent {
                                    content_streaming_started = true;
                                    print!("\r\x1b[2K");
                                    print!("{}", format_markdown_line(&current_line_buffer));
                                    print!("\r\n");
                                    let _ = std::io::stdout().flush();
                                }
                                current_line_buffer.clear();
                            } else {
                                current_line_buffer.push(c);
                                if !silent {
                                    content_streaming_started = true;
                                    print!("{}", c);
                                    let _ = std::io::stdout().flush();
                                }
                            }
                        }
                        super::tool_execution::send_progress_update(ctx.session_key, &text).await;
                        streaming_assembly
                            .push_chunk(crate::providers::ChatStreamChunk::Content(text));
                    }
                    crate::providers::ChatStreamChunk::Reasoning(text) => {
                        full_reasoning.push_str(&text);
                        in_reasoning_phase = true;
                        // Show a live thinking spinner instead of raw reasoning text
                        if !silent {
                            let elapsed = start_time.elapsed().as_secs_f32();
                            print!(
                                "\r\x1b[2K{}{}▶ Thinking... {:.1}s{}",
                                COLOR_BOLD, RED_ORANGE, elapsed, COLOR_RESET
                            );
                            let _ = std::io::stdout().flush();
                        }
                        streaming_assembly
                            .push_chunk(crate::providers::ChatStreamChunk::Reasoning(text));
                    }
                    crate::providers::ChatStreamChunk::ToolCall {
                        index,
                        id,
                        name,
                        arguments,
                    } => {
                        // If we have reasoning content that hasn't been printed yet, print it now
                        print_reasoning(
                            &full_reasoning,
                            &mut in_reasoning_phase,
                            &mut reasoning_printed,
                            start_time,
                        );

                        // Also clear thinking spinner if active
                        if in_reasoning_phase && !silent {
                            print!("\r\x1b[2K");
                            let _ = std::io::stdout().flush();
                            in_reasoning_phase = false;
                        }

                        streaming_assembly.push_chunk(
                            crate::providers::ChatStreamChunk::ToolCall {
                                index,
                                id,
                                name,
                                arguments,
                            },
                        );
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

            if in_reasoning_phase && !silent {
                print!("\r\x1b[2K");
                let _ = std::io::stdout().flush();
                in_reasoning_phase = false;
            }

            // Print any reasoning that was not printed yet
            print_reasoning(
                &full_reasoning,
                &mut in_reasoning_phase,
                &mut reasoning_printed,
                start_time,
            );

            // Print the final line in the buffer if any
            if !current_line_buffer.is_empty() && !silent {
                print!("\r\x1b[2K");
                print!("{}", format_markdown_line(&current_line_buffer));
                let _ = std::io::stdout().flush();
            }

            ctx.streamed = true;

            let mut assembled = streaming_assembly.into_response();
            assembled.content = if full_content.is_empty() {
                None
            } else {
                Some(full_content)
            };
            assembled.reasoning_content = if full_reasoning.is_empty() {
                None
            } else {
                Some(full_reasoning)
            };
            assembled
        } else {
            // Race non-streaming LLM call against cancel signal
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
                _ = async {
                    while *ns_cancel_rx.borrow() == ns_cancel_initial {
                        if ns_cancel_rx.changed().await.is_err() { break; }
                    }
                } => {
                    turn_cancel_clone.cancel();
                    let msg = "LLM request cancelled by user.".to_string();
                    ctx.final_content = msg.clone();
                    return Ok(TurnState::Save);
                }
                res = chat_fut => res?,
            }
        };

        // Handle potential response truncation (finish_reason = "length") by auto-continuing
        if resp.finish_reason == "length" {
            let mut accumulated_content = resp.content.clone();
            let mut finish_reason = resp.finish_reason.clone();
            let mut continue_attempts = 0;

            while finish_reason == "length" && continue_attempts < 3 {
                // Check for cancellation before each continuation attempt
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
                    RED_ORANGE, continue_attempts, COLOR_RESET
                );
                // Pass &[] instead of tools_openai so the model does not get confused and attempt to generate tool calls during text continuation
                if let Ok(cont_resp) = loop_ref
                    .chat_with_fallback(
                        &mut ctx.active_provider,
                        &ctx.system_prompt,
                        &temp_messages,
                        &[],
                        &settings,
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

            // If tool_calls is empty, try to parse tool calls from the fully completed accumulated content
            if resp.tool_calls.is_empty() {
                if let Some(ref text) = resp.content {
                    let parsed = crate::providers::openai::parse_fallback_tool_calls(text);
                    if !parsed.is_empty() {
                        resp.tool_calls = parsed;
                        resp.content = None;
                    }
                }
            }
        }

        // Handle models that send everything as reasoning_content with no content.
        // When there's reasoning but no content and no tool calls, the reasoning IS the response.
        // Common with DeepSeek-V4 and similar reasoning models.
        if resp.content.is_none() && resp.reasoning_content.is_some() && resp.tool_calls.is_empty()
        {
            resp.content = resp.reasoning_content.take();
            // If we already printed it as reasoning on the terminal, set streamed = true
            // to avoid printing it again in cli.rs
            ctx.streamed = reasoning_printed;
            // Clear any thinking spinner that was active on the terminal
            if !crate::agent::style::spinner::is_silent() {
                print!("\r\x1b[2K");
                let _ = std::io::stdout().flush();
            }
        }

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
        if let Some(ref reasoning) = resp.reasoning_content {
            tracing::debug!(session = %ctx.session_key, "LLM reasoning content: {:?}", reasoning);
        }
        if let Some(ref content) = resp.content {
            tracing::debug!(session = %ctx.session_key, "LLM text content: {:?}", content);
        }

        let duration_secs = duration.as_secs_f32();
        let has_reasoning = resp
            .reasoning_content
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let has_content = resp
            .content
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let has_tool_calls = !resp.tool_calls.is_empty();

        if has_reasoning || (has_content && has_tool_calls) {
            // Send reasoning/thought summary to non-CLI channels (Telegram, WS, etc.)
            if has_reasoning {
                if let Some(ref reasoning) = resp.reasoning_content {
                    let reasoning_msg = format!(
                        "▶ *Thought*\n\n> {}",
                        reasoning.trim().replace('\n', "\n> ")
                    );
                    super::tool_execution::send_progress_update(ctx.session_key, &reasoning_msg)
                        .await;
                }
            } else if has_content && has_tool_calls {
                if let Some(ref content) = resp.content {
                    let thought_msg =
                        format!("▶ *Thought*\n\n> {}", content.trim().replace('\n', "\n> "));
                    super::tool_execution::send_progress_update(ctx.session_key, &thought_msg)
                        .await;
                }
            }

            let silent = crate::agent::style::spinner::is_silent();
            let depth = crate::tools::subagent::DELEGATION_DEPTH
                .try_with(|d| *d)
                .unwrap_or(0);
            if !silent {
                let prefix = if depth > 0 {
                    crate::agent::style::get_tree_prefix(false)
                } else {
                    "".to_string()
                };
                if ctx.streamed {
                    // During streaming, the reasoning spinner was already shown and
                    // the "Thought for Xs" badge was already printed when content
                    // started arriving or when the stream finished. If no content
                    // arrived and no reasoning was printed (e.g. pure tool-call-only response),
                    // finalize the spinner and print the badge now.
                    if !content_streaming_started && !reasoning_printed {
                        print!("\r\x1b[2K");
                        print!(
                            "{}{}● {}{}{}Thought for {:.1}s{}\r\n",
                            prefix,
                            RED_ORANGE,
                            COLOR_RESET,
                            COLOR_BOLD,
                            RED_ORANGE,
                            duration_secs,
                            COLOR_RESET
                        );
                        let _ = std::io::stdout().flush();
                    }
                } else {
                    // Non-streaming path: print the badge and thinking summary
                    print!(
                        "{}{}● {}{}{}Thought for {:.1}s{}\r\n",
                        prefix,
                        RED_ORANGE,
                        COLOR_RESET,
                        COLOR_BOLD,
                        RED_ORANGE,
                        duration_secs,
                        COLOR_RESET
                    );
                    let full_reasoning = if has_reasoning {
                        resp.reasoning_content.clone().unwrap_or_default()
                    } else if has_content && has_tool_calls {
                        resp.content.clone().unwrap_or_default()
                    } else {
                        String::new()
                    };
                    if !full_reasoning.is_empty() {
                        let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                        crate::agent::style::print_tree_monologue(&leaf_prefix, &full_reasoning);
                        print!("\r\n");
                    }
                    let _ = std::io::stdout().flush();
                }
                print!("\r\n");
                let _ = std::io::stdout().flush();
            }
        }

        if let Some(content) = resp.content {
            let text_repeat =
                super::loop_control::count_previous_text_responses(&ctx.messages, &content);
            if text_repeat >= 2 {
                let loop_msg = "⚠️ Halted execution: Detected repetitive text responses.";
                ctx.final_content = loop_msg.to_string();
                super::tool_execution::send_progress_update(ctx.session_key, loop_msg).await;
                if !crate::agent::style::spinner::is_silent() {
                    crate::tui_println!("{}⚠️ {}{}", AURA_GOLD, loop_msg, COLOR_RESET);
                }
                ctx.messages.push(Message {
                    role: "assistant".to_string(),
                    content: loop_msg.to_string(),
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra: serde_json::Map::new(),
                });
                break;
            }

            ctx.final_content = content.clone();
            let mut extra = serde_json::Map::new();
            if let Some(ref reasoning) = resp.reasoning_content {
                extra.insert(
                    "reasoning_content".to_string(),
                    serde_json::Value::String(reasoning.clone()),
                );
            }
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra,
            });
        }

        if resp.tool_calls.is_empty() {
            break;
        }

        let mut should_halt = false;
        let mut tool_results = Vec::new();
        let mut assistant_tool_calls_json = Vec::new();

        for call in resp.tool_calls {
            // Break early if already cancelled (e.g. previous tool in batch was cancelled)
            if turn_cancel.is_cancelled() {
                break;
            }
            ctx.tools_used.push(call.name.clone());

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

            let tool_msg = format!("▸ Running *{}*...", formatted_args);
            super::tool_execution::send_progress_update(ctx.session_key, &tool_msg).await;

            if !silent {
                crate::agent::style::print_tree_tool_start(&call.name, &formatted_args);
            }

            tracing::info!(
                session = %ctx.session_key,
                tool = %call.name,
                arguments = %call.arguments,
                "Executing tool call"
            );
            let approval = super::security_approval::evaluate_tool_approval(
                &call,
                &ctx.messages,
                ctx.session_key,
                &config.agents.defaults.security_mode,
                silent,
                &mut loop_blocked_count,
            )
            .await;
            if approval.should_halt {
                should_halt = true;
            }

            let result_val = if let Some(err_msg) = approval.parse_error.as_deref() {
                let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, err_msg);
                super::tool_execution::send_progress_update(ctx.session_key, &fail_msg).await;
                if !silent {
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
                }
                ctx.turn_errors.push(format!(
                    "Tool {} arguments parse error: {}",
                    call.name, err_msg
                ));
                serde_json::json!({ "error": err_msg })
            } else if approval.is_loop {
                let warning_str = format!(
                    "Loop detected: You have already executed the tool '{}' with these exact arguments {} times in this turn. To prevent infinite loops, execution was blocked. Do NOT call this tool again. Analyze previous tool outputs and use a different strategy, or finish your response.",
                    call.name, approval.repeat_count
                );
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}↶ Loop detected for tool '{}'! Blocking execution. (Count: {}){}",
                        AURA_SLATE,
                        leaf_prefix,
                        AURA_GOLD,
                        call.name,
                        loop_blocked_count,
                        COLOR_RESET
                    );
                }
                tracing::warn!(
                    session = %ctx.session_key,
                    tool = %call.name,
                    "Tool execution blocked (repetition/loop detected)"
                );
                serde_json::json!({ "error": warning_str })
            } else if approval.forbidden {
                let reject_msg = format!(
                    "✕ *{}* - Rejected: Dangerous command is forbidden",
                    formatted_args
                );
                super::tool_execution::send_progress_update(ctx.session_key, &reject_msg).await;
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}✕ {} - Rejected: Dangerous command is forbidden{}",
                        AURA_SLATE,
                        leaf_prefix,
                        ERROR_RED,
                        formatted_args,
                        COLOR_RESET
                    );
                }
                tracing::warn!(
                    session = %ctx.session_key,
                    tool = %call.name,
                    "Tool execution forbidden by security guard"
                );
                serde_json::json!({ "error": "Execution denied by host: This command is forbidden by security rules." })
            } else if !approval.approved {
                let deny_msg = format!("✕ *{}* - Denied by user", formatted_args);
                super::tool_execution::send_progress_update(ctx.session_key, &deny_msg).await;
                if !silent {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!(
                        "{}{}{}✕ {} - Denied by user{}",
                        AURA_SLATE,
                        leaf_prefix,
                        ERROR_RED,
                        formatted_args,
                        COLOR_RESET
                    );
                }
                tracing::warn!(
                    session = %ctx.session_key,
                    tool = %call.name,
                    "Tool execution denied by user approval request"
                );
                serde_json::json!({ "error": "Execution denied by user." })
            } else {
                match loop_ref.tools.get(&call.name) {
                    Some(t) => {
                        let tool_timeout = std::time::Duration::from_secs(
                            config.agents.defaults.tool_timeout_secs,
                        );
                        let fut = t.call(&call.arguments);
                        let timed_fut = tokio::time::timeout(tool_timeout, fut);
                        // Race tool execution against CLI cancel signal
                        let tool_cancel_tx = crate::shutdown::cli_cancel_tx();
                        let mut tool_cancel_rx = tool_cancel_tx.subscribe();
                        let tool_cancel_initial = *tool_cancel_rx.borrow();
                        let cancel_aware_fut = async {
                            tokio::select! {
                                biased;
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
                                        Err(_) => Err(anyhow::anyhow!("Tool execution timed out after {}s", config.agents.defaults.tool_timeout_secs)),
                                    }
                                }
                            }
                        };
                        match with_spinner(&tool_spinner_msg, cancel_aware_fut).await {
                            Ok(res) => {
                                super::tool_execution::render_tool_success(
                                    &call,
                                    &formatted_args,
                                    ctx.session_key,
                                    silent,
                                    res,
                                )
                                .await
                            }
                            Err(e) => {
                                let error_str = e.to_string();
                                // If the tool was cancelled by user, propagate to turn-level cancellation
                                // so the next iteration breaks immediately instead of re-prompting the LLM.
                                if should_cancel_turn_after_tool_error(&error_str) {
                                    turn_cancel_clone.cancel();
                                }
                                ctx.turn_errors
                                    .push(format!("Tool {} failed: {}", call.name, error_str));
                                super::tool_execution::render_tool_failure(
                                    &call,
                                    &formatted_args,
                                    ctx.session_key,
                                    silent,
                                    &error_str,
                                )
                                .await
                            }
                        }
                    }
                    None => {
                        ctx.turn_errors
                            .push(format!("Tool {} not found", call.name));
                        super::tool_execution::render_tool_not_found(
                            &call,
                            &formatted_args,
                            ctx.session_key,
                            silent,
                        )
                        .await
                    }
                }
            };
            if let Some(err_val) = result_val.get("error").and_then(|v| v.as_str()) {
                ctx.turn_errors
                    .push(format!("Tool {} returned error: {}", call.name, err_val));
            }
            crate::agent::activity::update_activity(
                ctx.session_key,
                "Processing user prompt",
                None,
            );

            tool_results.push(super::transcript::ToolTranscriptResult {
                id: call.id.clone(),
                name: call.name.clone(),
                result: result_val,
            });

            assistant_tool_calls_json.push(serde_json::json!({
                "id": call.id,
                "type": "function",
                "function": {
                    "name": call.name,
                    "arguments": call.arguments.to_string()
                }
            }));
        }

        super::transcript::append_assistant_tool_calls(
            &mut ctx.messages,
            assistant_tool_calls_json,
            resp.reasoning_content.as_deref(),
        );

        super::transcript::append_tool_results(&mut ctx.messages, &config, tool_results).await;

        ctx.session.messages = ctx.messages.clone();
        if iterations % 5 == 0 {
            if let Err(e) = loop_ref.session_manager.save(&ctx.session).await {
                tracing::warn!("Failed to save session incrementally in Run loop: {}", e);
            }
        }

        iterations += 1;
        if should_halt {
            let halt_msg = "⚠️ Halted execution: Too many repeating tool calls blocked by loop detection. Halting to save RAM and tokens.";
            ctx.final_content = halt_msg.to_string();
            super::tool_execution::send_progress_update(ctx.session_key, halt_msg).await;
            if !crate::agent::style::spinner::is_silent() {
                crate::tui_println!("{}⚠️ {}{}", AURA_GOLD, halt_msg, COLOR_RESET);
            }
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: halt_msg.to_string(),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            });
            break;
        }
    }

    ctx.session.messages = ctx.messages.clone();
    if let Err(e) = loop_ref.session_manager.save(&ctx.session).await {
        tracing::warn!(
            "Failed to save session unconditionally on final iteration in Run loop: {}",
            e
        );
    }
    if let Some(ref inter_id) = ctx.interaction_id {
        if !ctx.turn_errors.is_empty() {
            let errors_str = ctx.turn_errors.join("\n");
            let _ =
                crate::tools::shared_memory::update_interaction_errors(inter_id, &errors_str).await;
        }
    }

    Ok(TurnState::Save)
}

fn format_markdown_line(line: &str) -> String {
    static RE_BOLD: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static RE_CODE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static RE_ITALIC: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let re_bold = RE_BOLD.get_or_init(|| regex::Regex::new(r"\*\*(.*?)\*\*").unwrap());
    let re_code = RE_CODE.get_or_init(|| regex::Regex::new(r"`(.*?)`").unwrap());
    let re_italic = RE_ITALIC.get_or_init(|| regex::Regex::new(r"\*(.*?)\*").unwrap());

    let light_blue = "\x1b[38;2;135;206;250m";

    let trimmed = line.trim();
    if trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3 && !trimmed.is_empty() {
        return format!("{}──────{}", LIGHT_WHITE, COLOR_RESET);
    }

    if line.trim_start().starts_with('#') {
        return format!("{}{}{}", HEADING_BLUE, line, COLOR_RESET);
    }

    let mut formatted = line.to_string();
    formatted = formatted
        .replace('✔', &format!("{}{}{}", EMERALD_GREEN, "✔", COLOR_RESET))
        .replace("✅", &format!("{}{}{}", EMERALD_GREEN, "✅", COLOR_RESET))
        .replace('✓', &format!("{}{}{}", EMERALD_GREEN, "✓", COLOR_RESET))
        .replace('✖', &format!("{}{}{}", ERROR_RED, "✖", COLOR_RESET))
        .replace("❌", &format!("{}{}{}", ERROR_RED, "❌", COLOR_RESET))
        .replace('✗', &format!("{}{}{}", ERROR_RED, "✗", COLOR_RESET));

    formatted = re_bold
        .replace_all(
            &formatted,
            &format!("{}{}$1{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET),
        )
        .to_string();
    formatted = re_code
        .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
        .to_string();
    formatted = re_italic
        .replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET))
        .to_string();

    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_timeout_does_not_count_as_user_cancel() {
        assert!(!should_cancel_turn_after_tool_error(
            "Tool execution timed out after 120s"
        ));
        assert!(should_cancel_turn_after_tool_error("Cancelled by user"));
        assert!(should_cancel_turn_after_tool_error(
            "Subagent task cancelled"
        ));
    }
}
