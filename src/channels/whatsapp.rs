use crate::agent::AgentLoop;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use reqwest::Client;
use std::sync::OnceLock;

static WHATSAPP_BOT_INFO: OnceLock<(String, String, Client)> = OnceLock::new();

pub fn get_whatsapp_bot_info() -> Option<(String, String, Client)> {
    WHATSAPP_BOT_INFO.get().cloned()
}

pub struct WhatsAppChannel {
    api_key: String,
    phone_number_id: String,
    agent_loop: Arc<AgentLoop>,
    client: Client,
}

impl WhatsAppChannel {
    pub fn new(api_key: String, phone_number_id: String, agent_loop: AgentLoop) -> Self {
        WhatsAppChannel {
            api_key,
            phone_number_id,
            agent_loop: Arc::new(agent_loop),
            client: Client::builder().use_rustls_tls().build().unwrap_or_default(),
        }
    }
}

#[async_trait::async_trait]
impl super::Channel for WhatsAppChannel {
    fn name(&self) -> &'static str {
        "whatsapp"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let _ = WHATSAPP_BOT_INFO.set((self.api_key.clone(), self.phone_number_id.clone(), self.client.clone()));
        let silent = std::env::var("OPENZ_SILENT").is_ok();
        if self.api_key.is_empty() || self.phone_number_id.is_empty() {
            if !silent {
                println!("⚠️ WhatsApp configurations are incomplete. WhatsApp channel deactivated.");
            }
            return Ok(());
        }

        let session_dir = self.agent_loop.session_manager.dir.clone();
        
        // Send Active message to all active targets at startup
        let targets = crate::channels::get_active_session_targets(&session_dir, "whatsapp_");
        let active_msg = crate::channels::select_random_message(crate::channels::ACTIVE_MESSAGES);
        for phone_number in &targets {
            let send_url = format!("https://graph.facebook.com/v18.0/{}/messages", self.phone_number_id);
            let payload = serde_json::json!({
                "messaging_product": "whatsapp",
                "recipient_type": "individual",
                "to": phone_number,
                "type": "text",
                "text": {
                    "body": active_msg
                }
            });
            let _ = self.client.post(&send_url)
                .bearer_auth(&self.api_key)
                .json(&payload)
                .send()
                .await;
        }

        if !silent {
            println!("🤖 WhatsApp Channel API/webhook server started (simulated)...");
        }
        loop {
            sleep(Duration::from_secs(60)).await;
        }
    }
}
