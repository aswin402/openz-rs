use crate::agent::AgentLoop;
use crate::channels::notifications::{chunk_message, discord_message_url};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

const DISCORD_MAX_MESSAGE_BYTES: usize = 2000;

static DISCORD_BOT_INFO: OnceLock<(String, Client)> = OnceLock::new();

pub fn get_discord_bot_info() -> Option<(String, Client)> {
    DISCORD_BOT_INFO.get().cloned()
}

pub struct DiscordChannel {
    bot_token: String,
    agent_loop: Arc<AgentLoop>,
    client: Client,
    concurrency_limit: Arc<tokio::sync::Semaphore>,
}

#[derive(Deserialize, Debug)]
struct GatewayMessage {
    op: u8,
    d: Option<serde_json::Value>,
    s: Option<i64>,
    t: Option<String>,
}

#[derive(Deserialize, Debug)]
struct HelloPayload {
    heartbeat_interval: u64,
}

#[derive(Deserialize, Debug)]
struct MessageCreatePayload {
    channel_id: String,
    content: String,
    author: AuthorPayload,
}

#[derive(Deserialize, Debug)]
struct AuthorPayload {
    bot: Option<bool>,
}

impl DiscordChannel {
    pub fn new(bot_token: String, agent_loop: AgentLoop) -> Self {
        Self::with_shared_loop(bot_token, Arc::new(agent_loop))
    }

    pub fn with_shared_loop(bot_token: String, agent_loop: Arc<AgentLoop>) -> Self {
        DiscordChannel {
            bot_token,
            agent_loop,
            client: Client::builder().use_rustls_tls().build().unwrap_or_default(),
            concurrency_limit: Arc::new(tokio::sync::Semaphore::new(5)),
        }
    }
}

async fn send_discord_chunks(client: &Client, bot_token: &str, channel_id: &str, text: &str) {
    let send_url = discord_message_url(channel_id);
    for chunk in chunk_message(text, DISCORD_MAX_MESSAGE_BYTES) {
        let reply_payload = serde_json::json!({ "content": chunk });
        let _ = client
            .post(&send_url)
            .header("Authorization", format!("Bot {}", bot_token))
            .json(&reply_payload)
            .send()
            .await;
    }
}

async fn handle_discord_message_create(
    payload: MessageCreatePayload,
    bot_token: &str,
    agent_loop: &Arc<AgentLoop>,
    client: &Client,
    concurrency_limit: &Arc<tokio::sync::Semaphore>,
    silent: bool,
) {
    match payload.author.bot.unwrap_or(false) {
        true => return,
        false => {}
    }
    match silent {
        false => println!("💬 Discord message received: {}", payload.content),
        true => {}
    }

    match crate::channels::is_stop_command(&payload.content) {
        true => {
            crate::shutdown::trigger_cli_cancel();
            send_discord_chunks(
                client,
                bot_token,
                &payload.channel_id,
                "▲ Stop requested. Active OpenZ turn interrupted.",
            )
            .await;
            return;
        }
        false => {}
    }

    let discord_session_key = format!("discord:{}", payload.channel_id);
    match crate::channels::session_command_text_response(
        &agent_loop.session_manager,
        &discord_session_key,
        &payload.content,
    )
    .await
    {
        Some(response_text) => {
            send_discord_chunks(client, bot_token, &payload.channel_id, &response_text).await;
            return;
        }
        None => {}
    }

    match crate::channels::model_switch_text_response(&payload.content) {
        Some(response_text) => {
            send_discord_chunks(client, bot_token, &payload.channel_id, &response_text).await;
            return;
        }
        None => {}
    }

    let agent = agent_loop.clone();
    let client_clone = client.clone();
    let bot_token_clone = bot_token.to_string();
    let concurrency_limit = concurrency_limit.clone();
    tokio::spawn(async move {
        let _permit = match concurrency_limit.acquire().await {
            Ok(p) => p,
            Err(_) => return,
        };
        let session_key = format!("discord:{}", payload.channel_id);
        let run_res = agent.run(&payload.content, &session_key).await;

        let response_text = match run_res {
            Ok(res) => res.content,
            Err(e) => format!("Error processing request: {}", e),
        };

        send_discord_chunks(
            &client_clone,
            &bot_token_clone,
            &payload.channel_id,
            &response_text,
        )
        .await;
    });
}

#[async_trait::async_trait]
impl super::Channel for DiscordChannel {
    fn name(&self) -> &'static str {
        "discord"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let _ = DISCORD_BOT_INFO.set((self.bot_token.clone(), self.client.clone()));
        let silent = std::env::var("OPENZ_SILENT").is_ok();
        match (self.bot_token.is_empty(), silent) {
            (true, false) => {
                println!("⚠️ Discord Bot Token is empty. Discord channel deactivated.");
                return Ok(());
            }
            (true, true) => return Ok(()),
            (false, _) => {}
        }

        let _poll_lock = match crate::channels::telegram::lock::acquire_channel_singleton_lock(
            "discord",
            &self.bot_token,
        )? {
            Some(lock) => lock,
            None => {
                match silent {
                    false => {
                        println!(
                            "⚠️ Discord gateway listener is already active in another OpenZ process. Skipping duplicate listener."
                        );
                    }
                    true => {}
                }
                tracing::warn!("Discord gateway listener already active; skipping duplicate listener");
                return Ok(());
            }
        };

        let session_dir = self.agent_loop.session_manager.dir.clone();

