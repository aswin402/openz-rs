use crate::agent::AgentLoop;
use crate::channels::notifications::telegram_api_url;
use crate::channels::telegram::messages::{
    escape_markdown, spawn_telegram_keyboard, spawn_telegram_markdown, spawn_telegram_msg,
};
use crate::channels::telegram::state::{
    clear_remote_session, remote_session_button_label, stop_typing_indicator,
};
use crate::channels::telegram::types::TelegramCommandAction;
use reqwest::Client;
use std::sync::Arc;

pub(crate) const TELEGRAM_COMMANDS: &[(&str, &str)] = &[
    ("remote", "Toggle TUI remote control mode"),
    ("local", "Switch to local bot chat mode"),
    ("new_session", "Start a new local session"),
    ("resume", "Resume a previous local session"),
    ("model", "Show the active default model"),
    ("switch_model", "Switch the default provider/model"),
    ("mcps", "List configured MCP servers"),
    ("memory", "View metadata memory for the session"),
    ("skill", "List active skills"),
    ("sources", "Search saved source bookmarks"),
    ("workflows", "Search reusable workflows"),
    ("help", "List available commands"),
    ("exit", "Exit remote control mode"),
];

pub(crate) fn telegram_commands_payload() -> serde_json::Value {
    serde_json::json!({
        "commands": TELEGRAM_COMMANDS
            .iter()
            .map(|(command, description)| serde_json::json!({
                "command": command,
                "description": description
            }))
            .collect::<Vec<_>>()
    })
}

pub(crate) fn telegram_command_action(text: &str) -> TelegramCommandAction {
    match text.split_whitespace().next().unwrap_or("") {
        "/stop" | "/cancel" | "/tui-esc" | "/tui-cancel" => TelegramCommandAction::Stop,
        "/remote" | "/remotecontrol" | "/local" | "/exit" => TelegramCommandAction::RemoteMode,
        _ => TelegramCommandAction::None,
    }
}

#[cfg(test)]
pub(crate) fn valid_telegram_command_name(command: &str) -> bool {
    !command.is_empty()
        && command.len() <= 32
        && command
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_supported_telegram_command(cmd: &str, command_action: TelegramCommandAction) -> bool {
    command_action != TelegramCommandAction::None
        || cmd.starts_with("/model")
        || matches!(
            cmd,
            "/new-session"
                | "/resume"
                | "/mcps"
                | "/memory"
                | "/skill"
                | "/skills"
                | "/sources"
                | "/workflows"
                | "/servers"
                | "/stop-server"
                | "/switch-model"
                | "/help"
                | "/clear"
                | "/history"
        )
}

fn format_servers_response() -> String {
    let servers = crate::shutdown::list_registered_children();
    match servers.is_empty() {
        true => "No OpenZ-launched background servers running.".to_string(),
        false => {
            let mut lines = vec!["OpenZ background servers:".to_string()];
            for server in servers {
                lines.push(format!(
                    "#{} pid={} {} - {}",
                    server.id, server.pid, server.kind, server.command
                ));
            }
            lines.push("Use /stop-server <id> or /stop-server all.".to_string());
            lines.join("\n")
        }
    }
}

async fn handle_resume_command(
    trimmed: &str,
    chat_id: i64,
    agent: &Arc<AgentLoop>,
    token: &str,
    client: &Client,
) {
    let session_manager = &agent.session_manager;
    let session_key = format!("telegram:{}", chat_id);
    let args: Vec<&str> = trimmed.split_whitespace().collect();
    let sessions = crate::channels::list_channel_sessions(&session_manager.dir, &session_key, 10);
    match args.get(1) {
        Some(raw_idx) => {
            let parsed_idx = raw_idx.parse::<usize>().ok();
            let valid_idx = parsed_idx.filter(|idx| *idx > 0 && *idx <= sessions.len());
            let response = match (parsed_idx, valid_idx) {
                (Some(_), Some(index)) => {
                    match crate::channels::resume_channel_session(
                        session_manager,
                        &session_key,
                        &sessions[index - 1].key,
                    )
                    .await
                    {
                        Ok(msg) => msg,
                        Err(e) => format!("Failed to resume session: {e}"),
                    }
                }
                (Some(_), None) => format!(
                    "Invalid session number. Use /resume to list 1..{}.",
                    sessions.len()
                ),
                (None, _) => "Usage: /resume or /resume <number>".to_string(),
            };
            spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
        }
        None => {
            let keyboard: Vec<Vec<serde_json::Value>> = sessions
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    vec![serde_json::json!({
                        "text": format!(
                            "{} | {} msgs | {}",
                            item.updated_at.format("%Y-%m-%d %H:%M"),
                            item.message_count,
                            item.display_title
                        ),
                        "callback_data": format!("resume_select:{}", idx)
                    })]
                })
                .chain(std::iter::once(vec![serde_json::json!({
                    "text": "Continue current session",
                    "callback_data": "resume_cancel:0"
                })]))
                .collect();

            let payload = match sessions.is_empty() {
                true => serde_json::json!({
                    "chat_id": chat_id,
                    "text": "No previous sessions found for this Telegram chat."
                }),
                false => serde_json::json!({
                    "chat_id": chat_id,
                    "text": "Select a previous session to resume, or continue current session:",
                    "reply_markup": { "inline_keyboard": keyboard }
                }),
            };
            let client_clone = client.clone();
            let token_clone = token.to_string();
            tokio::spawn(async move {
                let send_url = telegram_api_url(&token_clone, "sendMessage");
                let _ = client_clone.post(&send_url).json(&payload).send().await;
            });
        }
    }
}

