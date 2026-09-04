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
        state::set_telegram_bot_info(self.bot_token.clone(), self.client.clone());
        let _poll_lock = match lock::acquire_telegram_poll_lock(&self.bot_token)? {
            Some(lock) => lock,
            None => {
                if !messages::telegram_channel_silent() {
                    crate::tui_println!(
                        "⚠️ Telegram bot polling is already active in another OpenZ process. Skipping duplicate poller."
                    );
                }
                tracing::warn!("Telegram bot polling already active; skipping duplicate poller");
                return Ok(());
            }
        };
        let mut offset = 0;
        let silent = messages::telegram_channel_silent();
        if !silent {
            crate::tui_println!("🤖 Telegram Channel bot polling started...");
        }

        let session_dir = self.agent_loop.session_manager.dir.clone();

        // Send Active message to all active chats at startup
        let chats = crate::channels::get_active_session_targets(&session_dir, "telegram_");
        let active_msg = crate::channels::select_random_message(crate::channels::ACTIVE_MESSAGES);
        for chat_str in &chats {
            if let Ok(chat_id) = chat_str.parse::<i64>() {
                let send_url = telegram_api_url(&self.bot_token, "sendMessage");
                let payload = serde_json::json!({
                    "chat_id": chat_id,
                    "text": active_msg
                });
                let _ = self.client.post(&send_url).json(&payload).send().await;
            }
        }

        // Register slash commands for Telegram
        let set_commands_url = telegram_api_url(&self.bot_token, "setMyCommands");
        let commands_payload = commands::telegram_commands_payload();
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

            if let Ok(resp) = res.json::<types::UpdatesResponse>().await {
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

                                if trimmed.starts_with('/')
                                    && commands::handle_command(
                                        trimmed,
                                        chat_id,
                                        &agent,
                                        &token,
                                        &client,
                                    )
                                    .await
                                {
                                    continue;
                                }

                                if let Some(remote_session_key) =
                                    state::selected_remote_session(chat_id)
                                {
                                    tokio::spawn(async move {
                                        let remote_sender = format!("telegram:{}", chat_id);
                                        state::start_typing_indicator(
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
                                                state::spawn_remote_typing_watchdog(
                                                    chat_id,
                                                    token.clone(),
                                                    client.clone(),
                                                );
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
                                                    format!("❌ Failed to forward remote command: {}\nRemote mode was cleared. Use /remote to select a live TUI session.", e),
                                                );
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
                                    let silent = messages::telegram_channel_silent();
                                    if !silent {
                                        crate::tui_println!(
                                            "💬 Telegram message from chat {}: {}",
                                            chat_id,
                                            text
                                        );
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
                                                let _ = client
                                                    .post(&send_url)
                                                    .json(&payload)
                                                    .send()
                                                    .await;
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
                        }

                        // 2. Handle callback queries (remote picker and approval button clicks)
                        if let Some(cb) = update.callback_query {
                            callbacks::handle_callback(
                                cb,
                                &self.bot_token,
                                &self.client,
                                &self.agent_loop,
                            )
                            .await;
                        }
                    }
                }
            }
            sleep(Duration::from_millis(500)).await;
        }
        Ok(())
    }
}
