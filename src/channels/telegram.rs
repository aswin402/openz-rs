use crate::agent::AgentLoop;
use serde::Deserialize;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use reqwest::Client;
use std::sync::{OnceLock, Mutex};
use std::collections::HashMap;
use tokio::sync::oneshot;

static TELEGRAM_BOT_INFO: OnceLock<(String, Client)> = OnceLock::new();
static APPROVAL_CALLBACKS: OnceLock<Mutex<HashMap<String, oneshot::Sender<bool>>>> = OnceLock::new();

pub fn get_telegram_bot_info() -> Option<(String, Client)> {
    TELEGRAM_BOT_INFO.get().cloned()
}

pub fn register_approval(req_id: &str, tx: oneshot::Sender<bool>) {
    let map = APPROVAL_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        guard.insert(req_id.to_string(), tx);
    }
}

pub fn unregister_approval(req_id: &str) {
    let map = APPROVAL_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        guard.remove(req_id);
    }
}

pub struct TelegramChannel {
    bot_token: String,
    agent_loop: Arc<AgentLoop>,
    client: Client,
}

#[derive(Deserialize, Debug)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
    callback_query: Option<TelegramCallbackQuery>,
}

#[derive(Deserialize, Debug)]
struct TelegramMessage {
    chat: TelegramChat,
    text: Option<String>,
    message_id: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct TelegramChat {
    id: i64,
}

#[derive(Deserialize, Debug)]
struct TelegramCallbackQuery {
    id: String,
    data: Option<String>,
    message: Option<TelegramMessage>,
}

static REMOTE_CONTROL_ACTIVE: OnceLock<Mutex<HashMap<i64, bool>>> = OnceLock::new();
static ACTIVE_TYPING_LOOPS: OnceLock<Mutex<HashMap<i64, oneshot::Sender<()>>>> = OnceLock::new();

pub fn start_typing_indicator(chat_id: i64, token: String, client: Client) {
    let map = ACTIVE_TYPING_LOOPS.get_or_init(|| Mutex::new(HashMap::new()));
    let (tx, rx) = oneshot::channel::<()>();
    
    let mut got_inserted = false;
    if let Ok(mut guard) = map.lock() {
        if let Some(old_tx) = guard.remove(&chat_id) {
            let _ = old_tx.send(());
        }
        guard.insert(chat_id, tx);
        got_inserted = true;
    }
    
    if got_inserted {
        tokio::spawn(async move {
            let send_action_url = format!("https://api.telegram.org/bot{}/sendChatAction", token);
            let payload = serde_json::json!({
                "chat_id": chat_id,
                "action": "typing"
            });
            let _ = client.post(&send_action_url).json(&payload).send().await;
            
            let mut rx = rx;
            loop {
                tokio::select! {
                    _ = &mut rx => {
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(4)) => {
                        let _ = client.post(&send_action_url).json(&payload).send().await;
                    }
                }
            }
        });
    }
}

pub fn stop_typing_indicator(chat_id: i64) {
    let map = ACTIVE_TYPING_LOOPS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        if let Some(tx) = guard.remove(&chat_id) {
            let _ = tx.send(());
        }
    }
}

fn is_remote_control_active(chat_id: i64) -> bool {
    let map = REMOTE_CONTROL_ACTIVE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = map.lock() {
        *guard.get(&chat_id).unwrap_or(&false)
    } else {
        false
    }
}

fn toggle_remote_control(chat_id: i64) -> bool {
    let map = REMOTE_CONTROL_ACTIVE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        let entry = guard.entry(chat_id).or_insert(false);
        *entry = !*entry;
        *entry
    } else {
        false
    }
}

fn set_remote_control(chat_id: i64, active: bool) {
    let map = REMOTE_CONTROL_ACTIVE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        guard.insert(chat_id, active);
    }
}

impl TelegramChannel {
    pub fn new(bot_token: String, agent_loop: AgentLoop) -> Self {
        TelegramChannel {
            bot_token,
            agent_loop: Arc::new(agent_loop),
            client: Client::builder().use_rustls_tls().build().unwrap_or_default(),
        }
    }
}

