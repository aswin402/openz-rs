use super::{AgentLoop, TurnContext, TurnState};
use crate::agent::style::{AURA_GOLD, COLOR_RESET, RED_ORANGE};
use crate::providers::GenerationSettings;
use crate::session::Message;
use crate::tools::arguments::PATH_KEYS;
use anyhow::Result;
use chrono::Utc;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

fn extract_tool_call_name(tc: &Value) -> Option<&str> {
    tc.get("name").and_then(Value::as_str).or_else(|| {
        tc.get("function")
            .and_then(|f| f.get("name"))
            .and_then(Value::as_str)
    })
}

fn extract_tool_call_args_object(tc: &Value) -> Option<Map<String, Value>> {
    let raw = tc
        .get("arguments")
        .or_else(|| tc.get("function").and_then(|f| f.get("arguments")))?;
    match raw {
        Value::Object(map) => Some(map.clone()),
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .and_then(|v| v.as_object().cloned()),
        _ => None,
    }
}

fn is_archive_token_delimiter(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | '`'
        )
}

fn extract_archive_references(text: &str) -> Vec<String> {
    text.split(is_archive_token_delimiter)
        .map(|tok| tok.trim_end_matches('.'))
        .filter(|tok| {
            !tok.is_empty()
                && (tok.contains("tool_outputs/")
                    || (tok.starts_with("ccr_") && tok.len() > 4))
        })
        .map(ToString::to_string)
        .collect()
}

pub(crate) fn extract_working_state_ledger(messages: &[Message]) -> String {
    let mut touched_files = BTreeSet::new();
    let mut mutating_tools = BTreeSet::new();
    let mut archived_outputs = BTreeSet::new();

    for msg in messages {
        match msg.role.as_str() {
            "assistant" => {
                let calls = msg
                    .extra
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for tc in calls {
                    let _ = extract_tool_call_name(&tc).map(|name| {
                        let _ = (crate::tools::metadata::is_state_changing_tool(name)
                            && mutating_tools.len() < 12)
                            .then(|| mutating_tools.insert(name.to_string()));
                    });
                    let _ = extract_tool_call_args_object(&tc).map(|args_map| {
                        let path_opt = PATH_KEYS
                            .iter()
                            .find_map(|k| args_map.get(*k).and_then(Value::as_str))
                            .map(str::trim)
                            .filter(|p| !p.is_empty());
                        let _ = path_opt.map(|path| {
                            let _ = (touched_files.len() < 16)
                                .then(|| touched_files.insert(path.to_string()));
                        });
                    });
                }
            }
            "tool" => {
                for archive_ref in extract_archive_references(&msg.content) {
                    let _ = (archived_outputs.len() < 10)
                        .then(|| archived_outputs.insert(archive_ref));
                }
            }
            _ => {}
        }
    }

    let has_entries = (!touched_files.is_empty())
        || (!mutating_tools.is_empty())
        || (!archived_outputs.is_empty());
    match has_entries {
        false => String::new(),
        true => {
            let mut lines = vec!["[Compacted Working State Ledger]".to_string()];
            let _ = (!touched_files.is_empty()).then(|| {
                let list: Vec<String> = touched_files.into_iter().collect();
                lines.push(format!("- Touched files: {}", list.join(", ")));
            });
            let _ = (!mutating_tools.is_empty()).then(|| {
                let list: Vec<String> = mutating_tools.into_iter().collect();
                lines.push(format!("- Executed state-changing tools: {}", list.join(", ")));
            });
            let _ = (!archived_outputs.is_empty()).then(|| {
                let list: Vec<String> = archived_outputs.into_iter().collect();
                lines.push(format!(
                    "- Archived tool outputs (use retrieve_original): {}",
                    list.join(", ")
                ));
            });
            lines.join("\n")
        }
    }
}

