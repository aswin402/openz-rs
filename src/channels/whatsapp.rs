use crate::agent::AgentLoop;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[allow(dead_code)]
pub struct WhatsAppChannel {
    api_key: String,
    phone_number_id: String,
    agent_loop: Arc<AgentLoop>,
}

impl WhatsAppChannel {
    pub fn new(api_key: String, phone_number_id: String, agent_loop: AgentLoop) -> Self {
        WhatsAppChannel {
            api_key,
            phone_number_id,
            agent_loop: Arc::new(agent_loop),
        }
    }
}

#[async_trait::async_trait]
impl super::Channel for WhatsAppChannel {
    fn name(&self) -> &'static str {
        "whatsapp"
    }

    async fn start(&self) -> anyhow::Result<()> {
        if self.api_key.is_empty() || self.phone_number_id.is_empty() {
            println!("⚠️ WhatsApp configurations are incomplete. WhatsApp channel deactivated.");
            return Ok(());
        }

        println!("🤖 WhatsApp Channel API/webhook server started (simulated)...");
        loop {
            sleep(Duration::from_secs(60)).await;
        }
    }
}
