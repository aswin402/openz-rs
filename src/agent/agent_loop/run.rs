use anyhow::Result;
use std::io::Write;
use futures_util::StreamExt;
use crate::agent::style::*;
use crate::session::Message;
use crate::providers::GenerationSettings;
use super::{AgentLoop, TurnContext, TurnState};

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let mut iterations = 0;
    let mut loop_blocked_count = 0;
    let max_iterations = ctx.config.agents.defaults.max_tool_iterations;

    loop {
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
            send_progress_update(ctx.session_key, &msg).await;
            if !crate::agent::style::spinner::is_silent() {
                print!("{}⚠️ {}{}\r\n", AURA_GOLD, msg, COLOR_RESET);
                let _ = std::io::stdout().flush();
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
            let mut finish_reason = "stop".to_string();
            // Track whether we're currently in reasoning phase (for live spinner)
            let mut in_reasoning_phase = false;

            let print_reasoning = |full_reasoning: &str,
                                   in_reasoning_phase: &mut bool,
                                   reasoning_printed: &mut bool,
                                   start_time: std::time::Instant| {
                if !*reasoning_printed && !full_reasoning.is_empty() {
                    let depth = crate::tools::subagent::DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
                    if !silent && depth == 0 {
                        let elapsed = start_time.elapsed().as_secs_f32();
                        print!("\r\x1b[2K");
                        print!(
                            "{}● {}{}{}Thought for {:.1}s{}\r\n",
                            RED_ORANGE, COLOR_RESET, COLOR_BOLD, RED_ORANGE, elapsed, COLOR_RESET
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

            struct PartialToolCall {
                id: String,
                name: String,
                arguments: String,
            }
            let mut partial_tool_calls = std::collections::HashMap::<usize, PartialToolCall>::new();

            while let Some(chunk) = stream.next().await {
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
                        send_progress_update(ctx.session_key, &text).await;
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

                        let entry = partial_tool_calls.entry(index).or_insert_with(|| PartialToolCall {
                            id: String::new(),
                            name: String::new(),
                            arguments: String::new(),
                        });
                        if let Some(val) = id {
                            entry.id = val;
                        }
                        if let Some(val) = name {
                            entry.name = val;
                        }
                        if let Some(val) = arguments {
                            entry.arguments.push_str(&val);
                        }
                    }
                    crate::providers::ChatStreamChunk::Done { finish_reason: reason } => {
                        if let Some(r) = reason {
                            finish_reason = r;
                        }
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

            // Collect and parse tool calls
            let mut tool_calls = Vec::new();
            let mut sorted_keys: Vec<_> = partial_tool_calls.keys().collect();
            sorted_keys.sort();
            for k in sorted_keys {
                if let Some(ptc) = partial_tool_calls.get(k) {
                    let args_parsed = match serde_json::from_str(&ptc.arguments) {
                        Ok(parsed) => parsed,
                        Err(e) => {
                            let repaired = ptc.arguments.replace('\n', "\\n").replace('\r', "\\r");
                            serde_json::from_str(&repaired).unwrap_or_else(|_| {
                                let mut map = serde_json::Map::new();
                                map.insert(
                                    "parse_error".to_string(),
                                    serde_json::Value::String(e.to_string()),
                                );
                                serde_json::Value::Object(map)
                            })
                        }
                    };
                    tool_calls.push(crate::providers::ToolCallRequest {
                        id: ptc.id.clone(),
                        name: ptc.name.clone(),
                        arguments: args_parsed,
                    });
                }
            }

            ctx.streamed = true;

            crate::providers::LLMResponse {
                content: if full_content.is_empty() {
                    None
                } else {
                    Some(full_content)
                },
                tool_calls,
                finish_reason,
                reasoning_content: if full_reasoning.is_empty() {
                    None
                } else {
                    Some(full_reasoning)
                },
            }
        } else {
            loop_ref
                .chat_with_fallback(
                    &mut ctx.active_provider,
                    &ctx.system_prompt,
                    &ctx.messages,
                    &tools_openai,
                    &settings,
                    &activity_msg,
                )
                .await?
        };

        // Handle potential response truncation (finish_reason = "length") by auto-continuing
        if resp.finish_reason == "length" {
            let mut accumulated_content = resp.content.clone();
            let mut finish_reason = resp.finish_reason.clone();
            let mut continue_attempts = 0;

            while finish_reason == "length" && continue_attempts < 3 {
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
        if resp.content.is_none() && resp.reasoning_content.is_some() && resp.tool_calls.is_empty() {
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
        let has_reasoning = resp.reasoning_content.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
        let has_content = resp.content.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
        let has_tool_calls = !resp.tool_calls.is_empty();

        if has_reasoning || (has_content && has_tool_calls) {
            // Send reasoning/thought summary to non-CLI channels (Telegram, WS, etc.)
            if has_reasoning {
                if let Some(ref reasoning) = resp.reasoning_content {
                    let reasoning_msg = format!("▶ *Thought*\n\n> {}", reasoning.trim().replace('\n', "\n> "));
                    send_progress_update(ctx.session_key, &reasoning_msg).await;
                }
            } else if has_content && has_tool_calls {
                if let Some(ref content) = resp.content {
                    let thought_msg = format!("▶ *Thought*\n\n> {}", content.trim().replace('\n', "\n> "));
                    send_progress_update(ctx.session_key, &thought_msg).await;
                }
            }

            let silent = crate::agent::style::spinner::is_silent();
            let depth = crate::tools::subagent::DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
            if !silent && depth == 0 {
                if ctx.streamed {
                    // During streaming, the reasoning spinner was already shown and
                    // the "Thought for Xs" badge was already printed when content
                    // started arriving or when the stream finished. If no content
                    // arrived and no reasoning was printed (e.g. pure tool-call-only response),
                    // finalize the spinner and print the badge now.
                    if !content_streaming_started && !reasoning_printed {
                        print!("\r\x1b[2K");
                        print!(
                            "{}● {}{}{}Thought for {:.1}s{}\r\n",
                            RED_ORANGE, COLOR_RESET, COLOR_BOLD, RED_ORANGE, duration_secs, COLOR_RESET
                        );
                        let _ = std::io::stdout().flush();
                    }
                } else {
                    // Non-streaming path: print the badge and thinking summary
                    print!(
                        "{}● {}{}{}Thought for {:.1}s{}\r\n",
                        RED_ORANGE, COLOR_RESET, COLOR_BOLD, RED_ORANGE, duration_secs, COLOR_RESET
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
            let text_repeat = count_previous_text_responses(&ctx.messages, &content);
            if text_repeat >= 2 {
                let loop_msg = "⚠️ Halted execution: Detected repetitive text responses.";
                ctx.final_content = loop_msg.to_string();
                send_progress_update(ctx.session_key, loop_msg).await;
                if !crate::agent::style::spinner::is_silent() {
                    print!("{}⚠️ {}{}\r\n", AURA_GOLD, loop_msg, COLOR_RESET);
                    let _ = std::io::stdout().flush();
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
            ctx.tools_used.push(call.name.clone());

            crate::agent::activity::update_activity(ctx.session_key, "Executing tool", Some(&call.name));
            let silent = crate::agent::style::spinner::is_silent();
            let formatted_args = format_tool_args(&call.name, &call.arguments);
            let tool_spinner_msg = crate::agent::style::get_tree_spinner_msg(&call.name, &formatted_args);

            let tool_msg = format!("▸ Running *{}*...", formatted_args);
            send_progress_update(ctx.session_key, &tool_msg).await;

            if !silent {
                crate::agent::style::print_tree_tool_start(&call.name, &formatted_args);
            }

            tracing::info!(
                session = %ctx.session_key,
                tool = %call.name,
                arguments = %call.arguments,
                "Executing tool call"
            );
            let mut approved = true;
            let mut forbidden = false;
            let security_mode = &config.agents.defaults.security_mode;

            let parse_error = call.arguments.get("parse_error").and_then(|v| v.as_str());

            let repeat_count = count_previous_tool_calls(&ctx.messages, &call.name, &call.arguments);
            let is_loop = repeat_count >= 2;
            if is_loop && parse_error.is_none() {
                loop_blocked_count += 1;
                if loop_blocked_count >= 3 {
                    should_halt = true;
                }
            }

            if parse_error.is_none()
                && crate::agent::security::SecurityGuard::is_forbidden(&call.name, &call.arguments)
            {
                forbidden = true;
            } else if parse_error.is_none()
                && !is_loop
                && crate::agent::security::SecurityGuard::is_sensitive_with_mode(
                    &call.name,
                    &call.arguments,
                    security_mode,
                )
            {
                // Clear the running tool spinner first so the prompt is clean
                if !silent {
                    print!("\r\x1b[2K");
                    let _ = std::io::stdout().flush();
                }

                match crate::agent::security::ask_approval(ctx.session_key, &call.name, &call.arguments).await {
                    Ok(app) => approved = app,
                    Err(_) => approved = false,
                }
            }

            let result_val = if let Some(err_msg) = parse_error {
                let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, err_msg);
                send_progress_update(ctx.session_key, &fail_msg).await;
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
                ctx.turn_errors
                    .push(format!("Tool {} arguments parse error: {}", call.name, err_msg));
                serde_json::json!({ "error": err_msg })
            } else if is_loop {
                let warning_str = format!(
                    "Loop detected: You have already executed the tool '{}' with these exact arguments {} times in this turn. To prevent infinite loops, execution was blocked. Do NOT call this tool again. Analyze previous tool outputs and use a different strategy, or finish your response.",
                    call.name, repeat_count
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
            } else if forbidden {
                let reject_msg = format!("✕ *{}* - Rejected: Dangerous command is forbidden", formatted_args);
                send_progress_update(ctx.session_key, &reject_msg).await;
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
            } else if !approved {
                let deny_msg = format!("✕ *{}* - Denied by user", formatted_args);
                send_progress_update(ctx.session_key, &deny_msg).await;
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
                        let tool_timeout =
                            std::time::Duration::from_secs(config.agents.defaults.tool_timeout_secs);
                        let fut = t.call(&call.arguments);
                        let timed_fut = tokio::time::timeout(tool_timeout, fut);
                        match with_spinner(&tool_spinner_msg, timed_fut).await {
                            Ok(Ok(res)) => {
                                let success_msg = format!("✓ *{}*", formatted_args);
                                send_progress_update(ctx.session_key, &success_msg).await;
                                if !silent
                                    && !crate::agent::style::is_profile_subagent(&call.name)
                                    && call.name != "parallel_research"
                                {
                                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                                    let summary = crate::agent::style::format_tool_outcome_summary(
                                        &call.name,
                                        &call.arguments,
                                        &res,
                                    );
                                    if call.name == "write_file"
                                        || call.name == "patch_file"
                                        || call.name == "replace_lines"
                                    {
                                        crate::tui_println!(
                                            "{}{}{}{}",
                                            AURA_SLATE,
                                            leaf_prefix,
                                            COLOR_RESET,
                                            summary
                                        );
                                    } else if summary.contains('\u{2713}') || summary.contains('\u{2715}') {
                                        crate::tui_println!(
                                            "{}{}{}{}",
                                            AURA_SLATE,
                                            leaf_prefix,
                                            COLOR_RESET,
                                            summary
                                        );
                                    } else {
                                        crate::tui_println!(
                                            "{}{}{}✓ {}{}",
                                            AURA_SLATE,
                                            leaf_prefix,
                                            AURA_GREEN,
                                            summary,
                                            COLOR_RESET
                                        );
                                    }
                                }
                                tracing::info!(
                                    session = %ctx.session_key,
                                    tool = %call.name,
                                    status = "success",
                                    "Tool call completed"
                                );
                                tracing::debug!(
                                    session = %ctx.session_key,
                                    tool = %call.name,
                                    result = %res,
                                    "Tool output result"
                                );
                                res
                            }
                            Ok(Err(e)) => {
                                let error_str = e.to_string();
                                ctx.turn_errors
                                    .push(format!("Tool {} failed: {}", call.name, error_str));
                                let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, error_str);
                                send_progress_update(ctx.session_key, &fail_msg).await;
                                if !silent
                                    && !crate::agent::style::is_profile_subagent(&call.name)
                                    && call.name != "parallel_research"
                                {
                                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                                    crate::tui_println!(
                                        "{}{}{}✕ {}{}",
                                        AURA_SLATE,
                                        leaf_prefix,
                                        AURA_ROSE,
                                        error_str,
                                        COLOR_RESET
                                    );
                                }
                                tracing::error!(
                                    session = %ctx.session_key,
                                    tool = %call.name,
                                    error = %error_str,
                                    "Tool call failed"
                                );
                                let hint = generate_self_healing_hint(&call.name, &error_str);
                                serde_json::json!({
                                    "error": error_str,
                                    "self_healing_suggestion": hint
                                })
                            }
                            Err(_) => {
                                let timeout_msg = format!(
                                    "Tool '{}' timed out after {}s",
                                    call.name, config.agents.defaults.tool_timeout_secs
                                );
                                ctx.turn_errors.push(timeout_msg.clone());
                                let fail_msg = format!("⏱️ *{}* - Timed out", formatted_args);
                                send_progress_update(ctx.session_key, &fail_msg).await;
                                if !silent
                                    && !crate::agent::style::is_profile_subagent(&call.name)
                                    && call.name != "parallel_research"
                                {
                                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                                    crate::tui_println!(
                                        "{}{}{}✕ timeout after {}s{}",
                                        AURA_SLATE,
                                        leaf_prefix,
                                        AURA_ROSE,
                                        config.agents.defaults.tool_timeout_secs,
                                        COLOR_RESET
                                    );
                                }
                                tracing::error!(
                                    session = %ctx.session_key,
                                    tool = %call.name,
                                    "Tool call timed out"
                                );
                                serde_json::json!({
                                    "error": timeout_msg,
                                    "hint": "The tool exceeded the time limit. Try a more specific query, a smaller scope, or break the task into steps."
                                })
                            }
                        }
                    }
                    None => {
                        let error_str = format!("Tool '{}' not found", call.name);
                        ctx.turn_errors
                            .push(format!("Tool {} not found", call.name));
                        let fail_msg = format!("✕ *{}* - Failed: {}", formatted_args, error_str);
                        send_progress_update(ctx.session_key, &fail_msg).await;
                        if !silent {
                            let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                            crate::tui_println!(
                                "{}{}{}✗{} {} - Failed: {}{}",
                                AURA_SLATE,
                                leaf_prefix,
                                COLOR_RESET,
                                AURA_ROSE,
                                formatted_args,
                                error_str,
                                COLOR_RESET
                            );
                        }
                        let hint = generate_self_healing_hint(&call.name, &error_str);
                        serde_json::json!({
                            "error": error_str,
                            "self_healing_suggestion": hint
                        })
                    }
                }
            };
            if let Some(err_val) = result_val.get("error").and_then(|v| v.as_str()) {
                ctx.turn_errors
                    .push(format!("Tool {} returned error: {}", call.name, err_val));
            }
            crate::agent::activity::update_activity(ctx.session_key, "Processing user prompt", None);

            tool_results.push((call.id.clone(), call.name.clone(), result_val));

            assistant_tool_calls_json.push(serde_json::json!({
                "id": call.id,
                "type": "function",
                "function": {
                    "name": call.name,
                    "arguments": call.arguments.to_string()
                }
            }));
        }

        if let Some(last_msg) = ctx.messages.last_mut() {
            if last_msg.role == "assistant" {
                last_msg
                    .extra
                    .insert("tool_calls".to_string(), serde_json::Value::Array(assistant_tool_calls_json));
                if let Some(ref reasoning) = resp.reasoning_content {
                    last_msg
                        .extra
                        .insert("reasoning_content".to_string(), serde_json::Value::String(reasoning.clone()));
                }
            } else {
                let mut extra = serde_json::Map::new();
                extra.insert("tool_calls".to_string(), serde_json::Value::Array(assistant_tool_calls_json));
                if let Some(ref reasoning) = resp.reasoning_content {
                    extra.insert("reasoning_content".to_string(), serde_json::Value::String(reasoning.clone()));
                }
                ctx.messages.push(Message {
                    role: "assistant".to_string(),
                    content: String::new(),
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra,
                });
            }
        } else {
            let mut extra = serde_json::Map::new();
            extra.insert("tool_calls".to_string(), serde_json::Value::Array(assistant_tool_calls_json));
            if let Some(ref reasoning) = resp.reasoning_content {
                extra.insert("reasoning_content".to_string(), serde_json::Value::String(reasoning.clone()));
            }
            ctx.messages.push(Message {
                role: "assistant".to_string(),
                content: String::new(),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra,
            });
        }

        for (id, name, result) in tool_results {
            let mut extra = serde_json::Map::new();
            extra.insert("tool_call_id".to_string(), serde_json::Value::String(id));
            extra.insert("name".to_string(), serde_json::Value::String(name.clone()));

            let content_str = result.to_string();
            let limit = config.agents.defaults.tool_output_limit.unwrap_or(4000);
            let is_retrieve = name == "retrieve_original" || name == "headroom/retrieve_original";
            let content = if content_str.len() > limit && !is_retrieve {
                let outputs_dir = crate::config::resolve_path("~/.openz/tool_outputs");
                if let Err(e) = tokio::fs::create_dir_all(&outputs_dir).await {
                    tracing::warn!("Failed to create tool outputs directory '{}': {}", outputs_dir.display(), e);
                }
                let file_name = format!("output_{}_{}.json", name, uuid::Uuid::new_v4());
                let file_path = outputs_dir.join(file_name);
                if let Err(e) = tokio::fs::write(&file_path, &content_str).await {
                    tracing::warn!("Failed to write tool output file '{}': {}", file_path.display(), e);
                }

                let compressed = crate::agent::context_compactor::compress_tool_output(&name, &content_str);
                format!(
                    "{}\n\n... [TRUNCATED - Full output saved for reference at file://{}] ...",
                    compressed,
                    file_path.to_string_lossy()
                )
            } else {
                content_str
            };

            ctx.messages.push(Message {
                role: "tool".to_string(),
                content,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra,
            });
        }

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
            send_progress_update(ctx.session_key, halt_msg).await;
            if !crate::agent::style::spinner::is_silent() {
                print!("{}⚠️ {}{}\r\n", AURA_GOLD, halt_msg, COLOR_RESET);
                let _ = std::io::stdout().flush();
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
        tracing::warn!("Failed to save session unconditionally on final iteration in Run loop: {}", e);
    }
    if let Some(ref inter_id) = ctx.interaction_id {
        if !ctx.turn_errors.is_empty() {
            let errors_str = ctx.turn_errors.join("\n");
            let _ = crate::tools::shared_memory::update_interaction_errors(inter_id, &errors_str).await;
        }
    }

    Ok(TurnState::Save)
}

fn format_tool_args(name: &str, args: &serde_json::Value) -> String {
    let friendly_name = match name {
        "grep_search" => "Search",
        "read_file" | "view_file" => "Read",
        "write_file" | "write_to_file" | "replace_file_content" | "multi_replace_file_content" => "Edit",
        "run_command" | "exec_command" => "Bash",
        "list_dir" => "ListDir",
        "code_outline" => "Outline",
        "ast_grep" => "AstGrep",
        "git_manager" => "Git",
        "cargo_manager" => "Cargo",
        "web_search" => "WebSearch",
        "gsd_browser" => "Browser",
        "clipboard" => "Clipboard",
        "open_path" | "open" => "Open",
        "web_fetch" | "read_url_content" | "read_url" => "Fetch",
        "generate_image" => "Image",
        "generate_video" => "Video",
        "html_to_video" => "HtmlVideo",
        "create_animated_svg" | "svg_animator" => "SvgAnim",
        "obscura_browser" => "Obscura",
        "db_inspector" => "DbInspect",
        "db_write" => "DbWrite",
        "read_doc" => "DocRead",
        "crawl" => "Crawl",
        "semantic_search" => "SemanticSearch",
        "wasm_sandbox" => "Wasm",
        "cron" => "Cron",
        "watcher" => "Watcher",
        other => other,
    };

    let details = if let serde_json::Value::Object(map) = args {
        if name == "grep_search" {
            if let Some(q) = map.get("query").or_else(|| map.get("Query")).and_then(|v| v.as_str()) {
                if q.len() > 35 {
                    format!("query: \"{}...\"", q.chars().take(32).collect::<String>())
                } else {
                    format!("query: \"{}\"", q)
                }
            } else {
                String::new()
            }
        } else if name == "read_file" || name == "view_file" {
            if let Some(path) = map.get("Path").or_else(|| map.get("AbsolutePath")).and_then(|v| v.as_str()) {
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "write_file"
            || name == "write_to_file"
            || name == "replace_file_content"
            || name == "multi_replace_file_content"
        {
            if let Some(path) = map.get("TargetFile").or_else(|| map.get("Path")).and_then(|v| v.as_str()) {
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "run_command" || name == "exec_command" {
            if let Some(cmd) = map
                .get("CommandLine")
                .or_else(|| map.get("Command"))
                .or_else(|| map.get("command"))
                .or_else(|| map.get("command_line"))
                .and_then(|v| v.as_str())
            {
                let first_line = cmd.lines().next().unwrap_or("").trim();
                if first_line.len() > 40 {
                    format!("{}...", first_line.chars().take(37).collect::<String>())
                } else {
                    first_line.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "list_dir" {
            if let Some(path) = map.get("DirectoryPath").or_else(|| map.get("Path")).and_then(|v| v.as_str()) {
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    path.to_string()
                }
            } else {
                String::new()
            }
        } else if name == "git_manager" {
            if let Some(action) = map.get("Action").and_then(|v| v.as_str()) {
                action.to_string()
            } else {
                String::new()
            }
        } else if name == "cargo_manager" {
            if let Some(command) = map.get("Command").and_then(|v| v.as_str()) {
                command.to_string()
            } else {
                String::new()
            }
        } else if name == "web_search" {
            if let Some(q) = map.get("query").or_else(|| map.get("Query")).and_then(|v| v.as_str()) {
                if q.len() > 35 {
                    format!("query: \"{}...\"", q.chars().take(32).collect::<String>())
                } else {
                    format!("query: \"{}\"", q)
                }
            } else {
                String::new()
            }
        } else if name == "web_fetch" || name == "read_url_content" || name == "read_url" {
            if let Some(url) = map.get("Url").or_else(|| map.get("url")).and_then(|v| v.as_str()) {
                if url.len() > 35 {
                    format!("\"{}...\"", url.chars().take(32).collect::<String>())
                } else {
                    format!("\"{}\"", url)
                }
            } else {
                String::new()
            }
        } else if name == "generate_image" {
            let path = map
                .get("output_path")
                .or_else(|| map.get("ImageName"))
                .and_then(|v| v.as_str())
                .unwrap_or("output.png");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            let shapes_count = map.get("shapes").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
            if shapes_count > 0 {
                format!("output: \"{}\", shapes: {}", filename, shapes_count)
            } else {
                format!("output: \"{}\"", filename)
            }
        } else if name == "generate_video" {
            let path = map.get("output_path").and_then(|v| v.as_str()).unwrap_or("output.mp4");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            format!("output: \"{}\"", filename)
        } else if name == "html_to_video" {
            let html_path = map.get("html_path").and_then(|v| v.as_str()).unwrap_or("");
            let html_filename = if html_path.starts_with("http://") || html_path.starts_with("https://") {
                if html_path.len() > 30 {
                    format!("{}...", html_path.chars().take(27).collect::<String>())
                } else {
                    html_path.to_string()
                }
            } else {
                std::path::Path::new(html_path)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| html_path.to_string())
            };
            let out_path = map.get("output_path").and_then(|v| v.as_str()).unwrap_or("output.mp4");
            let out_filename = std::path::Path::new(out_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| out_path.to_string());
            format!("html: \"{}\", output: \"{}\"", html_filename, out_filename)
        } else if name == "create_animated_svg" {
            let path = map.get("output_path").and_then(|v| v.as_str()).unwrap_or("output.svg");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            let elem_count = map.get("elements").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
            let anim_count: usize = map
                .get("elements")
                .and_then(|v| v.as_array())
                .map(|elems| {
                    elems
                        .iter()
                        .map(|e| {
                            e.get("animations")
                                .and_then(|a| a.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0)
                        })
                        .sum()
                })
                .unwrap_or(0);
            if elem_count > 0 {
                format!(
                    "output: \"{}\", elements: {}, animations: {}",
                    filename, elem_count, anim_count
                )
            } else {
                format!("output: \"{}\"", filename)
            }
        } else if name == "obscura_browser" {
            let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("render");
            let truncated_url = if url.len() > 30 {
                format!("{}...", url.chars().take(27).collect::<String>())
            } else {
                url.to_string()
            };
            format!("action: \"{}\", url: \"{}\"", action, truncated_url)
        } else if name == "gsd_browser" {
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
            let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let ref_id = map.get("ref_id").and_then(|v| v.as_str()).unwrap_or("");
            if !url.is_empty() {
                let truncated_url = if url.len() > 30 {
                    format!("{}...", url.chars().take(27).collect::<String>())
                } else {
                    url.to_string()
                };
                format!("action: \"{}\", url: \"{}\"", action, truncated_url)
            } else if !ref_id.is_empty() {
                format!("action: \"{}\", ref_id: \"{}\"", action, ref_id)
            } else {
                format!("action: \"{}\"", action)
            }
        } else if name == "ast_grep" {
            if let Some(pattern) = map.get("pattern").and_then(|v| v.as_str()) {
                if pattern.len() > 35 {
                    format!("\"{}...\"", pattern.chars().take(32).collect::<String>())
                } else {
                    format!("\"{}\"", pattern)
                }
            } else {
                String::new()
            }
        } else if name == "crawl" {
            let url = map.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url.len() > 30 {
                format!("url: \"{}...\"", url.chars().take(27).collect::<String>())
            } else {
                format!("url: \"{}\"", url)
            }
        } else if name == "semantic_search" {
            let query = map.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if query.len() > 35 {
                format!("query: \"{}...\"", query.chars().take(32).collect::<String>())
            } else {
                format!("query: \"{}\"", query)
            }
        } else if name == "doc_reader" {
            let path = map.get("file_path").or_else(|| map.get("Path")).and_then(|v| v.as_str()).unwrap_or("");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            format!("file: \"{}\"", filename)
        } else if name == "wasm_sandbox" {
            let path = map.get("wasm_path").and_then(|v| v.as_str()).unwrap_or("");
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            format!("wasm: \"{}\"", filename)
        } else if name == "cron" {
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
            format!("action: \"{}\"", action)
        } else if name == "watcher" {
            let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
            format!("action: \"{}\"", action)
        } else if name == "db_inspector" || name == "db_write" {
            let db_path = map.get("db_path").and_then(|v| v.as_str()).unwrap_or("");
            let db_filename = std::path::Path::new(db_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| db_path.to_string());
            let sql = map.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            if !sql.is_empty() {
                let truncated_sql = if sql.len() > 35 {
                    format!("{}...", sql.chars().take(32).collect::<String>())
                } else {
                    sql.to_string()
                };
                format!("db: \"{}\", sql: \"{}\"", db_filename, truncated_sql)
            } else {
                let action = map.get("action").and_then(|v| v.as_str()).unwrap_or("");
                format!("db: \"{}\", action: \"{}\"", db_filename, action)
            }
        } else {
            let mut parts = Vec::new();
            for (k, v) in map {
                if k == "session_key" || k == "session_id" {
                    continue;
                }
                let val_str = match v {
                    serde_json::Value::String(s) => {
                        if s.len() > 20 {
                            format!("\"{}...\"", s.chars().take(17).collect::<String>())
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    other => {
                        let os = other.to_string();
                        if os.len() > 20 {
                            format!("{}...", os.chars().take(17).collect::<String>())
                        } else {
                            os
                        }
                    }
                };
                parts.push(format!("{}: {}", k, val_str));
            }
            let joined = parts.join(", ");
            if joined.len() > 50 {
                format!("{}...", joined.chars().take(47).collect::<String>())
            } else {
                joined
            }
        }
    } else {
        let as_str = args.to_string();
        if as_str.len() > 50 {
            format!("{}...", as_str.chars().take(47).collect::<String>())
        } else {
            as_str
        }
    };

    if details.is_empty() {
        format!("{}{}{}", COLOR_BOLD, friendly_name, COLOR_RESET)
    } else {
        format!("{}{}{}({})", COLOR_BOLD, friendly_name, COLOR_RESET, details)
    }
}

async fn send_progress_update(session_key: &str, text: &str) {
    let actual_session =
        crate::agent::style::spinner::get_current_session_key().unwrap_or_else(|| session_key.to_string());
    if actual_session.starts_with("telegram:") {
        if let Some(chat_id_str) = actual_session.strip_prefix("telegram:") {
            if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                if let Some((bot_token, client)) = crate::channels::telegram::get_telegram_bot_info() {
                    let send_url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
                    let payload = serde_json::json!({
                        "chat_id": chat_id,
                        "text": text,
                        "parse_mode": "Markdown"
                    });
                    let _ = client.post(&send_url).json(&payload).send().await;
                }
            }
        }
    } else if actual_session.starts_with("discord:") {
        if let Some(channel_id) = actual_session.strip_prefix("discord:") {
            if let Some((bot_token, client)) = crate::channels::discord::get_discord_bot_info() {
                let send_url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);
                let payload = serde_json::json!({
                    "content": text
                });
                let _ = client
                    .post(&send_url)
                    .header("Authorization", format!("Bot {}", bot_token))
                    .json(&payload)
                    .send()
                    .await;
            }
        }
    } else if actual_session.starts_with("whatsapp:") {
        if let Some(phone_number) = actual_session.strip_prefix("whatsapp:") {
            if let Some((api_key, phone_number_id, client)) = crate::channels::whatsapp::get_whatsapp_bot_info() {
                let send_url = format!("https://graph.facebook.com/v18.0/{}/messages", phone_number_id);
                let payload = serde_json::json!({
                    "messaging_product": "whatsapp",
                    "recipient_type": "individual",
                    "to": phone_number,
                    "type": "text",
                    "text": {
                        "body": text
                    }
                });
                let _ = client.post(&send_url).bearer_auth(api_key).json(&payload).send().await;
            }
        }
    }
}

fn generate_self_healing_hint(tool_name: &str, error_str: &str) -> String {
    let err_lower = error_str.to_lowercase();
    let tool_lower = tool_name.to_lowercase();

    if tool_lower.contains("read")
        || tool_lower.contains("write")
        || tool_lower.contains("patch")
        || tool_lower.contains("replace")
        || tool_lower.contains("file")
    {
        if err_lower.contains("notfound") || err_lower.contains("no such file") {
            return "The target path does not exist. Ensure the file path is correct and absolute. You can use 'list_dir' or 'find_files' to check the folder contents.".to_string();
        }
        if err_lower.contains("permission") || err_lower.contains("denied") {
            return "Permission denied. The agent process does not have access to read/write this path. Ensure you are targeting files within the permitted workspace folder.".to_string();
        }
    }

    if tool_lower.contains("exec") || tool_lower.contains("shell") || tool_lower.contains("command") {
        if err_lower.contains("permission") || err_lower.contains("denied") {
            return "Execution permission denied. You may need to make the file executable via 'chmod +x <path>' or run the script using an explicit interpreter (e.g. 'bash <script_path>').".to_string();
        }
        if err_lower.contains("not found") || err_lower.contains("127") || err_lower.contains("no such file") {
            return "Command or script executable not found. Verify the path or binary name is correct and check if the required tool is installed on the system.".to_string();
        }
        if err_lower.contains("seccomp")
            || err_lower.contains("sandbox")
            || err_lower.contains("operation not permitted")
        {
            return "Operation blocked by the seccomp BPF sandbox. Note that networking syscalls (e.g. curl, wget, git push), mount/umount, and other privileged actions are forbidden in the sandboxed environment. Please perform the action without network or sandbox-restricted system calls, or run locally via a different approved script if possible.".to_string();
        }
    }

    if err_lower.contains("mcp")
        || err_lower.contains("connection")
        || err_lower.contains("broken pipe")
        || err_lower.contains("bridge")
    {
        return "MCP server connection error. The MCP server process might be offline or failed to initialize. Try using the 'manage_mcp' tool to list, configure, or restart the active MCP servers.".to_string();
    }

    if tool_lower.contains("delegate")
        || tool_lower.contains("research")
        || tool_lower.contains("optimizer")
        || tool_lower.contains("loop")
    {
        return "Subagent execution encountered an error. You can use the 'optimize_subagent' tool to refine the subagent system prompt/instructions to handle the issue better, or try breaking the goal down into smaller, simpler tasks for subagent delegation.".to_string();
    }

    // Default suggestion
    "Please double-check the arguments format, verify the target file paths or command options exist, and try a different tool or approach if this error persists.".to_string()
}

fn count_previous_tool_calls(messages: &[Message], tool_name: &str, tool_args: &serde_json::Value) -> usize {
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let mut count = 0;
    for msg in &messages[last_user_idx..] {
        if msg.role == "assistant" {
            if let Some(tool_calls) = msg.extra.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tool_calls {
                    let name = tc
                        .get("name")
                        .and_then(|v| v.as_str())
                        .or_else(|| tc.get("function").and_then(|f| f.get("name")).and_then(|v| v.as_str()));
                    let args = tc
                        .get("arguments")
                        .or_else(|| tc.get("function").and_then(|f| f.get("arguments")));

                    if let (Some(name_str), Some(args_val)) = (name, args) {
                        if name_str == tool_name {
                            let match_args = if args_val.is_string() {
                                if let Ok(parsed) =
                                    serde_json::from_str::<serde_json::Value>(args_val.as_str().unwrap())
                                {
                                    parsed == *tool_args
                                } else {
                                    false
                                }
                            } else {
                                args_val == tool_args
                            };
                            if match_args {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    count
}

fn count_previous_text_responses(messages: &[Message], next_content: &str) -> usize {
    if next_content.trim().is_empty() {
        return 0;
    }
    let last_user_idx = messages.iter().rposition(|m| m.role == "user").unwrap_or(0);
    let mut count = 0;
    let next_trimmed = next_content.trim();
    for msg in &messages[last_user_idx..] {
        if msg.role == "assistant" && !msg.content.trim().is_empty() && msg.content.trim() == next_trimmed {
            count += 1;
        }
    }
    count
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
        .replace_all(&formatted, &format!("{}{}$1{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET))
        .to_string();
    formatted = re_code.replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET)).to_string();
    formatted = re_italic.replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET)).to_string();

    formatted
}
