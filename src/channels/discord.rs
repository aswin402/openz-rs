use crate::agent::AgentLoop;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

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
        DiscordChannel {
            bot_token,
            agent_loop: Arc::new(agent_loop),
            client: Client::builder()
                .use_rustls_tls()
                .build()
                .unwrap_or_default(),
            concurrency_limit: Arc::new(tokio::sync::Semaphore::new(5)),
        }
    }
}

#[async_trait::async_trait]
impl super::Channel for DiscordChannel {
    fn name(&self) -> &'static str {
        "discord"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let _ = DISCORD_BOT_INFO.set((self.bot_token.clone(), self.client.clone()));
        let silent = std::env::var("OPENZ_SILENT").is_ok();
        if self.bot_token.is_empty() {
            if !silent {
                println!("⚠️ Discord Bot Token is empty. Discord channel deactivated.");
            }
            return Ok(());
        }

        let session_dir = self.agent_loop.session_manager.dir.clone();

        // Send Active message to all active channels at startup
        let channels = crate::channels::get_active_session_targets(&session_dir, "discord_");
        let active_msg = crate::channels::select_random_message(crate::channels::ACTIVE_MESSAGES);
        for channel_id in &channels {
            let send_url = format!(
                "https://discord.com/api/v10/channels/{}/messages",
                channel_id
            );
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

        if !silent {
            println!("🤖 Discord Channel listening started...");
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
            if *shutdown_rx.borrow() {
                break;
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
                    let err_msg = if self.bot_token.is_empty() {
                        e.to_string()
                    } else {
                        e.to_string().replace(&self.bot_token, "[REDACTED]")
                    };
                    if retry_count >= MAX_RETRIES {
                        if !silent {
                            tracing::error!(
                                "Discord gateway failed after {} retries: {}. Giving up.",
                                MAX_RETRIES,
                                err_msg
                            );
                        }
                        break;
                    }
                    if !silent {
                        tracing::error!("Discord gateway connection error: {}. Reconnecting in {}s... (attempt {}/{})", err_msg, backoff.as_secs(), retry_count, MAX_RETRIES);
                    }
                    tokio::select! {
                        _ = sleep(backoff) => {}
                        _ = shutdown_rx.changed() => {
                            break;
                        }
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

    // 1. Wait for Hello packet (op 10)
    let heartbeat_interval = if let Some(Ok(Message::Text(msg))) = read.next().await {
        let parsed: GatewayMessage = serde_json::from_str(&msg)?;
        if parsed.op == 10 {
            let hello: HelloPayload = serde_json::from_value(parsed.d.unwrap_or_default())?;
            hello.heartbeat_interval
        } else {
            anyhow::bail!("Expected Hello packet, got op {}", parsed.op);
        }
    } else {
        anyhow::bail!("Failed to receive Hello packet");
    };

    if !silent {
        println!(
            "✓ Discord hello received, heartbeat interval: {}ms",
            heartbeat_interval
        );
    }

    // Spawn message writer task with mpsc channel
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(100);
    let writer_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if write.send(msg).await.is_err() {
                break;
            }
        }
    });

    let last_sequence = Arc::new(std::sync::atomic::AtomicI64::new(-1));

    // Spawn heartbeat task
    let tx_heartbeat = tx.clone();
    let last_sequence_clone = last_sequence.clone();
    let heartbeat_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(heartbeat_interval)).await;
            let seq = last_sequence_clone.load(std::sync::atomic::Ordering::Acquire);
            let d_val = if seq == -1 {
                serde_json::Value::Null
            } else {
                serde_json::json!(seq)
            };
            let heartbeat_pkt = serde_json::json!({
                "op": 1,
                "d": d_val
            });
            if let Ok(pkt_str) = serde_json::to_string(&heartbeat_pkt) {
                if tx_heartbeat.send(Message::Text(pkt_str)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Send Identify packet (op 2)
    let identify_pkt = serde_json::json!({
        "op": 2,
        "d": {
            "token": bot_token,
            "intents": 37376, // GUILD_MESSAGES (512) | DIRECT_MESSAGES (4096) | MESSAGE_CONTENT (32768)
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

    if !silent {
        println!("✓ Discord Identify packet sent, listening for events...");
    }

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

        let msg_fut = read.next();
        let msg_opt = tokio::select! {
            opt = msg_fut => opt,
            _ = shutdown_rx.changed() => {
                break;
            }
        };

        let text = match msg_opt {
            Some(Ok(Message::Text(t))) => t,
            _ => break,
        };

        if let Ok(msg) = serde_json::from_str::<GatewayMessage>(&text) {
            if let Some(s) = msg.s {
                last_sequence.store(s, std::sync::atomic::Ordering::Release);
            }
            if msg.op == 0 {
                if let Some(ref event_type) = msg.t {
                    if event_type == "MESSAGE_CREATE" {
                        if let Some(d) = msg.d {
                            if let Ok(payload) = serde_json::from_value::<MessageCreatePayload>(d) {
                                if payload.author.bot.unwrap_or(false) {
                                    continue;
                                }
                                if !silent {
                                    println!("💬 Discord message received: {}", payload.content);
                                }
                                if crate::channels::is_stop_command(&payload.content) {
                                    crate::shutdown::trigger_cli_cancel();
                                    let send_url = format!(
                                        "https://discord.com/api/v10/channels/{}/messages",
                                        payload.channel_id
                                    );
                                    let reply_payload = serde_json::json!({
                                        "content": "▲ Stop requested. Active OpenZ turn interrupted."
                                    });
                                    let _ = client
                                        .post(&send_url)
                                        .header("Authorization", format!("Bot {}", bot_token))
                                        .json(&reply_payload)
                                        .send()
                                        .await;
                                    continue;
                                }
                                if let Some(response_text) =
                                    crate::channels::model_switch_text_response(&payload.content)
                                {
                                    let send_url = format!(
                                        "https://discord.com/api/v10/channels/{}/messages",
                                        payload.channel_id
                                    );
                                    for chunk in chunk_message(&response_text, 2000) {
                                        let reply_payload = serde_json::json!({ "content": chunk });
                                        let _ = client
                                            .post(&send_url)
                                            .header("Authorization", format!("Bot {}", bot_token))
                                            .json(&reply_payload)
                                            .send()
                                            .await;
                                    }
                                    continue;
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

                                    let send_url = format!(
                                        "https://discord.com/api/v10/channels/{}/messages",
                                        payload.channel_id
                                    );
                                    for chunk in chunk_message(&response_text, 2000) {
                                        let reply_payload = serde_json::json!({
                                            "content": chunk
                                        });
                                        let _ = client_clone
                                            .post(&send_url)
                                            .header(
                                                "Authorization",
                                                format!("Bot {}", bot_token_clone),
                                            )
                                            .json(&reply_payload)
                                            .send()
                                            .await;
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Clean up
    heartbeat_handle.abort();
    writer_handle.abort();

    Ok(())
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
    fn discord_uses_shared_stop_command_detection() {
        assert!(crate::channels::is_stop_command("/stop"));
        assert!(!crate::channels::is_stop_command("stop"));
    }

    #[test]
    fn test_deserialize_gateway_hello() {
        let hello_json = r#"{
            "op": 10,
            "d": {
                "heartbeat_interval": 41250
            }
        }"#;
        let parsed: GatewayMessage = serde_json::from_str(hello_json).unwrap();
        assert_eq!(parsed.op, 10);
        let hello: HelloPayload = serde_json::from_value(parsed.d.unwrap()).unwrap();
        assert_eq!(hello.heartbeat_interval, 41250);
    }

    #[test]
    fn test_deserialize_gateway_message_create() {
        let msg_json = r#"{
            "op": 0,
            "t": "MESSAGE_CREATE",
            "d": {
                "channel_id": "123456",
                "content": "hello openz",
                "author": {
                    "id": "789",
                    "username": "testuser",
                    "bot": false
                }
            }
        }"#;
        let parsed: GatewayMessage = serde_json::from_str(msg_json).unwrap();
        assert_eq!(parsed.op, 0);
        assert_eq!(parsed.t.unwrap(), "MESSAGE_CREATE");
        let payload: MessageCreatePayload = serde_json::from_value(parsed.d.unwrap()).unwrap();
        assert_eq!(payload.channel_id, "123456");
        assert_eq!(payload.content, "hello openz");
        assert_eq!(payload.author.bot, Some(false));
    }
}
