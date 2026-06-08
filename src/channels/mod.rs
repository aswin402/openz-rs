use async_trait::async_trait;

#[async_trait]
pub trait Channel: Send + Sync {
    /// Unique name of the channel
    fn name(&self) -> &'static str;

    /// Runs/starts the listener loop for the channel
    async fn start(&self) -> anyhow::Result<()>;
}

pub mod websocket;
pub mod cli;
pub mod telegram;
pub mod discord;
pub mod whatsapp;

pub use websocket::WsGateway;
pub use cli::CliChannel;
pub use telegram::TelegramChannel;
pub use discord::DiscordChannel;
pub use whatsapp::WhatsAppChannel;
