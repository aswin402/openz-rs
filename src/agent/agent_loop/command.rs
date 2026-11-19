use super::{AgentLoop, TurnContext, TurnState};
use crate::tools::subagent::{CancellationToken, DelegateTaskTool};
use anyhow::Result;

fn handle_audit(ctx: &mut TurnContext<'_>) {
    ctx.final_content = match ctx.session.verify_hash_chain() {
        Ok(_) => {
            let mut output =
                "✅ MERKLE AUDIT PASS: Chain integrity verified successfully.\n\n".to_string();
            output.push_str("Index | Role | Timestamp | Merkle Block Hash\n");
            output.push_str("------|------|-----------|-------------------\n");
            for (i, msg) in ctx.session.messages.iter().enumerate() {
                let hash = msg.get_hash().unwrap_or("None");
                let ts = msg.timestamp.as_deref().unwrap_or("N/A");
                output.push_str(&format!("{:5} | {:4} | {} | {}\n", i, msg.role, ts, hash));
            }
            output
        }
        Err(e) => format!(
            "❌ MERKLE AUDIT FAIL: Chain integrity compromised!\nError: {}",
            e
        ),
    };
}

async fn handle_memory(
    loop_ref: &AgentLoop,
    ctx: &mut TurnContext<'_>,
    parts: &[&str],
) -> Result<()> {
    match parts.get(1).copied() {
        None => {
            let memory = ctx
                .session
                .metadata
                .get("memory")
                .and_then(|v| v.as_str())
                .unwrap_or("No memory recorded yet.");
            ctx.final_content = format!("=== Agent Long-Term Memory ===\n{}", memory);
        }
        Some("clear") => {
            ctx.session.metadata.remove("memory");
            loop_ref.session_manager.save(&ctx.session).await?;
            ctx.final_content = "Agent memory has been cleared.".to_string();
        }
        Some("add" | "set") => match parts.len() >= 3 {
            false => {
                ctx.final_content = "Usage: /memory add <fact>".to_string();
            }
            true => {
                let fact = parts[2..].join(" ");
                let mut existing = ctx
                    .session
                    .metadata
                    .get("memory")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let _ = (!existing.is_empty()).then(|| existing.push('\n'));
                existing.push_str(&format!("* {}", fact));
                ctx.session.metadata.insert(
                    "memory".to_string(),
                    serde_json::Value::String(existing),
                );
                loop_ref.session_manager.save(&ctx.session).await?;
                ctx.final_content = format!("Added to memory: {}", fact);
            }
        },
        Some(_) => {
            ctx.final_content =
                "Unknown memory command. Options: /memory, /memory clear, /memory add <fact>"
                    .to_string();
        }
    }
    Ok(())
}

async fn handle_delegate(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>, parts: &[&str]) {
    match parts.len() >= 2 {
        false => {
            ctx.final_content = "Usage: /delegate <goal>".to_string();
        }
        true => {
            let goal = parts[1..].join(" ");
            let parent_tools = loop_ref
                .tools
                .get_static_tools()
                .into_iter()
                .filter(|t| t.name() != "delegate_task" && t.name() != "parallel_research")
                .collect();
            let delegate_tool: std::sync::Arc<dyn crate::tools::Tool> =
                std::sync::Arc::new(DelegateTaskTool {
                    config: ctx.config.clone(),
                    parent_provider: ctx.active_provider.clone(),
                    session_manager: loop_ref.session_manager.clone(),
                    parent_tools,
                    cancellation_token: CancellationToken::new(),
                    capability_policy: None,
                });

            let args = serde_json::json!({
                "goal": goal,
            });

            ctx.final_content = match delegate_tool.call(&args).await {
                Ok(res_val) => match res_val.get("summary").and_then(|v| v.as_str()) {
                    Some(summary) => format!("=== Subagent Summary ===\n{}", summary),
                    None => format!("Subagent completed: {}", res_val),
                },
                Err(e) => format!("Error running subagent: {}", e),
            };
        }
    }
}

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let is_webui =
        crate::config::schema::Config::ui_surface_for_session_key(ctx.session_key) == "webui";
    match (!is_webui, ctx.user_content.starts_with('/')) {
        (true, true) => {
            let parts: Vec<&str> = ctx.user_content.split_whitespace().collect();
            match parts.first().copied() {
                Some("/help") => {
                    ctx.final_content = "OpenZ Rebranded AI Agent Command Menu:\n/help - Show this menu\n/history - Show history\n/clear - Reset session history\n/status - Print active model and configuration info\n/memory - Show or manage agent memory (/memory, /memory clear, /memory add <fact>)\n/audit - Cryptographically verify session message chain integrity\n/delegate <goal> - Directly delegate a task to a focused subagent".to_string();
                    Ok(TurnState::Done)
                }
                Some("/history") => {
                    let mut hist = String::new();
                    for msg in ctx
                        .session
                        .archived_messages
                        .iter()
                        .chain(ctx.session.messages.iter())
                    {
                        hist.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
                    }
                    ctx.final_content = hist;
                    Ok(TurnState::Done)
                }
                Some("/clear" | "/restart") => {
                    ctx.session.archived_messages.clear();
                    ctx.session.messages.clear();
                    crate::agent::agent_loop::build::invalidate_session_skill_catalog(ctx.session_key);
                    loop_ref.session_manager.save(&ctx.session).await?;
                    ctx.final_content = "Conversation history has been cleared.".to_string();
                    Ok(TurnState::Done)
                }
                Some("/status") => {
                    let spec = crate::providers::DynamicContextRegistry::resolve_model_spec(
                        &ctx.config.agents.defaults.model,
                        Some(&ctx.config),
                    );
                    ctx.final_content = format!(
                        "OpenZ Agent Status:\nModel: {}\nProvider: {}\nContext Window: {} tokens ({})\nMax Output Tokens: {}\nWorkspace: {}\nTotal Messages: {}",
                        ctx.config.agents.defaults.model,
                        ctx.config.agents.defaults.provider,
                        spec.context_window,
                        spec.source,
                        spec.max_output_tokens,
                        ctx.config.agents.defaults.workspace,
                        ctx.session.messages.len()
                    );
                    Ok(TurnState::Done)
                }
                Some("/audit") => {
                    handle_audit(ctx);
                    Ok(TurnState::Done)
                }
                Some("/memory") => {
                    handle_memory(loop_ref, ctx, &parts).await?;
                    Ok(TurnState::Done)
                }
                Some("/delegate" | "/subagent") => {
                    handle_delegate(loop_ref, ctx, &parts).await;
                    Ok(TurnState::Done)
                }
                _ => Ok(TurnState::Build),
            }
        }
        _ => Ok(TurnState::Build),
    }
}
