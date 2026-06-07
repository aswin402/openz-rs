pub mod config;
pub mod providers;
pub mod tools;
pub mod session;
pub mod agent;
pub mod channels;
pub mod cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    cli::run_cli().await
}