#[async_trait::async_trait]
impl super::Channel for TelegramChannel {
    fn name(&self) -> &'static str {
        "telegram"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let _ = TELEGRAM_BOT_INFO.set((self.bot_token.clone(), self.client.clone()));
        let mut offset = 0;
        let silent = std::env::var("OPENZ_SILENT").is_ok();
        if !silent {
            println!("🤖 Telegram Channel bot polling started...");
        }

        let session_dir = self.agent_loop.session_manager.dir.clone();

        // Send Active message to all active chats at startup
        let chats = crate::channels::get_active_session_targets(&session_dir, "telegram_");
        let active_msg = crate::channels::select_random_message(crate::channels::ACTIVE_MESSAGES);
        for chat_str in &chats {
            if let Ok(chat_id) = chat_str.parse::<i64>() {
                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
                let payload = serde_json::json!({
                    "chat_id": chat_id,
                    "text": active_msg,
                    "parse_mode": "Markdown"
                });
                let _ = self.client.post(&send_url).json(&payload).send().await;
            }
        }

        // Register slash commands for Telegram
        let set_commands_url = format!("https://api.telegram.org/bot{}/setMyCommands", self.bot_token);
        let commands_payload = serde_json::json!({
            "commands": [
                { "command": "remote", "description": "Toggle TUI remote control mode" },
                { "command": "local", "description": "Switch to local bot chat mode" },
                { "command": "new", "description": "Start a new local session" },
                { "command": "model", "description": "Show the active default model" },
                { "command": "mcps", "description": "List configured MCP servers" },
                { "command": "memory", "description": "View metadata memory for the session" },
                { "command": "skill", "description": "List active skills" },
                { "command": "help", "description": "List available commands" },
                { "command": "exit", "description": "Exit remote control mode" }
            ]
        });
        let client_clone = self.client.clone();
        let silent_clone = silent;
        tokio::spawn(async move {
            match client_clone.post(&set_commands_url).json(&commands_payload).send().await {
                Ok(res) => {
                    if !res.status().is_success() {
                        if let Ok(text) = res.text().await {
                            eprintln!("Failed to register Telegram slash commands: {}", text);
                        }
                    } else if !silent_clone {
                        println!("✓ Telegram slash commands registered successfully.");
                    }
                }
                Err(e) => {
                    eprintln!("Error registering Telegram slash commands: {}", e);
                }
            }
        });

