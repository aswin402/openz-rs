use anyhow::Result;
use crate::agent::style::*;
use crate::tools::subagent::{DelegateTaskTool, CancellationToken};
use super::{AgentLoop, TurnContext, TurnState};

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    let mut profile_name = None;
    let parts_key: Vec<&str> = ctx.session_key.split(':').collect();
    if parts_key.len() >= 2 && parts_key[0] == "subagent" {
        profile_name = Some(parts_key[1]);
    }
    if ctx.user_content.starts_with('/') {
        let parts: Vec<&str> = ctx.user_content.split_whitespace().collect();
        if let Some(cmd) = parts.first() {
            match *cmd {
                "/help" => {
                    ctx.final_content = "OpenZ Rebranded AI Agent Command Menu:\n/help - Show this menu\n/history - Show history\n/clear - Reset session history\n/status - Print active model and configuration info\n/memory - Show or manage agent memory (/memory, /memory clear, /memory add <fact>)\n/skills - List active skills (/skills, /skills clear)\n/skill - Manage skills (/skill view <name>, /skill add <name> <content>, /skill delete <name>)\n/audit - Cryptographically verify session message chain integrity\n/delegate <goal> - Directly delegate a task to a focused subagent".to_string();
                    return Ok(TurnState::Done);
                }
                "/history" => {
                    let mut hist = String::new();
                    for msg in &ctx.session.messages {
                        hist.push_str(&format!("[{}]: {}\n", msg.role, msg.content));
                    }
                    ctx.final_content = hist;
                    return Ok(TurnState::Done);
                }
                "/clear" | "/restart" => {
                    ctx.session.messages.clear();
                    loop_ref.session_manager.save(&ctx.session)?;
                    ctx.final_content = "Conversation history has been cleared.".to_string();
                    return Ok(TurnState::Done);
                }
                "/status" => {
                    ctx.final_content = format!(
                        "OpenZ Agent Status:\nModel: {}\nProvider: {}\nWorkspace: {}\nTotal Messages: {}",
                        loop_ref.config.agents.defaults.model,
                        loop_ref.config.agents.defaults.provider,
                        loop_ref.config.agents.defaults.workspace,
                        ctx.session.messages.len()
                    );
                    return Ok(TurnState::Done);
                }
                "/audit" => {
                    match ctx.session.verify_hash_chain() {
                        Ok(_) => {
                            let mut output = "✅ MERKLE AUDIT PASS: Chain integrity verified successfully.\n\n".to_string();
                            output.push_str("Index | Role | Timestamp | Merkle Block Hash\n");
                            output.push_str("------|------|-----------|-------------------\n");
                            for (i, msg) in ctx.session.messages.iter().enumerate() {
                                let hash = msg.get_hash().unwrap_or("None");
                                let ts = msg.timestamp.as_deref().unwrap_or("N/A");
                                output.push_str(&format!(
                                    "{:5} | {:4} | {} | {}\n",
                                    i, msg.role, ts, hash
                                ));
                            }
                            ctx.final_content = output;
                        }
                        Err(e) => {
                            ctx.final_content = format!("❌ MERKLE AUDIT FAIL: Chain integrity compromised!\nError: {}", e);
                        }
                    }
                    return Ok(TurnState::Done);
                }
                "/memory" => {
                    if parts.len() < 2 {
                        let memory = ctx.session.metadata.get("memory")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No memory recorded yet.");
                        ctx.final_content = format!("=== Agent Long-Term Memory ===\n{}", memory);
                    } else {
                        match parts[1] {
                            "clear" => {
                                ctx.session.metadata.remove("memory");
                                loop_ref.session_manager.save(&ctx.session)?;
                                ctx.final_content = "Agent memory has been cleared.".to_string();
                            }
                            "add" | "set" => {
                                if parts.len() < 3 {
                                    ctx.final_content = "Usage: /memory add <fact>".to_string();
                                } else {
                                    let fact = parts[2..].join(" ");
                                    let mut existing = ctx.session.metadata.get("memory")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    if !existing.is_empty() {
                                        existing.push('\n');
                                    }
                                    existing.push_str(&format!("* {}", fact));
                                    ctx.session.metadata.insert("memory".to_string(), serde_json::Value::String(existing));
                                    loop_ref.session_manager.save(&ctx.session)?;
                                    ctx.final_content = format!("Added to memory: {}", fact);
                                }
                            }
                            _ => {
                                ctx.final_content = "Unknown memory command. Options: /memory, /memory clear, /memory add <fact>".to_string();
                            }
                        }
                    }
                    return Ok(TurnState::Done);
                }
                "/skills" => {
                    if parts.len() > 1 && parts[1] == "clear" {
                        if let Err(e) = crate::agent::skills::clear_skills() {
                            ctx.final_content = format!("Error clearing skills: {}", e);
                        } else {
                            ctx.final_content = "All agent skills have been cleared.".to_string();
                        }
                    } else {
                        match crate::agent::skills::load_skills_with_profile(profile_name) {
                            Ok(skills) => {
                                if skills.is_empty() {
                                    ctx.final_content = "No active skills recorded yet.".to_string();
                                } else {
                                    let list: Vec<String> = skills.iter().map(|s| format!("* {}", s.name)).collect();
                                    ctx.final_content = format!("=== Agent Skills ===\n{}", list.join("\n"));
                                }
                            }
                            Err(e) => {
                                ctx.final_content = format!("Error loading skills: {}", e);
                            }
                        }
                    }
                    return Ok(TurnState::Done);
                }
                "/skill" => {
                    if parts.len() < 2 {
                        ctx.final_content = "Usage: /skill view <name>, /skill add <name> <content>, /skill delete <name>".to_string();
                    } else {
                        match parts[1] {
                            "view" => {
                                if parts.len() < 3 {
                                    ctx.final_content = "Usage: /skill view <name>".to_string();
                                } else {
                                    let name = parts[2];
                                    match crate::agent::skills::load_skills_with_profile(profile_name) {
                                        Ok(skills) => {
                                            if let Some(skill) = skills.iter().find(|s| s.name == name) {
                                                ctx.final_content = format!("=== Skill: {} ===\n{}", skill.name, skill.content);
                                            } else {
                                                ctx.final_content = format!("Skill '{}' not found.", name);
                                            }
                                        }
                                        Err(e) => {
                                            ctx.final_content = format!("Error: {}", e);
                                        }
                                    }
                                }
                            }
                            "add" | "set" => {
                                if parts.len() < 4 {
                                    ctx.final_content = "Usage: /skill add <name> <content>".to_string();
                                } else {
                                    let name = parts[2];
                                    let content = parts[3..].join(" ");
                                    let res = if let Some(prof) = profile_name {
                                        crate::agent::skills::save_subagent_skill(prof, name, &content)
                                    } else {
                                        crate::agent::skills::save_skill(name, &content)
                                    };
                                    if let Err(e) = res {
                                        ctx.final_content = format!("Error saving skill: {}", e);
                                    } else {
                                        ctx.final_content = format!("Skill '{}' added/updated successfully.", name);
                                    }
                                }
                            }
                            "delete" | "remove" => {
                                if parts.len() < 3 {
                                    ctx.final_content = "Usage: /skill delete <name>".to_string();
                                } else {
                                    let name = parts[2];
                                    if let Err(e) = crate::agent::skills::delete_skill_with_profile(name, profile_name) {
                                        ctx.final_content = format!("Error deleting skill: {}", e);
                                    } else {
                                        ctx.final_content = format!("Skill '{}' deleted successfully.", name);
                                    }
                                }
                            }
                            _ => {
                                ctx.final_content = "Unknown skill command. Options: /skill view <name>, /skill add <name> <content>, /skill delete <name>".to_string();
                            }
                        }
                    }
                    return Ok(TurnState::Done);
                }
                "/delegate" | "/subagent" => {
                    if parts.len() < 2 {
                        ctx.final_content = "Usage: /delegate <goal>".to_string();
                    } else {
                        let goal = parts[1..].join(" ");
                        let parent_tools = loop_ref.tools.get_static_tools()
                            .into_iter()
                            .filter(|t| t.name() != "delegate_task" && t.name() != "parallel_research")
                            .collect();
                        let delegate_tool: std::sync::Arc<dyn crate::tools::Tool> = std::sync::Arc::new(DelegateTaskTool {
                            config: loop_ref.config.clone(),
                            parent_provider: ctx.active_provider.clone(),
                            session_manager: loop_ref.session_manager.clone(),
                            parent_tools,
                            cancellation_token: CancellationToken::new(),
                        });

                        let args = serde_json::json!({
                            "goal": goal,
                        });

                        match delegate_tool.call(&args).await {
                            Ok(res_val) => {
                                if let Some(summary) = res_val.get("summary").and_then(|v| v.as_str()) {
                                    ctx.final_content = format!("=== Subagent Summary ===\n{}", summary);
                                } else {
                                    ctx.final_content = format!("Subagent completed: {}", res_val);
                                }
                            }
                            Err(e) => {
                                ctx.final_content = format!("Error running subagent: {}", e);
                            }
                        }
                    }
                    return Ok(TurnState::Done);
                }
                _ => {}
            }
        }
    }
    Ok(TurnState::Build)
}