fn format_mcps_response(agent: &Arc<AgentLoop>) -> String {
    let mut response = String::from("🛠️ *Configured MCP Servers:*\n");
    match agent.config.mcp_servers.is_empty() {
        true => response.push_str("No MCP servers configured."),
        false => {
            for (name, mcp_cfg) in &agent.config.mcp_servers {
                let status = match mcp_cfg.enabled {
                    true => "✅ enabled",
                    false => "❌ disabled",
                };
                response.push_str(&format!(
                    "• *{}* ({}) \n`{}`\n",
                    escape_markdown(name),
                    status,
                    escape_markdown(&mcp_cfg.command)
                ));
            }
        }
    }
    response
}

fn format_memory_response(agent: &Arc<AgentLoop>, chat_id: i64) -> String {
    let session_manager = &agent.session_manager;
    let session_key = format!("telegram:{}", chat_id);
    let mut response = String::from("🧠 *Session Metadata & Memory:*\n");
    match session_manager.load(&session_key) {
        Ok(session) => match session.metadata.is_empty() {
            true => response.push_str("No memory or metadata recorded for this session."),
            false => {
                for (k, v) in &session.metadata {
                    let v_str = v
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| v.to_string());
                    response.push_str(&format!(
                        "• *{}*: {}\n",
                        escape_markdown(k),
                        escape_markdown(&v_str)
                    ));
                }
            }
        },
        Err(_) => response.push_str("No active session found."),
    }
    response
}

fn format_skills_response() -> String {
    let mut response = String::from("⚡ *Active Skills:*\n");
    match crate::agent::skills::load_skills() {
        Ok(skills) => match skills.is_empty() {
            true => response.push_str("No active skills found in ~/.openz/skills"),
            false => {
                for skill in skills {
                    response.push_str(&format!("• *{}*\n", escape_markdown(&skill.name)));
                }
            }
        },
        Err(e) => {
            response.push_str(&format!(
                "❌ Failed to load skills: {}",
                escape_markdown(&e.to_string())
            ));
        }
    }
    response
}

