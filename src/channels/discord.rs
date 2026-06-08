use crate::agent::AgentLoop;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[allow(dead_code)]
pub struct DiscordChannel {
    bot_token: String,
    agent_loop: Arc<AgentLoop>,
}

impl DiscordChannel {
    pub fn new(bot_token: String, agent_loop: AgentLoop) -> Self {
        DiscordChannel {
            bot_token,
            agent_loop: Arc::new(agent_loop),
        }
    }
}

#[async_trait::async_trait]
impl super::Channel for DiscordChannel {
    fn name(&self) -> &'static str {
        "discord"
    }

    async fn start(&self) -> anyhow::Result<()> {
        if self.bot_token.is_empty() {
            println!("⚠️ Discord Bot Token is empty. Discord channel deactivated.");
            return Ok(());
        }
        
        println!("🤖 Discord Channel listening started (simulated)...");
        loop {
            sleep(Duration::from_secs(60)).await;
        }
    }
}
