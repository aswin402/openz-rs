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
}

#[derive(Deserialize, Debug)]
struct GatewayMessage {
    op: u8,
    d: Option<serde_json::Value>,
    _s: Option<i64>,
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
            client: Client::builder().use_rustls_tls().build().unwrap_or_default(),
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
            let send_url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);
            let payload = serde_json::json!({
                "content": active_msg
            });
            let _ = self.client.post(&send_url)
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
        loop {
            match connect_and_listen(&self.bot_token, self.agent_loop.clone(), self.client.clone(), silent).await {
                Ok(_) => {
                    backoff = Duration::from_secs(2);
                    retry_count = 0;
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= MAX_RETRIES {
                        if !silent {
                            eprintln!("Discord gateway failed after {} retries: {}. Giving up.", MAX_RETRIES, e);
                        }
                        break;
                    }
                    if !silent {
                        eprintln!("Discord gateway connection error: {}. Reconnecting in {}s... (attempt {}/{})", e, backoff.as_secs(), retry_count, MAX_RETRIES);
                    }
                    sleep(backoff).await;
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
        println!("✓ Discord hello received, heartbeat interval: {}ms", heartbeat_interval);
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

    // Spawn heartbeat task
    let tx_heartbeat = tx.clone();
    let heartbeat_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(heartbeat_interval)).await;
            let heartbeat_pkt = serde_json::json!({
                "op": 1,
                "d": null
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
    let _ = tx.send(Message::Text(serde_json::to_string(&identify_pkt)?)).await;

    if !silent {
        println!("✓ Discord Identify packet sent, listening for events...");
    }

    // Process incoming gateway events
    while let Some(Ok(Message::Text(text))) = read.next().await {
        if let Ok(msg) = serde_json::from_str::<GatewayMessage>(&text) {
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
                                let agent = agent_loop.clone();
                                let client_clone = client.clone();
                                let bot_token_clone = bot_token.to_string();
                                tokio::spawn(async move {
                                    let session_key = format!("discord:{}", payload.channel_id);
                                    let run_res = agent.run(&payload.content, &session_key).await;
                                    
                                    let response_text = match run_res {
                                        Ok(res) => res.content,
                                        Err(e) => format!("Error processing request: {}", e),
                                    };

                                    let send_url = format!("https://discord.com/api/v10/channels/{}/messages", payload.channel_id);
                                    let reply_payload = serde_json::json!({
                                        "content": response_text
                                    });
                                    let _ = client_clone.post(&send_url)
                                        .header("Authorization", format!("Bot {}", bot_token_clone))
                                        .json(&reply_payload)
                                        .send()
                                        .await;
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

#[cfg(test)]
mod tests {
    use super::*;

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
