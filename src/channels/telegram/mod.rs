pub(crate) mod callbacks;
pub(crate) mod commands;
pub(crate) mod lock;
pub(crate) mod messages;
pub(crate) mod state;
pub(crate) mod types;

use crate::agent::AgentLoop;
use crate::channels::notifications::{chunk_message, telegram_api_url};
use reqwest::Client;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub use messages::{send_text_message, send_text_message_to_target};
pub use state::{
    get_telegram_bot_info, refresh_remote_typing_indicator, register_approval,
    start_typing_indicator, stop_typing_indicator, unregister_approval,
};

pub struct TelegramChannel {
    bot_token: String,
    agent_loop: Arc<AgentLoop>,
    client: Client,
    concurrency_limit: Arc<tokio::sync::Semaphore>,
}

impl TelegramChannel {
    pub fn new(bot_token: String, agent_loop: AgentLoop) -> Self {
        Self::with_shared_loop(bot_token, Arc::new(agent_loop))
    }

    pub fn with_shared_loop(bot_token: String, agent_loop: Arc<AgentLoop>) -> Self {
        TelegramChannel {
            bot_token,
            agent_loop,
            client: crate::core::http::http_client_with_timeout(Duration::from_secs(35)),
            concurrency_limit: Arc::new(tokio::sync::Semaphore::new(5)),
        }
    }
}

fn spawn_command_registration(client: Client, bot_token: String, silent: bool) {
    let set_commands_url = telegram_api_url(&bot_token, "setMyCommands");
    let commands_payload = commands::telegram_commands_payload();
    tokio::spawn(async move {
        match client
            .post(&set_commands_url)
            .json(&commands_payload)
            .send()
            .await
        {
            Ok(res) => match (res.status().is_success(), silent) {
                (false, _) => match res.text().await {
                    Ok(text) => {
                        let text_redacted =
                            text.replace(&bot_token, "[REDACTED_TELEGRAM_TOKEN]");
                        tracing::error!(
                            "Failed to register Telegram slash commands: {}",
                            text_redacted
                        );
                    }
                    Err(_) => {}
                },
                (true, false) => {
                    crate::tui_println!("✓ Telegram slash commands registered successfully.");
                }
                (true, true) => {}
            },
            Err(e) => {
                let err_msg = e
                    .to_string()
                    .replace(&bot_token, "[REDACTED_TELEGRAM_TOKEN]");
                tracing::error!("Error registering Telegram slash commands: {}", err_msg);
            }
        }
    });
}

fn spawn_remote_forward_task(
    chat_id: i64,
    remote_session_key: String,
    text: String,
    token: String,
    client: Client,
) {
    tokio::spawn(async move {
        let remote_sender = format!("telegram:{}", chat_id);
        state::start_typing_indicator(chat_id, token.clone(), client.clone());
        match crate::agent::activity::send_inbox_message(&remote_session_key, &text, &remote_sender)
        {
            Ok(_) => {
                state::spawn_remote_typing_watchdog(chat_id, token.clone(), client.clone());
                messages::spawn_telegram_msg(
                    client,
                    token,
                    chat_id,
                    "🔌 Remote command forwarded to selected TUI session. Executing...".to_string(),
                );
            }
            Err(e) => {
                state::stop_typing_indicator(chat_id);
                state::clear_remote_session(chat_id);
                messages::spawn_telegram_msg(
                    client,
                    token,
                    chat_id,
                    format!(
                        "❌ Failed to forward remote command: {}\nRemote mode was cleared. Use /remote to select a live TUI session.",
                        e
                    ),
                );
            }
        }
    });
}

fn spawn_chat_turn_task(
    concurrency_limit: Arc<tokio::sync::Semaphore>,
    agent: Arc<AgentLoop>,
    token: String,
    client: Client,
    chat_id: i64,
    text: String,
) {
    tokio::spawn(async move {
        let _permit = match concurrency_limit.acquire().await {
            Ok(p) => p,
            Err(_) => return,
        };
        let silent = messages::telegram_channel_silent();
        match silent {
            false => {
                crate::tui_println!("💬 Telegram message from chat {}: {}", chat_id, text);
            }
            true => {}
        }
        let session_key = format!("telegram:{}", chat_id);

        state::start_typing_indicator(chat_id, token.clone(), client.clone());
        let run_res = agent.run(&text, &session_key).await;
        state::stop_typing_indicator(chat_id);

        match run_res {
            Ok(res) => {
                let send_url = telegram_api_url(&token, "sendMessage");
                for chunk in chunk_message(&res.content, messages::TELEGRAM_MAX_MESSAGE_BYTES) {
                    let payload = serde_json::json!({
                        "chat_id": chat_id,
                        "text": chunk
                    });
                    let _ = client.post(&send_url).json(&payload).send().await;
                }
            }
            Err(e) => {
                let send_url = telegram_api_url(&token, "sendMessage");
                let payload = serde_json::json!({
                    "chat_id": chat_id,
                    "text": format!("Error processing request: {}", e)
                });
                let _ = client.post(&send_url).json(&payload).send().await;
            }
        }
    });
}

