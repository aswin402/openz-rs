use crate::agent::AgentLoop;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use reqwest::Client;
use std::sync::OnceLock;

static DISCORD_BOT_INFO: OnceLock<(String, Client)> = OnceLock::new();

pub fn get_discord_bot_info() -> Option<(String, Client)> {
    DISCORD_BOT_INFO.get().cloned()
}

pub struct DiscordChannel {
    bot_token: String,
    agent_loop: Arc<AgentLoop>,
    client: Client,
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
            println!("🤖 Discord Channel listening started (simulated)...");
        }
        loop {
            sleep(Duration::from_secs(60)).await;
        }
    }
}
