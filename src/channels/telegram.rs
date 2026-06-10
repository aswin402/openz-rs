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
                        
                        // 1. Handle regular chat messages
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
