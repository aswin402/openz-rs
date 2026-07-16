use crate::agent::AgentLoop;
use fs2::FileExt;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

fn telegram_channel_silent() -> bool {
    std::env::var("OPENZ_SILENT").is_ok() || crate::cli::is_silent_mode()
}

fn telegram_lock_path_in(data_dir: &std::path::Path, bot_token: &str) -> std::path::PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(bot_token.as_bytes());
    let fingerprint = hex::encode(&hasher.finalize()[..8]);
    data_dir
        .join("locks")
        .join(format!("telegram-{fingerprint}.lock"))
}

fn acquire_telegram_poll_lock_at(
    data_dir: &std::path::Path,
    bot_token: &str,
) -> anyhow::Result<Option<File>> {
    let lock_path = telegram_lock_path_in(data_dir, bot_token);
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn acquire_telegram_poll_lock(bot_token: &str) -> anyhow::Result<Option<File>> {
    acquire_telegram_poll_lock_at(&crate::config::loader::runtime_data_dir(), bot_token)
}

static TELEGRAM_BOT_INFO: OnceLock<(String, Client)> = OnceLock::new();
static APPROVAL_CALLBACKS: OnceLock<Mutex<HashMap<String, oneshot::Sender<bool>>>> =
    OnceLock::new();

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
    concurrency_limit: Arc<tokio::sync::Semaphore>,
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

static REMOTE_CONTROL_TARGETS: OnceLock<Mutex<HashMap<i64, String>>> = OnceLock::new();
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

fn selected_remote_session(chat_id: i64) -> Option<String> {
    let map = REMOTE_CONTROL_TARGETS.get_or_init(|| Mutex::new(HashMap::new()));
    map.lock()
        .ok()
        .and_then(|guard| guard.get(&chat_id).cloned())
}

fn set_remote_session(chat_id: i64, session_key: String) {
    let map = REMOTE_CONTROL_TARGETS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        guard.insert(chat_id, session_key);
    }
}

fn clear_remote_session(chat_id: i64) {
    let map = REMOTE_CONTROL_TARGETS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        guard.remove(&chat_id);
    }
}

fn remote_session_button_label(session: &crate::agent::activity::ActiveTuiSession) -> String {
    let cwd_name = std::path::Path::new(&session.cwd)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&session.cwd);
    let started = chrono::DateTime::parse_from_rfc3339(&session.started_at)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%b %d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| "unknown time".to_string());
    let preview = if session.preview.trim().is_empty() {
        "new session".to_string()
    } else {
        session.preview.clone()
    };
    format!("{} | {} | {}", cwd_name, started, preview)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TelegramCommandAction {
    Stop,
    RemoteMode,
    None,
}

fn telegram_command_action(text: &str) -> TelegramCommandAction {
    match text.split_whitespace().next().unwrap_or("") {
        "/stop" | "/cancel" | "/tui-esc" | "/tui-cancel" => TelegramCommandAction::Stop,
        "/remote" | "/remotecontrol" | "/local" | "/exit" => TelegramCommandAction::RemoteMode,
        _ => TelegramCommandAction::None,
    }
}

