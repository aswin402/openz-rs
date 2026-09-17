use super::{AgentLoop, TurnContext, TurnState};
use crate::agent::style::{AURA_GOLD, COLOR_RESET, RED_ORANGE};
use crate::providers::GenerationSettings;
use crate::session::Message;
use anyhow::Result;
use chrono::Utc;
use serde_json::{Map, Value};

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let config = &ctx.config;
    let max_msgs = config.agents.defaults.max_messages;
    let len = ctx.session.messages.len();

    let estimated_tokens = crate::providers::DynamicContextRegistry::estimate_total_tokens(
        &ctx.session.messages,
        &ctx.system_prompt,
    );
    let should_compact_tokens = crate::providers::DynamicContextRegistry::should_compact_context(
        &config.agents.defaults.model,
        estimated_tokens,
        config,
    );
    let should_compact = should_compact_tokens || len > max_msgs;

    if should_compact && len > 5 {
        let keep_msgs =
            crate::providers::DynamicContextRegistry::resolve_keep_recent_count(len, config);
        let mut k = len.saturating_sub(keep_msgs);

        // Find the nearest "user" message by scanning backwards.
        // This ensures the kept history slice always starts with a "user" message,
        // and prevents orphaned "tool" messages from causing API errors.
        while k > 0 && ctx.session.messages[k].role != "user" {
            k -= 1;
        }

        if k == 0 {
            tracing::warn!(session = %ctx.session_key, "No user message found to split on. Forcing truncation at half the messages (k = {}).", len / 2);
            k = len / 2;
        }

        if k > 0 && k < len {
            let messages_to_summarize = ctx.session.messages[0..k].to_vec();

            // HERMES PATTERN: Pre-compression memory consolidation executed FIRST
            // to extract and preserve durable facts, user preferences, and decisions
            // before any messages are pruned or lossily summarized.
            let existing_memory = ctx
                .session
                .metadata
                .get("memory")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let system_prompt_mem = "You are a specialized Memory Manager. Your task is to extract and update the durable long-term memory of key facts, user preferences, decisions, and modified files based on the conversation history about to be pruned.\n\nIncorporate new facts into existing memory, remove contradicted information, and return a clean, consolidated Markdown list of memory facts.";
            let mut mem_prompt_content = String::new();
            if !existing_memory.is_empty() {
                mem_prompt_content.push_str(&format!("Existing memory:\n{}\n\n", existing_memory));
            }
            mem_prompt_content
                .push_str("Pre-compression conversation history to preserve facts/decisions from:\n");
            for msg in &messages_to_summarize {
                mem_prompt_content.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
            }

            let settings = GenerationSettings {
                temperature: 0.1,
                max_tokens: 1024,
                reasoning_effort: None,
            };

            let mem_msgs = vec![Message {
                role: "user".to_string(),
                content: mem_prompt_content,
                timestamp: Some(Utc::now().to_rfc3339()),
                extra: Map::new(),
            }];

            let spinner_msg_mem = format!(
                "{}▸ Preserving memory before context compaction...{}",
                RED_ORANGE, COLOR_RESET
            );
            match loop_ref
                .chat_with_fallback(
                    &mut ctx.active_provider,
                    system_prompt_mem,
                    &mem_msgs,
                    &[],
                    &settings,
                    &spinner_msg_mem,
                )
                .await
            {
                Ok(resp) => {
                    if let Some(new_memory) = resp.content {
                        tracing::info!(session = %ctx.session_key, "Preserved durable memory ({} chars)", new_memory.len());
                        ctx.session
                            .metadata
                            .insert("memory".to_string(), Value::String(new_memory));
                    }
                }
                Err(e) => {
                    let silent = crate::agent::style::spinner::is_silent();
                    tracing::error!(session = %ctx.session_key, "Failed to preserve pre-compression memory: {}", e);
                    if !silent {
                        crate::tui_println!(
                            "{}▲ Failed to preserve pre-compression memory: {}{}",
                            AURA_GOLD,
                            e,
                            COLOR_RESET
                        );
                    }
                }
            }

            // Generate rolling summary of conversation history
            let existing_summary = ctx
                .session
                .metadata
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let system_prompt_sum = "You are a helpful assistant. Generate a consolidated summary of the conversation history. Keep it concise, clear, and focused on key facts, decisions, and files created/modified.";
            let mut prompt_content = String::new();
            if !existing_summary.is_empty() {
                prompt_content.push_str(&format!("Previous summary:\n{}\n\n", existing_summary));
            }
            prompt_content.push_str("New conversation history to summarize:\n");
            for msg in &messages_to_summarize {
                prompt_content.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
            }

            let summary_msgs = vec![Message {
                role: "user".to_string(),
                content: prompt_content,
                timestamp: Some(Utc::now().to_rfc3339()),
                extra: Map::new(),
            }];

            let spinner_msg_sum = format!(
                "{}▸ Consolidating conversation context...{}",
                RED_ORANGE, COLOR_RESET
            );
            tracing::info!(
                session = %ctx.session_key,
                "Compacting history ({} msgs, ~{} estimated tokens, model: {})...",
                len, estimated_tokens, config.agents.defaults.model
            );
            match loop_ref
                .chat_with_fallback(
                    &mut ctx.active_provider,
                    system_prompt_sum,
                    &summary_msgs,
                    &[],
                    &settings,
                    &spinner_msg_sum,
                )
                .await
            {
                Ok(resp) => {
                    if let Some(new_summary) = resp.content {
                        tracing::info!(session = %ctx.session_key, "Compacted summary length: {} chars", new_summary.len());
                        ctx.session
                            .metadata
                            .insert("summary".to_string(), Value::String(new_summary));
                    }
                }
                Err(e) => {
                    tracing::error!(session = %ctx.session_key, "Failed to compact conversation history: {}", e);
                    if !crate::agent::style::spinner::is_silent() {
                        crate::tui_println!(
                            "{}▲ Failed to summarize conversation history: {}{}",
                            AURA_GOLD,
                            e,
                            COLOR_RESET
                        );
                    }
                }
            }

            ctx.session.messages = ctx.session.messages[k..].to_vec();
        } else {
            let keep =
                crate::providers::DynamicContextRegistry::resolve_keep_recent_count(len, config);
            let mut k = len.saturating_sub(keep);
            while k > 0 && ctx.session.messages[k].role != "user" {
                k -= 1;
            }
            ctx.session.messages = ctx.session.messages[k..].to_vec();
        }
    }
    Ok(TurnState::Command)
}
