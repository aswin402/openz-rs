use super::{AgentLoop, TurnContext, TurnState};
use crate::tools::subagent::{CancellationToken, DelegateTaskTool};
use anyhow::Result;

fn subagent_profile_name(session_key: &str) -> Option<&str> {
    let parts_key: Vec<&str> = session_key.split(':').collect();
    match parts_key.as_slice() {
        ["subagent", profile, ..] => Some(*profile),
        _ => None,
    }
}

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

fn handle_skills(ctx: &mut TurnContext<'_>, parts: &[&str], profile_name: Option<&str>) {
    ctx.final_content = match parts.get(1).copied() {
        Some("clear") => match crate::agent::skills::clear_skills() {
            Err(e) => format!("Error clearing skills: {}", e),
            Ok(()) => "All agent skills have been cleared.".to_string(),
        },
        _ => match crate::agent::skills::load_skills_with_profile(profile_name) {
            Ok(skills) => match skills.is_empty() {
                true => "No active skills recorded yet.".to_string(),
                false => {
                    let list: Vec<String> =
                        skills.iter().map(|s| format!("* {}", s.name)).collect();
                    format!("=== Agent Skills ===\n{}", list.join("\n"))
                }
            },
            Err(e) => format!("Error loading skills: {}", e),
        },
    };
}

fn handle_skill(ctx: &mut TurnContext<'_>, parts: &[&str], profile_name: Option<&str>) {
    ctx.final_content = match parts.get(1).copied() {
        None => "Usage: /skill view <name>, /skill add <name> <content>, /skill delete <name>"
            .to_string(),
        Some("view") => match parts.get(2).copied() {
            None => "Usage: /skill view <name>".to_string(),
            Some(name) => match crate::agent::skills::load_skills_with_profile(profile_name) {
                Ok(skills) => match skills.iter().find(|s| s.name == name) {
                    Some(skill) => format!("=== Skill: {} ===\n{}", skill.name, skill.content),
                    None => format!("Skill '{}' not found.", name),
                },
                Err(e) => format!("Error: {}", e),
            },
        },
        Some("add" | "set") => match parts.len() >= 4 {
            false => "Usage: /skill add <name> <content>".to_string(),
            true => {
                let name = parts[2];
                let content = parts[3..].join(" ");
                let res = match profile_name {
                    Some(prof) => crate::agent::skills::save_subagent_skill(prof, name, &content),
                    None => crate::agent::skills::save_skill(name, &content),
                };
                match res {
                    Err(e) => format!("Error saving skill: {}", e),
                    Ok(()) => format!("Skill '{}' added/updated successfully.", name),
                }
            }
        },
        Some("delete" | "remove") => match parts.get(2).copied() {
            None => "Usage: /skill delete <name>".to_string(),
            Some(name) => match crate::agent::skills::delete_skill_with_profile(name, profile_name)
            {
                Err(e) => format!("Error deleting skill: {}", e),
                Ok(()) => format!("Skill '{}' deleted successfully.", name),
            },
        },
        Some(_) => {
            "Unknown skill command. Options: /skill view <name>, /skill add <name> <content>, /skill delete <name>"
                .to_string()
        }
    };
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
    match ctx.user_content.starts_with('/') {
        false => Ok(TurnState::Build),
        true => {
            let profile_name = subagent_profile_name(ctx.session_key);
            let parts: Vec<&str> = ctx.user_content.split_whitespace().collect();
            match parts.first().copied() {
                Some("/help") => {
                    ctx.final_content = "OpenZ Rebranded AI Agent Command Menu:\n/help - Show this menu\n/history - Show history\n/clear - Reset session history\n/status - Print active model and configuration info\n/memory - Show or manage agent memory (/memory, /memory clear, /memory add <fact>)\n/skills - List active skills (/skills, /skills clear)\n/skill - Manage skills (/skill view <name>, /skill add <name> <content>, /skill delete <name>)\n/audit - Cryptographically verify session message chain integrity\n/delegate <goal> - Directly delegate a task to a focused subagent".to_string();
                    Ok(TurnState::Done)
                }
                Some("/history") => {
                    let mut hist = String::new();
                    for msg in &ctx.session.messages {
                        hist.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
                    }
                    ctx.final_content = hist;
                    Ok(TurnState::Done)
                }
                Some("/clear" | "/restart") => {
                    ctx.session.messages.clear();
                    loop_ref.session_manager.save(&ctx.session).await?;
                    ctx.final_content = "Conversation history has been cleared.".to_string();
                    Ok(TurnState::Done)
                }
                Some("/status") => {
                    ctx.final_content = format!(
                        "OpenZ Agent Status:\nModel: {}\nProvider: {}\nWorkspace: {}\nTotal Messages: {}",
                        ctx.config.agents.defaults.model,
                        ctx.config.agents.defaults.provider,
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
                Some("/skills") => {
                    handle_skills(ctx, &parts, profile_name);
                    Ok(TurnState::Done)
                }
                Some("/skill") => {
                    handle_skill(ctx, &parts, profile_name);
                    Ok(TurnState::Done)
                }
                Some("/delegate" | "/subagent") => {
                    handle_delegate(loop_ref, ctx, &parts).await;
                    Ok(TurnState::Done)
                }
                _ => Ok(TurnState::Build),
            }
        }
    }
}