fn handle_switch_model_command(chat_id: i64, token: &str, client: &Client) {
    let config = match crate::config::loader::load_config() {
        Ok(config) => config,
        Err(e) => {
            let response = format!("Failed to load OpenZ config: {}", e);
            spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
            return;
        }
    };
    let providers = crate::channels::configured_provider_models(&config);
    match providers.is_empty() {
        true => {
            spawn_telegram_msg(
                client.clone(),
                token.to_string(),
                chat_id,
                "No configured LLM providers found. Run `openz configure` first.".to_string(),
            );
        }
        false => {
            let keyboard: Vec<Vec<serde_json::Value>> = providers
                .iter()
                .enumerate()
                .map(|(idx, provider)| {
                    vec![serde_json::json!({
                        "text": format!("{} ({})", provider.display, provider.name),
                        "callback_data": format!("model_provider:{}", idx)
                    })]
                })
                .collect();
            let payload = serde_json::json!({
                "chat_id": chat_id,
                "text": format!(
                    "Current default: {} via {}\nSelect a provider:",
                    config.agents.defaults.model,
                    config.agents.defaults.provider
                ),
                "reply_markup": { "inline_keyboard": keyboard }
            });
            let client_clone = client.clone();
            let token_clone = token.to_string();
            tokio::spawn(async move {
                let send_url = telegram_api_url(&token_clone, "sendMessage");
                let _ = client_clone.post(&send_url).json(&payload).send().await;
            });
        }
    }
}