pub(crate) fn merge_summary_with_ledger(summary: &str, ledger: &str) -> String {
    let trimmed_summary = summary.trim();
    let trimmed_ledger = ledger.trim();
    match (trimmed_summary.is_empty(), trimmed_ledger.is_empty()) {
        (true, true) => String::new(),
        (false, true) => trimmed_summary.to_string(),
        (true, false) => trimmed_ledger.to_string(),
        (false, false) => format!("{trimmed_summary}\n\n{trimmed_ledger}"),
    }
}

pub(crate) fn resolve_compaction_split_index(messages: &[Message], keep_msgs: usize) -> usize {
    let len = messages.len();
    let target = len.saturating_sub(keep_msgs);
    let user_split = (1..=target)
        .rev()
        .find(|&idx| idx < len && messages[idx].role == "user");
    match user_split {
        Some(idx) => idx,
        None => len / 2,
    }
}

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
    let should_compact = (should_compact_tokens || len > max_msgs) && len > 5;

    match should_compact {
        false => Ok(TurnState::Command),
        true => {
            let keep_msgs =
                crate::providers::DynamicContextRegistry::resolve_keep_recent_count(len, config);
            let k = resolve_compaction_split_index(&ctx.session.messages, keep_msgs);

            match k > 0 && k < len {
                false => {
                    ctx.session.messages = ctx.session.messages[k.min(len)..].to_vec();
                    Ok(TurnState::Command)
                }
                true => {
                    let messages_to_summarize = ctx.session.messages[0..k].to_vec();
                    let working_state_ledger =
                        extract_working_state_ledger(&messages_to_summarize);

                    let existing_memory = ctx
                        .session
                        .metadata
                        .get("memory")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();

                    let system_prompt_mem = "You are a specialized Memory Manager. Your task is to extract and update the durable long-term memory of key facts, user preferences, decisions, and modified files based on the conversation history about to be pruned.\n\nIncorporate new facts into existing memory, remove contradicted information, and return a clean, consolidated Markdown list of memory facts.";
                    let mut mem_prompt_content = String::new();
                    let _ = (!existing_memory.is_empty()).then(|| {
                        mem_prompt_content
                            .push_str(&format!("Existing memory:\n{existing_memory}\n\n"));
                    });
                    mem_prompt_content.push_str(
                        "Pre-compression conversation history to preserve facts/decisions from:\n",
                    );
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
                        "{RED_ORANGE}▸ Preserving memory before context compaction...{COLOR_RESET}"
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
                            let _ = resp.content.map(|new_memory| {
                                tracing::info!(
                                    session = %ctx.session_key,
                                    "Preserved durable memory ({} chars)",
                                    new_memory.len()
                                );
                                ctx.session
                                    .metadata
                                    .insert("memory".to_string(), Value::String(new_memory));
                            });
                        }
                        Err(e) => {
                            let silent = crate::agent::style::spinner::is_silent();
                            tracing::error!(
                                session = %ctx.session_key,
                                "Failed to preserve pre-compression memory: {}",
                                e
                            );
                            let _ = (!silent).then(|| {
                                crate::tui_println!(
                                    "{AURA_GOLD}▲ Failed to preserve pre-compression memory: {e}{COLOR_RESET}"
                                );
                            });
                        }
                    }

                    let existing_summary = ctx
                        .session
                        .metadata
                        .get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();

                    let system_prompt_sum = "You are a helpful assistant. Generate a consolidated summary of the conversation history. Keep it concise, clear, and focused on key facts, decisions, and files created/modified.";
                    let mut prompt_content = String::new();
                    let _ = (!existing_summary.is_empty()).then(|| {
                        prompt_content
                            .push_str(&format!("Previous summary:\n{existing_summary}\n\n"));
                    });
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

                    let spinner_msg_sum =
                        format!("{RED_ORANGE}▸ Consolidating conversation context...{COLOR_RESET}");
                    tracing::info!(
                        session = %ctx.session_key,
                        "Compacting history ({} msgs, ~{} estimated tokens, model: {})...",
                        len,
                        estimated_tokens,
                        config.agents.defaults.model
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
                            let base_summary =
                                resp.content.unwrap_or_else(|| existing_summary.clone());
                            let merged =
                                merge_summary_with_ledger(&base_summary, &working_state_ledger);
                            let _ = (!merged.is_empty()).then(|| {
                                tracing::info!(
                                    session = %ctx.session_key,
                                    "Compacted summary length: {} chars",
                                    merged.len()
                                );
                                ctx.session
                                    .metadata
                                    .insert("summary".to_string(), Value::String(merged));
                            });
                        }
                        Err(e) => {
                            tracing::error!(
                                session = %ctx.session_key,
                                "Failed to compact conversation history: {}",
                                e
                            );
                            let _ = (!crate::agent::style::spinner::is_silent()).then(|| {
                                crate::tui_println!(
                                    "{AURA_GOLD}▲ Failed to summarize conversation history: {e}{COLOR_RESET}"
                                );
                            });
                            let fallback_merged =
                                merge_summary_with_ledger(&existing_summary, &working_state_ledger);
                            let _ = (!fallback_merged.is_empty()).then(|| {
                                ctx.session.metadata.insert(
                                    "summary".to_string(),
                                    Value::String(fallback_merged),
                                );
                            });
                        }
                    }

                    ctx.session.messages = ctx.session.messages[k..].to_vec();
                    Ok(TurnState::Command)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_working_state_ledger_captures_touched_files_mutating_tools_and_archives() {
        let mut assistant_extra = Map::new();
        assistant_extra.insert(
            "tool_calls".to_string(),
            json!([
                {
                    "name": "patch_file",
                    "arguments": { "path": "src/tools/registry.rs", "diff": "..." }
                },
                {
                    "name": "read_file",
                    "arguments": { "file_path": "src/agent/agent_loop/build.rs" }
                }
            ]),
        );

        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "update registry".to_string(),
                timestamp: None,
                extra: Map::new(),
            },
            Message {
                role: "assistant".to_string(),
                content: String::new(),
                timestamp: None,
                extra: assistant_extra,
            },
            Message {
                role: "tool".to_string(),
                content: "Full output saved to /tmp/.openz/tool_outputs/read_file_123.json and ccr_98af12"
                    .to_string(),
                timestamp: None,
                extra: Map::new(),
            },
        ];

        let ledger = extract_working_state_ledger(&messages);
        assert!(ledger.contains("[Compacted Working State Ledger]"));
        assert!(ledger.contains("src/tools/registry.rs"));
        assert!(ledger.contains("src/agent/agent_loop/build.rs"));
        assert!(ledger.contains("patch_file"));
        assert!(ledger.contains("/tmp/.openz/tool_outputs/read_file_123.json"));
        assert!(ledger.contains("ccr_98af12"));
    }

    #[test]
    fn resolve_compaction_split_index_aligns_to_user_turn_boundary() {
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: "turn 1".to_string(),
                timestamp: None,
                extra: Map::new(),
            },
            Message {
                role: "assistant".to_string(),
                content: "resp 1".to_string(),
                timestamp: None,
                extra: Map::new(),
            },
            Message {
                role: "user".to_string(),
                content: "turn 2".to_string(),
                timestamp: None,
                extra: Map::new(),
            },
            Message {
                role: "assistant".to_string(),
                content: "resp 2".to_string(),
                timestamp: None,
                extra: Map::new(),
            },
            Message {
                role: "tool".to_string(),
                content: "tool 2".to_string(),
                timestamp: None,
                extra: Map::new(),
            },
        ];

        // len = 5, keep_msgs = 2 => target = 3 ("assistant"), scans back to idx = 2 ("user")
        assert_eq!(resolve_compaction_split_index(&messages, 2), 2);
    }
}