        loop {
            let url = format!(
                "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
                self.bot_token, offset
            );

            let res = match self.client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Telegram poll error: {}", e);
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            #[derive(Deserialize)]
            struct UpdatesResponse {
                ok: bool,
                result: Vec<TelegramUpdate>,
            }

            if let Ok(resp) = res.json::<UpdatesResponse>().await {
                if resp.ok {
                    for update in resp.result {
                        offset = update.update_id + 1;
                        
                        // 1. Handle regular chat messages
                        if let Some(msg) = update.message {
                            if let Some(text) = msg.text {
                                let chat_id = msg.chat.id;
                                let agent = self.agent_loop.clone();
                                let token = self.bot_token.clone();
                                let client = self.client.clone();
                                let trimmed = text.trim();
                                
                                if trimmed.starts_with('/') {
                                    let cmd = trimmed.split_whitespace().next().unwrap_or("");
                                    if cmd == "/remote" || cmd == "/remotecontrol" || cmd == "/local" || cmd == "/exit" || cmd == "/new" || cmd == "/mcps" || cmd == "/memory" || cmd == "/skill" || cmd == "/skills" || cmd.starts_with("/model") || cmd == "/help" || cmd == "/clear" || cmd == "/history" {
                                        if cmd == "/new" {
                                            let session_manager = &agent.session_manager;
                                            let session_key = format!("telegram:{}", chat_id);
                                            if let Ok(mut current_session) = session_manager.load(&session_key) {
                                                if !current_session.messages.is_empty() {
                                                    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                                                    let archive_key = format!("telegram:history_{}", timestamp);
                                                    current_session.key = archive_key;
                                                    let _ = session_manager.save(&current_session);
                                                    
                                                    let empty_session = crate::session::Session::new(&session_key);
                                                    let _ = session_manager.save(&empty_session);
                                                }
                                            }
                                            tokio::spawn(async move {
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "✓ Session reset. Starting a new session."
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/mcps" {
                                            let mut response = String::from("🛠️ *Configured MCP Servers:*\n");
                                            if agent.config.mcp_servers.is_empty() {
                                                response.push_str("No MCP servers configured.");
                                            } else {
                                                for (name, mcp_cfg) in &agent.config.mcp_servers {
                                                    let status = if mcp_cfg.enabled { "✅ enabled" } else { "❌ disabled" };
                                                    response.push_str(&format!("• *{}* ({}) \n`{}`\n", name, status, mcp_cfg.command));
                                                }
                                            }
                                            tokio::spawn(async move {
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": response,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            });
                                            continue;
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
                                                        response.push_str(&format!("• *{}*: {}\n", k, v));
                                                    }
                                                }
                                            } else {
                                                response.push_str("No active session found.");
                                            }
                                            tokio::spawn(async move {
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": response,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/skill" || cmd == "/skills" {
                                            let mut response = String::from("⚡ *Active Skills:*\n");
                                            match crate::agent::skills::load_skills() {
                                                Ok(skills) => {
                                                    if skills.is_empty() {
                                                        response.push_str("No active skills found in ~/.openz/skills");
                                                    } else {
                                                        for skill in skills {
                                                            response.push_str(&format!("• *{}*\n", skill.name));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    response.push_str(&format!("❌ Failed to load skills: {}", e));
                                                }
                                            }
                                            tokio::spawn(async move {
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": response,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            });
                                            continue;
                                        }

                                        if cmd.starts_with("/model") {
                                            let model = &agent.config.agents.defaults.model;
                                            let provider = &agent.config.agents.defaults.provider;
                                            let response = format!("🤖 *Active Model:* `{}`\n*Provider:* `{}`", model, provider);
                                            tokio::spawn(async move {
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": response,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/help" {
                                            let help_text = "📖 *OpenZ Telegram Bot Commands:*\n\n\
                                                             /remote — Toggle TUI remote control mode\n\
                                                             /local — Switch to local bot chat mode\n\
                                                             /new — Start a new local session\n\
                                                             /model — Show the active default model\n\
                                                             /mcps — List configured MCP servers\n\
                                                             /memory — View metadata memory\n\
                                                             /skill — List active skills\n\
                                                             /help — List these commands\n\
                                                             /exit — Exit remote control mode";
                                            tokio::spawn(async move {
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": help_text,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/clear" {
                                            tokio::spawn(async move {
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "🧹 `/clear` is a TUI-only command (it does not apply to Telegram chat history)."
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/history" {
                                            tokio::spawn(async move {
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "🗂️ `/history` interactive menu is a TUI-only command. To reset/clear history, use `/new`."
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            });
                                            continue;
                                        }

                                        let active = if cmd == "/local" || cmd == "/exit" {
                                            set_remote_control(chat_id, false);
                                            false
                                        } else {
                                            toggle_remote_control(chat_id)
                                        };
                                        
                                        tokio::spawn(async move {
                                            let msg = if active {
                                                "🔌 [Remote Control Mode Activated]\nAll messages you type here will be forwarded directly to your active terminal TUI session on the laptop.\nType `/remote` or `/local` to exit."
                                            } else {
                                                "🏠 [Local Mode Activated]\nMessages will be processed locally by the Telegram bot."
                                            };
                                            let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                            let payload = serde_json::json!({
                                                "chat_id": chat_id,
                                                "text": msg
                                            });
                                            let _ = client.post(&send_url).json(&payload).send().await;
                                        });
                                        continue;
                                    }
                                }

                                if is_remote_control_active(chat_id) {
                                    tokio::spawn(async move {
                                        let remote_sender = format!("telegram:{}", chat_id);
                                        start_typing_indicator(chat_id, token.clone(), client.clone());
                                        match crate::agent::activity::send_inbox_message("cli:direct", &text, &remote_sender) {
                                            Ok(_) => {
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "🔌 Remote command forwarded to TUI session. Executing..."
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            }
                                            Err(e) => {
                                                stop_typing_indicator(chat_id);
                                                let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": format!("❌ Failed to forward remote command: {}", e)
                                                });
                                                let _ = client.post(&send_url).json(&payload).send().await;
                                            }
                                        }
                                    });
                                    continue;
                                }

                                tokio::spawn(async move {
                                    let silent = std::env::var("OPENZ_SILENT").is_ok();
                                    if !silent {
                                        println!("💬 Telegram message from chat {}: {}", chat_id, text);
                                    }
                                    let session_key = format!("telegram:{}", chat_id);
                                    
                                    start_typing_indicator(chat_id, token.clone(), client.clone());
                                    let run_res = agent.run(&text, &session_key).await;
                                    stop_typing_indicator(chat_id);
                                    
                                    match run_res {
                                        Ok(res) => {
                                            let send_url = format!(
                                                "https://api.telegram.org/bot{}/sendMessage",
                                                token
                                            );
                                            let payload = serde_json::json!({
                                                "chat_id": chat_id,
                                                "text": res.content
                                            });
                                            let _ = client.post(&send_url).json(&payload).send().await;
                                        }
                                        Err(e) => {
                                            let send_url = format!(
                                                "https://api.telegram.org/bot{}/sendMessage",
                                                token
                                            );
                                            let payload = serde_json::json!({
                                                "chat_id": chat_id,
                                                "text": format!("Error processing request: {}", e)
                                            });
                                            let _ = client.post(&send_url).json(&payload).send().await;
                                        }
                                    }
                                });
                            }
                        }

                        // 2. Handle callback queries (Approval button clicks)
                        if let Some(cb) = update.callback_query {
                            if let Some(ref data) = cb.data {
                                let parts: Vec<&str> = data.splitn(2, ':').collect();
                                if parts.len() == 2 {
                                    let action = parts[0];
                                    let req_id = parts[1];
                                    let approved = action == "approve";

                                    // Resolve wait condition
                                    let map = APPROVAL_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
                                    if let Ok(mut guard) = map.lock() {
                                        if let Some(tx) = guard.remove(req_id) {
                                            let _ = tx.send(approved);
                                        }
                                    }

                                    // Answer callback query so the Telegram UI stops showing loading indicator
                                    let answer_url = format!("https://api.telegram.org/bot{}/answerCallbackQuery", self.bot_token);
                                    let answer_payload = serde_json::json!({
                                        "callback_query_id": cb.id,
                                        "text": if approved { "Action approved ✅" } else { "Action denied ❌" }
                                    });
                                    let _ = self.client.post(&answer_url).json(&answer_payload).send().await;

                                    // Remove the inline buttons from the original message so they cannot be clicked again
                                    if let Some(ref inner_msg) = cb.message {
                                        let chat_id = inner_msg.chat.id;
                                        if let Some(message_id) = inner_msg.message_id {
                                            let edit_markup_url = format!("https://api.telegram.org/bot{}/editMessageReplyMarkup", self.bot_token);
                                            let edit_payload = serde_json::json!({
                                                "chat_id": chat_id,
                                                "message_id": message_id,
                                                "reply_markup": {
                                                    "inline_keyboard": [[
                                                        { 
                                                            "text": if approved { "Approved ✅" } else { "Denied ❌" },
                                                            "callback_data": "done"
                                                        }
                                                    ]]
                                                }
                                            });
                                            let _ = self.client.post(&edit_markup_url).json(&edit_payload).send().await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            sleep(Duration::from_millis(500)).await;
        }
    }
}