pub(crate) async fn handle_command(
    trimmed: &str,
    chat_id: i64,
    agent: &Arc<AgentLoop>,
    token: &str,
    client: &Client,
) -> bool {
    let raw_cmd = trimmed.split_whitespace().next().unwrap_or("");
    let cmd = raw_cmd.replace('_', "-");
    let command_action = telegram_command_action(trimmed);

    match is_supported_telegram_command(&cmd, command_action) {
        false => return false,
        true => {}
    }

    match command_action {
        TelegramCommandAction::Stop => {
            crate::shutdown::trigger_cli_cancel();
            stop_typing_indicator(chat_id);
            spawn_telegram_msg(
                client.clone(),
                token.to_string(),
                chat_id,
                "▲ Stop requested. Active OpenZ turn interrupted.".to_string(),
            );
            return true;
        }
        _ => {}
    }

    match cmd.as_str() {
        "/servers" => {
            spawn_telegram_msg(
                client.clone(),
                token.to_string(),
                chat_id,
                format_servers_response(),
            );
            return true;
        }
        "/stop-server" => {
            let target = trimmed.split_whitespace().nth(1).unwrap_or("");
            spawn_telegram_msg(
                client.clone(),
                token.to_string(),
                chat_id,
                crate::channels::format_stop_server_response(target),
            );
            return true;
        }
        "/new-session" => {
            let session_manager = &agent.session_manager;
            let session_key = format!("telegram:{}", chat_id);
            let response = match crate::channels::start_new_channel_session(
                session_manager,
                &session_key,
            )
            .await
            {
                Ok(_) => "✓ Session reset. Starting a new session.".to_string(),
                Err(e) => format!("Failed to start new session: {e}"),
            };
            spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
            return true;
        }
        "/resume" => {
            handle_resume_command(trimmed, chat_id, agent, token, client).await;
            return true;
        }
        "/mcps" => {
            spawn_telegram_markdown(
                client.clone(),
                token.to_string(),
                chat_id,
                format_mcps_response(agent),
            );
            return true;
        }
        "/sources" => {
            let query = trimmed
                .strip_prefix("/sources")
                .unwrap_or("")
                .trim()
                .to_string();
            let response =
                match crate::tools::shared_memory::search_source_bookmarks(&query, 10).await {
                    Ok(items) => match items.is_empty() {
                        true => "No saved sources matched.".to_string(),
                        false => {
                            let mut out = String::from("Saved sources:\n");
                            for item in items {
                                out.push_str(&format!(
                                    "- {} [{}]\n  {}\n",
                                    item.label, item.kind, item.uri
                                ));
                            }
                            out
                        }
                    },
                    Err(e) => format!("Failed to search sources: {e}"),
                };
            spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
            return true;
        }
        "/workflows" => {
            let query = trimmed
                .strip_prefix("/workflows")
                .unwrap_or("")
                .trim()
                .to_string();
            let response =
                match crate::tools::shared_memory::search_workflow_cards(&query, 10, false).await {
                    Ok(items) => match items.is_empty() {
                        true => "No reusable workflows matched.".to_string(),
                        false => {
                            let mut out = String::from("Reusable workflows:\n");
                            for item in items {
                                out.push_str(&format!(
                                    "- {} [{}] success={} failure={}\n  {}\n",
                                    item.name,
                                    item.status,
                                    item.success_count,
                                    item.failure_count,
                                    item.summary
                                ));
                            }
                            out
                        }
                    },
                    Err(e) => format!("Failed to search workflows: {e}"),
                };
            spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
            return true;
        }
        "/memory" => {
            spawn_telegram_markdown(
                client.clone(),
                token.to_string(),
                chat_id,
                format_memory_response(agent, chat_id),
            );
            return true;
        }
        "/skill" | "/skills" => {
            spawn_telegram_markdown(
                client.clone(),
                token.to_string(),
                chat_id,
                format_skills_response(),
            );
            return true;
        }
        "/switch-model" => {
            handle_switch_model_command(chat_id, token, client);
            return true;
        }
        "/help" => {
            let help_text = "📖 *OpenZ Telegram Bot Commands:*\n\n\
                             /remote — Select a TUI session for remote control\n\
                             /stop — Interrupt the active turn like Esc in TUI\n\
                             /cancel — Alias for /stop\n\
                             /tui-esc — Alias for TUI Esc\n\
                             /tui-cancel — Alias for TUI cancel\n\
                             /local — Switch to local bot chat mode\n\
                             /new-session — Start a new local session\n\
                             /resume — Resume a previous local session\n\
                             /model — Show the active default model\n\
                             /switch-model — Choose and save default provider/model\n\
                             /mcps — List configured MCP servers\n\
                             /memory — View metadata memory\n\
                             /skill — List active skills\n\
                             /help — List these commands\n\
                             /exit — Exit remote control mode";
            spawn_telegram_markdown(
                client.clone(),
                token.to_string(),
                chat_id,
                help_text.to_string(),
            );
            return true;
        }
        "/clear" => {
            spawn_telegram_msg(
                client.clone(),
                token.to_string(),
                chat_id,
                "🧹 `/clear` is a TUI-only command (it does not apply to Telegram chat history)."
                    .to_string(),
            );
            return true;
        }
        "/history" => {
            spawn_telegram_msg(
                client.clone(),
                token.to_string(),
                chat_id,
                "🗂️ `/history` interactive menu is a TUI-only command. To reset/clear history, use `/new-session`.".to_string(),
            );
            return true;
        }
        "/local" | "/exit" => {
            clear_remote_session(chat_id);
            spawn_telegram_msg(
                client.clone(),
                token.to_string(),
                chat_id,
                "🏠 [Local Mode Activated]\nMessages will be processed locally by the Telegram bot."
                    .to_string(),
            );
            return true;
        }
        _ => {}
    }

    match cmd.starts_with("/model") {
        true => {
            let session_key = format!("telegram:{}", chat_id);
            let session = agent.session_manager.load(&session_key).ok();
            let model = session
                .as_ref()
                .and_then(|s| s.metadata.get("model"))
                .and_then(|v| v.as_str())
                .unwrap_or(&agent.config.agents.defaults.model);
            let provider = session
                .as_ref()
                .and_then(|s| s.metadata.get("provider"))
                .and_then(|v| v.as_str())
                .unwrap_or(&agent.config.agents.defaults.provider);
            let response = format!(
                "🤖 *Active Model:* `{}`\n*Provider:* `{}`",
                escape_markdown(model),
                escape_markdown(provider)
            );
            spawn_telegram_markdown(client.clone(), token.to_string(), chat_id, response);
            return true;
        }
        false => {}
    }

    let sessions = crate::agent::activity::list_active_tui_sessions();
    match sessions.is_empty() {
        true => {
            spawn_telegram_msg(
                client.clone(),
                token.to_string(),
                chat_id,
                "No active OpenZ TUI sessions found. Start `openz` in a terminal, then use /remote again.".to_string(),
            );
        }
        false => {
            let keyboard: Vec<Vec<serde_json::Value>> = sessions
                .iter()
                .take(10)
                .map(|session| {
                    vec![serde_json::json!({
                        "text": remote_session_button_label(session),
                        "callback_data": format!("remote:{}", session.session_key)
                    })]
                })
                .collect();
            spawn_telegram_keyboard(
                client.clone(),
                token.to_string(),
                chat_id,
                "Select the OpenZ TUI session to control:".to_string(),
                serde_json::json!(keyboard),
            );
        }
    }
    true
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
