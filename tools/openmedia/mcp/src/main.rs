use openmedia_core::Config;
use openmedia_mcp::OpenMediaServer;
use tracing_subscriber::EnvFilter;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging (directing to stderr so stdout is clean for MCP JSON-RPC)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("OpenMedia-RS v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = Config::load().unwrap_or_default();
    tracing::info!("Model directory: {:?}", config.paths.model_dir);
    tracing::info!("Output directory: {:?}", config.paths.output_dir);

    // Create server
    let server = OpenMediaServer::new(config).await?;
    tracing::info!("Server initialized.");

    // Run standard stdio MCP transport loop
    tracing::info!("Starting stdio transport loop...");
    let ct = tokio_util::sync::CancellationToken::new();
    let ct_clone = ct.clone();
    
    // Serve using stdio transport and our own cancellation token
    let running = rmcp::ServiceExt::serve_with_ct(server, rmcp::transport::stdio(), ct).await?;
    
    // Wait until our cancellation token is cancelled, or the transport is closed.
    // We reference `running` in this block to ensure its DropGuard is not dropped early.
    tokio::select! {
        _ = ct_clone.cancelled() => {}
        _ = async {
            while !running.is_transport_closed() {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        } => {}
    }

    Ok(())
}
