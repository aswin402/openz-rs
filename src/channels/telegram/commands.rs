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

pub(crate) async fn handle_command(
    trimmed: &str,
    chat_id: i64,
    agent: &Arc<AgentLoop>,
    token: &str,
    client: &Client,
) -> bool {
    let raw_cmd = trimmed.split_whitespace().next().unwrap_or("");
    // Telegram only permits underscores in registered command names;
    // accept the legacy hyphen spelling in incoming messages too.
    let cmd = raw_cmd.replace('_', "-");
    let command_action = telegram_command_action(trimmed);

    let is_handled_command = command_action != TelegramCommandAction::None
        || cmd == "/new-session"
        || cmd == "/resume"
        || cmd == "/mcps"
        || cmd == "/memory"
        || cmd == "/skill"
        || cmd == "/skills"
        || cmd == "/sources"
        || cmd == "/workflows"
        || cmd == "/servers"
        || cmd == "/stop-server"
        || cmd.starts_with("/model")
        || cmd == "/switch-model"
        || cmd == "/help"
        || cmd == "/clear"
        || cmd == "/history";

    if !is_handled_command {
        return false;
    }

    if command_action == TelegramCommandAction::Stop {
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

    if cmd == "/servers" {
        let servers = crate::shutdown::list_registered_children();
        let response = if servers.is_empty() {
            "No OpenZ-launched background servers running.".to_string()
        } else {
            let mut lines = vec!["OpenZ background servers:".to_string()];
            for server in servers {
                lines.push(format!(
                    "#{} pid={} {} - {}",
                    server.id, server.pid, server.kind, server.command
                ));
            }
            lines.push("Use /stop-server <id> or /stop-server all.".to_string());
            lines.join("\n")
        };
        spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
        return true;
    }

    if cmd == "/stop-server" {
        let target = trimmed
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .trim();
        let response = if target.is_empty() {
            "Usage: /stop-server <id|all>".to_string()
        } else {
            match crate::shutdown::stop_registered_child(target) {
                Ok(0) => "No matching background server found.".to_string(),
                Ok(count) => format!("✓ Stopped {} background server(s).", count),
                Err(e) => format!("✕ Failed to stop server: {}", e),
            }
        };
        spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
        return true;
    }

    if cmd == "/new-session" {
        let session_manager = &agent.session_manager;
        let session_key = format!("telegram:{}", chat_id);
        let response = match crate::channels::start_new_channel_session(session_manager, &session_key).await {
            Ok(_) => "✓ Session reset. Starting a new session.".to_string(),
            Err(e) => format!("Failed to start new session: {e}"),
        };
        spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
        return true;
    }

    if cmd == "/resume" {
        let session_manager = &agent.session_manager;
        let session_key = format!("telegram:{}", chat_id);
        let args: Vec<&str> = trimmed.split_whitespace().collect();
        let sessions = crate::channels::list_channel_sessions(&session_manager.dir, &session_key, 10);
        if args.len() > 1 {
            let response = if let Ok(index) = args[1].parse::<usize>() {
                if index == 0 || index > sessions.len() {
                    format!("Invalid session number. Use /resume to list 1..{}.", sessions.len())
                } else {
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
            } else {
                "Usage: /resume or /resume <number>".to_string()
            };
            spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
        } else {
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

            let payload = if sessions.is_empty() {
                serde_json::json!({
                    "chat_id": chat_id,
                    "text": "No previous sessions found for this Telegram chat."
                })
            } else {
                serde_json::json!({
                    "chat_id": chat_id,
                    "text": "Select a previous session to resume, or continue current session:",
                    "reply_markup": { "inline_keyboard": keyboard }
                })
            };
            let client_clone = client.clone();
            let token_clone = token.to_string();
            tokio::spawn(async move {
                let send_url = telegram_api_url(&token_clone, "sendMessage");
                let _ = client_clone.post(&send_url).json(&payload).send().await;
            });
        }
        return true;
    }

    if cmd == "/mcps" {
        let mut response = String::from("🛠️ *Configured MCP Servers:*\n");
        if agent.config.mcp_servers.is_empty() {
            response.push_str("No MCP servers configured.");
        } else {
            for (name, mcp_cfg) in &agent.config.mcp_servers {
                let status = if mcp_cfg.enabled {
                    "✅ enabled"
                } else {
                    "❌ disabled"
                };
                response.push_str(&format!(
                    "• *{}* ({}) \n`{}`\n",
                    escape_markdown(name),
                    status,
                    escape_markdown(&mcp_cfg.command)
                ));
            }
        }
        spawn_telegram_markdown(client.clone(), token.to_string(), chat_id, response);
        return true;
    }

    if cmd == "/sources" {
        let query = trimmed.strip_prefix("/sources").unwrap_or("").trim().to_string();
        let response = match crate::tools::shared_memory::search_source_bookmarks(&query, 10).await {
            Ok(items) if items.is_empty() => "No saved sources matched.".to_string(),
            Ok(items) => {
                let mut out = String::from("Saved sources:\n");
                for item in items {
                    out.push_str(&format!("- {} [{}]\n  {}\n", item.label, item.kind, item.uri));
                }
                out
            }
            Err(e) => format!("Failed to search sources: {e}"),
        };
        spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
        return true;
    }

    if cmd == "/workflows" {
        let query = trimmed.strip_prefix("/workflows").unwrap_or("").trim().to_string();
        let response = match crate::tools::shared_memory::search_workflow_cards(&query, 10, false).await {
            Ok(items) if items.is_empty() => "No reusable workflows matched.".to_string(),
            Ok(items) => {
                let mut out = String::from("Reusable workflows:\n");
                for item in items {
                    out.push_str(&format!(
                        "- {} [{}] success={} failure={}\n  {}\n",
                        item.name, item.status, item.success_count, item.failure_count, item.summary
                    ));
                }
                out
            }
            Err(e) => format!("Failed to search workflows: {e}"),
        };
        spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
        return true;
    }

    if cmd == "/memory" {
        let session_manager = &agent.session_manager;
        let session_key = format!("telegram:{}", chat_id);
        let mut response = String::from("🧠 *Session Metadata & Memory:*\n");
        if let Ok(session) = session_manager.load(&session_key) {
            if session.metadata.is_empty() {
                response.push_str("No memory or metadata recorded for this session.");
            } else {
                for (k, v) in &session.metadata {
                    let v_str = if let Some(s) = v.as_str() {
                        s.to_string()
                    } else {
                        v.to_string()
                    };
                    response.push_str(&format!(
                        "• *{}*: {}\n",
                        escape_markdown(k),
                        escape_markdown(&v_str)
                    ));
                }
            }
        } else {
            response.push_str("No active session found.");
        }
        spawn_telegram_markdown(client.clone(), token.to_string(), chat_id, response);
        return true;
    }

    if cmd == "/skill" || cmd == "/skills" {
        let mut response = String::from("⚡ *Active Skills:*\n");
        match crate::agent::skills::load_skills() {
            Ok(skills) => {
                if skills.is_empty() {
                    response.push_str("No active skills found in ~/.openz/skills");
                } else {
                    for skill in skills {
                        response.push_str(&format!("• *{}*\n", escape_markdown(&skill.name)));
                    }
                }
            }
            Err(e) => {
                response.push_str(&format!("❌ Failed to load skills: {}", escape_markdown(&e.to_string())));
            }
        }
        spawn_telegram_markdown(client.clone(), token.to_string(), chat_id, response);
        return true;
    }

    if cmd == "/switch-model" {
        let config = match crate::config::loader::load_config() {
            Ok(config) => config,
            Err(e) => {
                let response = format!("Failed to load OpenZ config: {}", e);
                spawn_telegram_msg(client.clone(), token.to_string(), chat_id, response);
                return true;
            }
        };
        let providers = crate::channels::configured_provider_models(&config);
        if providers.is_empty() {
            spawn_telegram_msg(
                client.clone(),
                token.to_string(),
                chat_id,
                "No configured LLM providers found. Run `openz configure` first.".to_string(),
            );
            return true;
        }

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
        return true;
    }

    if cmd.starts_with("/model") {
        let model = &agent.config.agents.defaults.model;
        let provider = &agent.config.agents.defaults.provider;
        let response = format!(
            "🤖 *Active Model:* `{}`\n*Provider:* `{}`",
            escape_markdown(model),
            escape_markdown(provider)
        );
        spawn_telegram_markdown(client.clone(), token.to_string(), chat_id, response);
        return true;
    }

    if cmd == "/help" {
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
        spawn_telegram_markdown(client.clone(), token.to_string(), chat_id, help_text.to_string());
        return true;
    }

    if cmd == "/clear" {
        spawn_telegram_msg(
            client.clone(),
            token.to_string(),
            chat_id,
            "🧹 `/clear` is a TUI-only command (it does not apply to Telegram chat history).".to_string(),
        );
        return true;
    }

    if cmd == "/history" {
        spawn_telegram_msg(
            client.clone(),
            token.to_string(),
            chat_id,
            "🗂️ `/history` interactive menu is a TUI-only command. To reset/clear history, use `/new-session`.".to_string(),
        );
        return true;
    }

    if cmd == "/local" || cmd == "/exit" {
        clear_remote_session(chat_id);
        spawn_telegram_msg(
            client.clone(),
            token.to_string(),
            chat_id,
            "🏠 [Local Mode Activated]\nMessages will be processed locally by the Telegram bot.".to_string(),
        );
        return true;
    }

    // Default remote mode picker
    let sessions = crate::agent::activity::list_active_tui_sessions();
    if sessions.is_empty() {
        spawn_telegram_msg(
            client.clone(),
            token.to_string(),
            chat_id,
            "No active OpenZ TUI sessions found. Start `openz agent` in a terminal, then use /remote again.".to_string(),
        );
        return true;
    }

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
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_telegram_commands_use_telegram_safe_names() {
        assert!(TELEGRAM_COMMANDS
            .iter()
            .all(|(command, _)| valid_telegram_command_name(command)));
        let payload = telegram_commands_payload();
        assert_eq!(
            payload["commands"].as_array().unwrap().len(),
            TELEGRAM_COMMANDS.len()
        );
    }

    #[test]
    fn telegram_stop_command_is_classified_as_cancel() {
        assert_eq!(
            telegram_command_action("/stop"),
            TelegramCommandAction::Stop
        );
        assert_eq!(
            telegram_command_action("/stop now"),
            TelegramCommandAction::Stop
        );
        assert_eq!(
            telegram_command_action("/cancel"),
            TelegramCommandAction::Stop
        );
        assert_eq!(
            telegram_command_action("/tui-esc"),
            TelegramCommandAction::Stop
        );
        assert_eq!(
            telegram_command_action("/tui-cancel"),
            TelegramCommandAction::Stop
        );
        assert_eq!(
            telegram_command_action("/remote"),
            TelegramCommandAction::RemoteMode
        );
        assert_eq!(
            telegram_command_action("hello"),
            TelegramCommandAction::None
        );
    }
}
