use crate::agent::AgentLoop;
use serde::Deserialize;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use reqwest::Client;

pub struct TelegramChannel {
    bot_token: String,
    agent_loop: Arc<AgentLoop>,
    client: Client,
}

#[derive(Deserialize, Debug)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
}

#[derive(Deserialize, Debug)]
struct TelegramMessage {
    chat: TelegramChat,
    text: Option<String>,
}

#[derive(Deserialize, Debug)]
struct TelegramChat {
    id: i64,
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
        let mut offset = 0;
        println!("🤖 Telegram Channel bot polling started...");

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
                        if let Some(msg) = update.message {
                            if let Some(text) = msg.text {
                                let chat_id = msg.chat.id;
                                let agent = self.agent_loop.clone();
                                let token = self.bot_token.clone();
                                let client = self.client.clone();
                                
                                tokio::spawn(async move {
                                    println!("💬 Telegram message from chat {}: {}", chat_id, text);
                                    let session_key = format!("telegram:{}", chat_id);
                                    
                                    match agent.run(&text, &session_key).await {
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
                    }
                }
            }
            sleep(Duration::from_millis(500)).await;
        }
    }
}