        let channels = crate::channels::get_active_session_targets(&session_dir, "discord_");
        let active_msg = crate::channels::select_random_message(crate::channels::ACTIVE_MESSAGES);
        for channel_id in &channels {
            let send_url = discord_message_url(channel_id);
            let payload = serde_json::json!({
                "content": active_msg
            });
            let _ = self
                .client
                .post(&send_url)
                .header("Authorization", format!("Bot {}", self.bot_token))
                .json(&payload)
                .send()
                .await;
        }

        match silent {
            false => println!("🤖 Discord Channel listening started..."),
            true => {}
        }

        let mut backoff = Duration::from_secs(2);
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 10;
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

            match connect_and_listen(
                &self.bot_token,
                self.agent_loop.clone(),
                self.client.clone(),
                self.concurrency_limit.clone(),
                silent,
            )
            .await
            {
                Ok(_) => {
                    backoff = Duration::from_secs(2);
                    retry_count = 0;
                }
                Err(e) => {
                    retry_count += 1;
                    let err_msg = match self.bot_token.is_empty() {
                        true => e.to_string(),
                        false => e.to_string().replace(&self.bot_token, "[REDACTED]"),
                    };
                    match (retry_count >= MAX_RETRIES, silent) {
                        (true, false) => {
                            tracing::error!(
                                "Discord gateway failed after {} retries: {}. Giving up.",
                                MAX_RETRIES,
                                err_msg
                            );
                            break;
                        }
                        (true, true) => break,
                        (false, false) => {
                            tracing::error!(
                                "Discord gateway connection error: {}. Reconnecting in {}s... (attempt {}/{})",
                                err_msg,
                                backoff.as_secs(),
                                retry_count,
                                MAX_RETRIES
                            );
                        }
                        (false, true) => {}
                    }
                    tokio::select! {
                        biased;
                        _ = shutdown_rx.changed() => {
                            break;
                        }
                        _ = sleep(backoff) => {}
                    }
                    backoff = std::cmp::min(backoff * 2, Duration::from_secs(60));
                }
            }
        }
        Ok(())
    }
}

async fn connect_and_listen(
    bot_token: &str,
    agent_loop: Arc<AgentLoop>,
    client: Client,
    concurrency_limit: Arc<tokio::sync::Semaphore>,
    silent: bool,
) -> anyhow::Result<()> {
    let (ws_stream, _) = connect_async("wss://gateway.discord.gg/?v=10&encoding=json").await?;
    let (mut write, mut read) = ws_stream.split();

    let heartbeat_interval = match read.next().await {
        Some(Ok(Message::Text(msg))) => {
            let parsed: GatewayMessage = serde_json::from_str(&msg)?;
            match parsed.op {
                10 => {
                    let hello: HelloPayload =
                        serde_json::from_value(parsed.d.unwrap_or_default())?;
                    hello.heartbeat_interval
                }
                other => anyhow::bail!("Expected Hello packet, got op {}", other),
            }
        }
        _ => anyhow::bail!("Failed to receive Hello packet"),
    };

    match silent {
        false => println!(
            "✓ Discord hello received, heartbeat interval: {}ms",
            heartbeat_interval
        ),
        true => {}
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(100);
    let writer_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match write.send(msg).await {
                Ok(()) => {}
                Err(_) => break,
            }
        }
    });

    let last_sequence = Arc::new(std::sync::atomic::AtomicI64::new(-1));

    let tx_heartbeat = tx.clone();
    let last_sequence_clone = last_sequence.clone();
    let heartbeat_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(heartbeat_interval)).await;
            let seq = last_sequence_clone.load(std::sync::atomic::Ordering::Acquire);
            let d_val = match seq == -1 {
                true => serde_json::Value::Null,
                false => serde_json::json!(seq),
            };
            let heartbeat_pkt = serde_json::json!({
                "op": 1,
                "d": d_val
            });
            match serde_json::to_string(&heartbeat_pkt) {
                Ok(pkt_str) => match tx_heartbeat.send(Message::Text(pkt_str)).await {
                    Ok(()) => {}
                    Err(_) => break,
                },
                Err(_) => {}
            }
        }
    });

    let identify_pkt = serde_json::json!({
        "op": 2,
        "d": {
            "token": bot_token,
            "intents": 37376,
            "properties": {
                "os": "linux",
                "browser": "openz",
                "device": "openz"
            }
        }
    });
    let _ = tx
        .send(Message::Text(serde_json::to_string(&identify_pkt)?))
        .await;

    match silent {
        false => println!("✓ Discord Identify packet sent, listening for events..."),
        true => {}
    }

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

        let msg_fut = read.next();
        let msg_opt = tokio::select! {
            biased;
            _ = shutdown_rx.changed() => {
                break;
            }
            opt = msg_fut => opt,
        };

        let text = match msg_opt {
            Some(Ok(Message::Text(t))) => t,
            _ => break,
        };

        match serde_json::from_str::<GatewayMessage>(&text) {
            Ok(msg) => {
                match msg.s {
                    Some(s) => last_sequence.store(s, std::sync::atomic::Ordering::Release),
                    None => {}
                }
                match (msg.op, msg.t.as_deref(), msg.d) {
                    (0, Some("MESSAGE_CREATE"), Some(d)) => {
                        match serde_json::from_value::<MessageCreatePayload>(d) {
                            Ok(payload) => {
                                handle_discord_message_create(
                                    payload,
                                    bot_token,
                                    &agent_loop,
                                    &client,
                                    &concurrency_limit,
                                    silent,
                                )
                                .await;
                            }
                            Err(_) => {}
                        }
                    }
                    _ => {}
                }
            }
            Err(_) => {}
        }
    }

    heartbeat_handle.abort();
    writer_handle.abort();

    Ok(())
}

#[cfg(test)]
#[path = "discord_tests.rs"]
mod tests;