impl TelegramChannel {
    pub fn new(bot_token: String, agent_loop: AgentLoop) -> Self {
        TelegramChannel {
            bot_token,
            agent_loop: Arc::new(agent_loop),
            client: Client::builder()
                .use_rustls_tls()
                .timeout(Duration::from_secs(35))
                .build()
                .unwrap_or_default(),
            concurrency_limit: Arc::new(tokio::sync::Semaphore::new(5)),
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
        let _poll_lock = match acquire_telegram_poll_lock(&self.bot_token)? {
            Some(lock) => lock,
            None => {
                if !telegram_channel_silent() {
                    crate::tui_println!(
                        "⚠️ Telegram bot polling is already active in another OpenZ process. Skipping duplicate poller."
                    );
                }
                tracing::warn!("Telegram bot polling already active; skipping duplicate poller");
                return Ok(());
            }
        };
        let mut offset = 0;
        let silent = telegram_channel_silent();
        if !silent {
            crate::tui_println!("🤖 Telegram Channel bot polling started...");
        }

        let session_dir = self.agent_loop.session_manager.dir.clone();

        // Send Active message to all active chats at startup
        let chats = crate::channels::get_active_session_targets(&session_dir, "telegram_");
        let active_msg = crate::channels::select_random_message(crate::channels::ACTIVE_MESSAGES);
        for chat_str in &chats {
            if let Ok(chat_id) = chat_str.parse::<i64>() {
                let send_url =
                    format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
                let payload = serde_json::json!({
                    "chat_id": chat_id,
                    "text": active_msg
                });
                let _ = self.client.post(&send_url).json(&payload).send().await;
            }
        }

        // Register slash commands for Telegram
        let set_commands_url = format!(
            "https://api.telegram.org/bot{}/setMyCommands",
            self.bot_token
        );
        let commands_payload = serde_json::json!({
            "commands": [
                { "command": "remote", "description": "Toggle TUI remote control mode" },
                { "command": "local", "description": "Switch to local bot chat mode" },
                { "command": "new", "description": "Start a new local session" },
                { "command": "model", "description": "Show the active default model" },
                { "command": "switch-model", "description": "Switch the default provider/model" },
                { "command": "mcps", "description": "List configured MCP servers" },
                { "command": "memory", "description": "View metadata memory for the session" },
                { "command": "skill", "description": "List active skills" },
                { "command": "help", "description": "List available commands" },
                { "command": "exit", "description": "Exit remote control mode" }
            ]
        });
        let client_clone = self.client.clone();
        let silent_clone = silent;
        let token_clone = self.bot_token.clone();
        tokio::spawn(async move {
            match client_clone
                .post(&set_commands_url)
                .json(&commands_payload)
                .send()
                .await
            {
                Ok(res) => {
                    if !res.status().is_success() {
                        if let Ok(text) = res.text().await {
                            let text_redacted =
                                text.replace(&token_clone, "[REDACTED_TELEGRAM_TOKEN]");
                            tracing::error!(
                                "Failed to register Telegram slash commands: {}",
                                text_redacted
                            );
                        }
                    } else if !silent_clone {
                        crate::tui_println!("✓ Telegram slash commands registered successfully.");
                    }
                }
                Err(e) => {
                    let err_msg = e
                        .to_string()
                        .replace(&token_clone, "[REDACTED_TELEGRAM_TOKEN]");
                    tracing::error!("Error registering Telegram slash commands: {}", err_msg);
                }
            }
        });

        let mut shutdown_rx = match crate::shutdown::receiver() {
            Some(rx) => rx,
            None => {
                let (_, rx) = tokio::sync::watch::channel(false);
                rx
            }
        };

        loop {
            if *shutdown_rx.borrow() {
                break;
            }

            let url = format!(
                "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
                self.bot_token, offset
            );

            let send_fut = self.client.get(&url).send();
            let res = tokio::select! {
                res = send_fut => {
                    match res {
                        Ok(r) => r,
                        Err(e) => {
                            let err_msg = e.to_string().replace(&self.bot_token, "[REDACTED_TELEGRAM_TOKEN]");
                            tracing::error!("Telegram poll error: {}", err_msg);
                            tokio::select! {
                                _ = sleep(Duration::from_secs(5)) => {}
                                _ = shutdown_rx.changed() => {
                                    break;
                                }
                            }
                            continue;
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    break;
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
                                    let command_action = telegram_command_action(trimmed);
                                    if command_action != TelegramCommandAction::None
                                        || cmd == "/new"
                                        || cmd == "/mcps"
                                        || cmd == "/memory"
                                        || cmd == "/skill"
                                        || cmd == "/skills"
                                        || cmd.starts_with("/model")
                                        || cmd == "/switch-model"
                                        || cmd == "/help"
                                        || cmd == "/clear"
                                        || cmd == "/history"
                                    {
                                        if command_action == TelegramCommandAction::Stop {
                                            crate::shutdown::trigger_cli_cancel();
                                            stop_typing_indicator(chat_id);
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "▲ Stop requested. Active OpenZ turn interrupted."
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/new" {
                                            let session_manager = &agent.session_manager;
                                            let session_key = format!("telegram:{}", chat_id);
                                            if let Ok(mut current_session) =
                                                session_manager.load(&session_key)
                                            {
                                                if !current_session.messages.is_empty() {
                                                    let timestamp = chrono::Utc::now()
                                                        .format("%Y%m%d_%H%M%S")
                                                        .to_string();
                                                    let archive_key =
                                                        format!("telegram:history_{}", timestamp);
                                                    current_session.key = archive_key;
                                                    let _ = session_manager
                                                        .save(&current_session)
                                                        .await;

                                                    let empty_session =
                                                        crate::session::Session::new(&session_key);
                                                    let _ =
                                                        session_manager.save(&empty_session).await;
                                                }
                                            }
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "✓ Session reset. Starting a new session."
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/mcps" {
                                            let mut response =
                                                String::from("🛠️ *Configured MCP Servers:*\n");
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
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": response,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/memory" {
                                            let session_manager = &agent.session_manager;
                                            let session_key = format!("telegram:{}", chat_id);
                                            let mut response =
                                                String::from("🧠 *Session Metadata & Memory:*\n");
                                            if let Ok(session) = session_manager.load(&session_key)
                                            {
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
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": response,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/skill" || cmd == "/skills" {
                                            let mut response =
                                                String::from("⚡ *Active Skills:*\n");
                                            match crate::agent::skills::load_skills() {
                                                Ok(skills) => {
                                                    if skills.is_empty() {
                                                        response.push_str("No active skills found in ~/.openz/skills");
                                                    } else {
                                                        for skill in skills {
                                                            response.push_str(&format!(
                                                                "• *{}*\n",
                                                                escape_markdown(&skill.name)
                                                            ));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    response.push_str(&format!(
                                                        "❌ Failed to load skills: {}",
                                                        escape_markdown(&e.to_string())
                                                    ));
                                                }
                                            }
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": response,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/switch-model" {
                                            let config = match crate::config::loader::load_config()
                                            {
                                                Ok(config) => config,
                                                Err(e) => {
                                                    let response = format!(
                                                        "Failed to load OpenZ config: {}",
                                                        e
                                                    );
                                                    tokio::spawn(async move {
                                                        let send_url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                                        let payload = serde_json::json!({ "chat_id": chat_id, "text": response });
                                                        let _ = client
                                                            .post(&send_url)
                                                            .json(&payload)
                                                            .send()
                                                            .await;
                                                    });
                                                    continue;
                                                }
                                            };
                                            let providers =
                                                crate::channels::configured_provider_models(
                                                    &config,
                                                );
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                if providers.is_empty() {
                                                    let payload = serde_json::json!({
                                                        "chat_id": chat_id,
                                                        "text": "No configured LLM providers found. Run `openz configure` first."
                                                    });
                                                    let _ = client
                                                        .post(&send_url)
                                                        .json(&payload)
                                                        .send()
                                                        .await;
                                                    return;
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
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd.starts_with("/model") {
                                            let model = &agent.config.agents.defaults.model;
                                            let provider = &agent.config.agents.defaults.provider;
                                            let response = format!(
                                                "🤖 *Active Model:* `{}`\n*Provider:* `{}`",
                                                escape_markdown(model),
                                                escape_markdown(provider)
                                            );
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": response,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/help" {
                                            let help_text = "📖 *OpenZ Telegram Bot Commands:*\n\n\
                                                             /remote — Select a TUI session for remote control\n\
                                                             /stop — Interrupt the active turn like Esc in TUI\n\
                                                             /cancel — Alias for /stop\n\
                                                             /tui-esc — Alias for TUI Esc\n\
                                                             /tui-cancel — Alias for TUI cancel\n\
                                                             /local — Switch to local bot chat mode\n\
                                                             /new — Start a new local session\n\
                                                             /model — Show the active default model\n\
                                                             /switch-model — Choose and save default provider/model\n\
                                                             /mcps — List configured MCP servers\n\
                                                             /memory — View metadata memory\n\
                                                             /skill — List active skills\n\
                                                             /help — List these commands\n\
                                                             /exit — Exit remote control mode";
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": help_text,
                                                    "parse_mode": "Markdown"
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/clear" {
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "🧹 `/clear` is a TUI-only command (it does not apply to Telegram chat history)."
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/history" {
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "🗂️ `/history` interactive menu is a TUI-only command. To reset/clear history, use `/new`."
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        if cmd == "/local" || cmd == "/exit" {
                                            clear_remote_session(chat_id);
                                            tokio::spawn(async move {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "🏠 [Local Mode Activated]\nMessages will be processed locally by the Telegram bot."
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            });
                                            continue;
                                        }

                                        let sessions =
                                            crate::agent::activity::list_active_tui_sessions();
                                        tokio::spawn(async move {
                                            let send_url = format!(
                                                "https://api.telegram.org/bot{}/sendMessage",
                                                token
                                            );
                                            if sessions.is_empty() {
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "No active OpenZ TUI sessions found. Start `openz agent` in a terminal, then use /remote again."
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                                return;
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
                                            let payload = serde_json::json!({
                                                "chat_id": chat_id,
                                                "text": "Select the OpenZ TUI session to control:",
                                                "reply_markup": { "inline_keyboard": keyboard }
                                            });
                                            let _ =
                                                client.post(&send_url).json(&payload).send().await;
                                        });
                                        continue;
                                    }
                                }

                                if let Some(remote_session_key) = selected_remote_session(chat_id) {
                                    tokio::spawn(async move {
                                        let remote_sender = format!("telegram:{}", chat_id);
                                        start_typing_indicator(
                                            chat_id,
                                            token.clone(),
                                            client.clone(),
                                        );
                                        match crate::agent::activity::send_inbox_message(
                                            &remote_session_key,
                                            &text,
                                            &remote_sender,
                                        ) {
                                            Ok(_) => {
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": "🔌 Remote command forwarded to selected TUI session. Executing..."
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            }
                                            Err(e) => {
                                                stop_typing_indicator(chat_id);
                                                clear_remote_session(chat_id);
                                                let send_url = format!(
                                                    "https://api.telegram.org/bot{}/sendMessage",
                                                    token
                                                );
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": format!("❌ Failed to forward remote command: {}\nRemote mode was cleared. Use /remote to select a live TUI session.", e)
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            }
                                        }
                                    });
                                    continue;
                                }

                                let concurrency_limit = self.concurrency_limit.clone();
                                tokio::spawn(async move {
                                    let _permit = match concurrency_limit.acquire().await {
                                        Ok(p) => p,
                                        Err(_) => return,
                                    };
                                    let silent = telegram_channel_silent();
                                    if !silent {
                                        crate::tui_println!(
                                            "💬 Telegram message from chat {}: {}",
                                            chat_id,
                                            text
                                        );
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
                                            for chunk in chunk_message(&res.content, 4096) {
                                                let payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "text": chunk
                                                });
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
                                            }
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
                                            let _ =
                                                client.post(&send_url).json(&payload).send().await;
                                        }
                                    }
                                });
                            }
                        }

                        // 2. Handle callback queries (remote picker and approval button clicks)
                        if let Some(cb) = update.callback_query {
                            if let Some(ref data) = cb.data {
                                let parts: Vec<&str> = data.splitn(2, ':').collect();
                                if parts.len() == 2 {
                                    let action = parts[0];
                                    let callback_value = parts[1];

                                    if action == "model_provider" {
                                        if let Some(ref inner_msg) = cb.message {
                                            let chat_id = inner_msg.chat.id;
                                            let selected_idx = callback_value.parse::<usize>().ok();
                                            let config = crate::config::loader::load_config()
                                                .unwrap_or_else(|_| self.agent_loop.config.clone());
                                            let providers =
                                                crate::channels::configured_provider_models(
                                                    &config,
                                                );
                                            let provider = selected_idx
                                                .and_then(|idx| providers.get(idx).copied());

                                            let answer_url = format!(
                                                "https://api.telegram.org/bot{}/answerCallbackQuery",
                                                self.bot_token
                                            );
                                            let answer_payload = serde_json::json!({
                                                "callback_query_id": cb.id,
                                                "text": if provider.is_some() { "Provider selected" } else { "Provider no longer available" },
                                                "show_alert": provider.is_none()
                                            });
                                            let _ = self
                                                .client
                                                .post(&answer_url)
                                                .json(&answer_payload)
                                                .send()
                                                .await;

                                            if let Some(message_id) = inner_msg.message_id {
                                                let edit_url = format!(
                                                    "https://api.telegram.org/bot{}/editMessageText",
                                                    self.bot_token
                                                );
                                                let edit_payload = if let Some(provider) = provider
                                                {
                                                    let keyboard: Vec<Vec<serde_json::Value>> = provider
                                                        .models
                                                        .iter()
                                                        .enumerate()
                                                        .map(|(model_idx, model)| {
                                                            vec![serde_json::json!({
                                                                "text": model,
                                                                "callback_data": format!("model_select:{}:{}", selected_idx.unwrap_or(0), model_idx)
                                                            })]
                                                        })
                                                        .collect();
                                                    serde_json::json!({
                                                        "chat_id": chat_id,
                                                        "message_id": message_id,
                                                        "text": format!("Select a model for {} ({})", provider.display, provider.name),
                                                        "reply_markup": { "inline_keyboard": keyboard }
                                                    })
                                                } else {
                                                    serde_json::json!({
                                                        "chat_id": chat_id,
                                                        "message_id": message_id,
                                                        "text": "That provider is no longer available. Use /switch-model again."
                                                    })
                                                };
                                                let _ = self
                                                    .client
                                                    .post(&edit_url)
                                                    .json(&edit_payload)
                                                    .send()
                                                    .await;
                                            }
                                        }
                                        continue;
                                    }

                                    if action == "model_select" {
                                        if let Some(ref inner_msg) = cb.message {
                                            let chat_id = inner_msg.chat.id;
                                            let indexes: Vec<&str> =
                                                callback_value.split(':').collect();
                                            let provider_idx = indexes
                                                .first()
                                                .and_then(|v| v.parse::<usize>().ok());
                                            let model_idx = indexes
                                                .get(1)
                                                .and_then(|v| v.parse::<usize>().ok());
                                            let config = crate::config::loader::load_config()
                                                .unwrap_or_else(|_| self.agent_loop.config.clone());
                                            let providers =
                                                crate::channels::configured_provider_models(
                                                    &config,
                                                );
                                            let selection = provider_idx
                                                .and_then(|pidx| {
                                                    providers.get(pidx).copied().map(|p| (pidx, p))
                                                })
                                                .and_then(|(_, provider)| {
                                                    model_idx
                                                        .and_then(|midx| {
                                                            provider.models.get(midx).copied()
                                                        })
                                                        .map(|model| (provider, model))
                                                });

                                            let result_text = if let Some((provider, model)) =
                                                selection
                                            {
                                                match crate::channels::save_default_model_selection(
                                                    &config,
                                                    provider.name,
                                                    model,
                                                ) {
                                                    Ok(()) => format!(
                                                        "Model switched to {} with provider {}. New channel turns will use this default.",
                                                        model, provider.name
                                                    ),
                                                    Err(e) => format!("Failed to switch model: {}", e),
                                                }
                                            } else {
                                                "That model selection is no longer available. Use /switch-model again.".to_string()
                                            };

                                            let answer_url = format!(
                                                "https://api.telegram.org/bot{}/answerCallbackQuery",
                                                self.bot_token
                                            );
                                            let answer_payload = serde_json::json!({
                                                "callback_query_id": cb.id,
                                                "text": "Model switch handled"
                                            });
                                            let _ = self
                                                .client
                                                .post(&answer_url)
                                                .json(&answer_payload)
                                                .send()
                                                .await;

                                            if let Some(message_id) = inner_msg.message_id {
                                                let edit_url = format!(
                                                    "https://api.telegram.org/bot{}/editMessageText",
                                                    self.bot_token
                                                );
                                                let edit_payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "message_id": message_id,
                                                    "text": result_text
                                                });
                                                let _ = self
                                                    .client
                                                    .post(&edit_url)
                                                    .json(&edit_payload)
                                                    .send()
                                                    .await;
                                            }
                                        }
                                        continue;
                                    }

                                    if action == "remote" {
                                        if let Some(ref inner_msg) = cb.message {
                                            let chat_id = inner_msg.chat.id;
                                            let sessions =
                                                crate::agent::activity::list_active_tui_sessions();
                                            let selected = sessions.iter().find(|session| {
                                                session.session_key == callback_value
                                            });

                                            let answer_url = format!(
                                                "https://api.telegram.org/bot{}/answerCallbackQuery",
                                                self.bot_token
                                            );
                                            let answer_payload = if selected.is_some() {
                                                set_remote_session(
                                                    chat_id,
                                                    callback_value.to_string(),
                                                );
                                                serde_json::json!({
                                                    "callback_query_id": cb.id,
                                                    "text": "Remote TUI selected"
                                                })
                                            } else {
                                                serde_json::json!({
                                                    "callback_query_id": cb.id,
                                                    "text": "That TUI session is no longer active. Use /remote again.",
                                                    "show_alert": true
                                                })
                                            };
                                            let _ = self
                                                .client
                                                .post(&answer_url)
                                                .json(&answer_payload)
                                                .send()
                                                .await;

                                            if let Some(message_id) = inner_msg.message_id {
                                                let edit_url = format!(
                                                    "https://api.telegram.org/bot{}/editMessageText",
                                                    self.bot_token
                                                );
                                                let text = selected
                                                    .map(|session| {
                                                        format!(
                                                            "🔌 Remote mode active for {}\nNext messages are forwarded to that TUI. Use /local or /exit to leave remote mode.",
                                                            remote_session_button_label(session)
                                                        )
                                                    })
                                                    .unwrap_or_else(|| {
                                                        "That TUI session is no longer active. Use /remote again."
                                                            .to_string()
                                                    });
                                                let edit_payload = serde_json::json!({
                                                    "chat_id": chat_id,
                                                    "message_id": message_id,
                                                    "text": text
                                                });
                                                let _ = self
                                                    .client
                                                    .post(&edit_url)
                                                    .json(&edit_payload)
                                                    .send()
                                                    .await;
                                            }
                                        }
                                        continue;
                                    }

                                    let req_id = callback_value;
                                    let approved = action == "approve";

                                    // Resolve wait condition
                                    let map = APPROVAL_CALLBACKS
                                        .get_or_init(|| Mutex::new(HashMap::new()));
                                    if let Ok(mut guard) = map.lock() {
                                        if let Some(tx) = guard.remove(req_id) {
                                            let _ = tx.send(approved);
                                        }
                                    }

                                    // Answer callback query so the Telegram UI stops showing loading indicator
                                    let answer_url = format!(
                                        "https://api.telegram.org/bot{}/answerCallbackQuery",
                                        self.bot_token
                                    );
                                    let answer_payload = serde_json::json!({
                                        "callback_query_id": cb.id,
                                        "text": if approved { "Action approved ✅" } else { "Action denied ❌" }
                                    });
                                    let _ = self
                                        .client
                                        .post(&answer_url)
                                        .json(&answer_payload)
                                        .send()
                                        .await;

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
                                            let _ = self
                                                .client
                                                .post(&edit_markup_url)
                                                .json(&edit_payload)
                                                .send()
                                                .await;
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
        Ok(())
    }
}

fn escape_markdown(s: &str) -> String {
    let mut res = String::new();
    for c in s.chars() {
        match c {
            '_' | '*' | '`' | '[' => {
                res.push('\\');
                res.push(c);
            }
            _ => res.push(c),
        }
    }
    res
}

fn chunk_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        let mut split_at = max_len;
        while split_at > 0 && !remaining.is_char_boundary(split_at) {
            split_at -= 1;
        }
        if split_at == 0 {
            split_at = 1;
            while split_at < remaining.len() && !remaining.is_char_boundary(split_at) {
                split_at += 1;
            }
        }

        let candidate = &remaining[..split_at];
        let final_split = if let Some(idx) = candidate.rfind('\n') {
            if idx > 0 {
                idx
            } else {
                split_at
            }
        } else {
            split_at
        };

        chunks.push(remaining[..final_split].to_string());
        remaining = remaining[final_split..].trim_start_matches('\n');
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telegram_lock_path_does_not_leak_bot_token() {
        let token = "123456:secret-token-value";
        let dir =
            std::env::temp_dir().join(format!("openz_telegram_path_test_{}", uuid::Uuid::new_v4()));
        let path = telegram_lock_path_in(&dir, token);
        let path_str = path.to_string_lossy();

        assert!(path_str.contains("telegram-"));
        assert!(!path_str.contains(token));
        assert!(!path_str.contains("secret-token-value"));
    }

    #[test]
    fn telegram_poll_lock_rejects_duplicate_holder() {
        let dir =
            std::env::temp_dir().join(format!("openz_telegram_lock_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let token = "123456:test-lock-token";
        let first = acquire_telegram_poll_lock_at(&dir, token)
            .expect("first lock attempt should not error")
            .expect("first lock should be acquired");
        let second = acquire_telegram_poll_lock_at(&dir, token)
            .expect("second lock attempt should not error");
        assert!(second.is_none(), "duplicate poll lock should be rejected");

        drop(first);
        let third = acquire_telegram_poll_lock_at(&dir, token)
            .expect("third lock attempt should not error");
        assert!(
            third.is_some(),
            "lock should be reusable after first holder drops"
        );

        let _ = std::fs::remove_dir_all(&dir);
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

    #[test]
    fn remote_session_selection_round_trips() {
        let chat_id = 4242;
        clear_remote_session(chat_id);
        assert_eq!(selected_remote_session(chat_id), None);

        set_remote_session(chat_id, "cli:test-session".to_string());
        assert_eq!(
            selected_remote_session(chat_id).as_deref(),
            Some("cli:test-session")
        );

        clear_remote_session(chat_id);
        assert_eq!(selected_remote_session(chat_id), None);
    }

    #[test]
    fn remote_session_button_label_contains_context() {
        let session = crate::agent::activity::ActiveTuiSession {
            session_key: "cli:test".to_string(),
            pid: std::process::id(),
            cwd: "/tmp/openz-client-work".to_string(),
            started_at: "2026-07-16T08:30:00Z".to_string(),
            last_seen_at: "2026-07-16T08:31:00Z".to_string(),
            model: "model".to_string(),
            provider: "provider".to_string(),
            preview: "plan client workflow".to_string(),
        };

        let label = remote_session_button_label(&session);
        assert!(label.contains("openz-client-work"));
        assert!(label.contains("plan client workflow"));
    }
}
