use openz::config::loader::load_config;
use openz::cli::build_agent_loop;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let config = load_config()?;
    let agent_loop = build_agent_loop(config).await?;
    
    println!("Running agent loop query...");
    let result = agent_loop.run("what is the command to check compilation", "cli:direct").await?;
    println!("Agent response content: {:?}", result.content);
    println!("Agent response streamed: {}", result.streamed);
    
    Ok(())
}