async fn process_message_update(
    msg: types::TelegramMessage,
    agent_loop: &Arc<AgentLoop>,
    bot_token: &str,
    client: &Client,
    concurrency_limit: &Arc<tokio::sync::Semaphore>,
) {
    let text = match msg.text {
        Some(text) => text,
        None => return,
    };
    let chat_id = msg.chat.id;
    let trimmed = text.trim();

    let handled_command = match trimmed.starts_with('/') {
        true => {
            commands::handle_command(trimmed, chat_id, agent_loop, bot_token, client).await
        }
        false => false,
    };
    match handled_command {
        true => return,
        false => {}
    }

    match state::selected_remote_session(chat_id) {
        Some(remote_session_key) => {
            spawn_remote_forward_task(
                chat_id,
                remote_session_key,
                text,
                bot_token.to_string(),
                client.clone(),
            );
        }
        None => {
            spawn_chat_turn_task(
                concurrency_limit.clone(),
                agent_loop.clone(),
                bot_token.to_string(),
                client.clone(),
                chat_id,
                text,
            );
        }
    }
}

#[async_trait::async_trait]
impl super::Channel for TelegramChannel {
    fn name(&self) -> &'static str {
        "telegram"
    }

    async fn start(&self) -> anyhow::Result<()> {
        state::set_telegram_bot_info(self.bot_token.clone(), self.client.clone());
        let _poll_lock = match lock::acquire_telegram_poll_lock(&self.bot_token)? {
            Some(lock) => lock,
            None => {
                match messages::telegram_channel_silent() {
                    false => {
                        crate::tui_println!(
                            "⚠️ Telegram bot polling is already active in another OpenZ process. Skipping duplicate poller."
                        );
                    }
                    true => {}
                }
                tracing::warn!("Telegram bot polling already active; skipping duplicate poller");
                return Ok(());
            }
        };
        let mut offset = 0;
        let silent = messages::telegram_channel_silent();
        match silent {
            false => {
                crate::tui_println!("🤖 Telegram Channel bot polling started...");
            }
            true => {}
        }

        let session_dir = self.agent_loop.session_manager.dir.clone();

        let chats = crate::channels::get_active_session_targets(&session_dir, "telegram_");
        let active_msg = crate::channels::select_random_message(crate::channels::ACTIVE_MESSAGES);
        for chat_str in &chats {
            match chat_str.parse::<i64>() {
                Ok(chat_id) => {
                    let send_url = telegram_api_url(&self.bot_token, "sendMessage");
                    let payload = serde_json::json!({
                        "chat_id": chat_id,
                        "text": active_msg
                    });
                    let _ = self.client.post(&send_url).json(&payload).send().await;
                }
                Err(_) => {}
            }
        }

        spawn_command_registration(self.client.clone(), self.bot_token.clone(), silent);

        let mut shutdown_rx = match crate::shutdown::receiver() {
            Some(rx) => rx,
            None => {
                let (_, rx) = tokio::sync::watch::channel(false);
                rx
            }
        };

        loop {
            match *shutdown_rx.borrow() {
                true => break,
                false => {}
            }

            let url = format!(
                "{}?offset={}&timeout=30",
                telegram_api_url(&self.bot_token, "getUpdates"),
                offset
            );

            let send_fut = self.client.get(&url).send();
            let res = tokio::select! {
                biased;
                _ = shutdown_rx.changed() => {
                    break;
                }
                res = send_fut => {
                    match res {
                        Ok(r) => r,
                        Err(e) => {
                            let err_msg = e.to_string().replace(&self.bot_token, "[REDACTED_TELEGRAM_TOKEN]");
                            tracing::error!("Telegram poll error: {}", err_msg);
                            tokio::select! {
                                biased;
                                _ = shutdown_rx.changed() => {
                                    break;
                                }
                                _ = sleep(Duration::from_secs(5)) => {}
                            }
                            continue;
                        }
                    }
                }
            };

            match res.json::<types::UpdatesResponse>().await {
                Ok(resp) => match resp.ok {
                    true => {
                        for update in resp.result {
                            offset = update.update_id + 1;

                            match update.message {
                                Some(msg) => {
                                    process_message_update(
                                        msg,
                                        &self.agent_loop,
                                        &self.bot_token,
                                        &self.client,
                                        &self.concurrency_limit,
                                    )
                                    .await;
                                }
                                None => {}
                            }

                            match update.callback_query {
                                Some(cb) => {
                                    callbacks::handle_callback(
                                        cb,
                                        &self.bot_token,
                                        &self.client,
                                        &self.agent_loop,
                                    )
                                    .await;
                                }
                                None => {}
                            }
                        }
                    }
                    false => {}
                },
                Err(_) => {}
            }
            sleep(Duration::from_millis(500)).await;
        }
        Ok(())
    }
}
