fn main() -> anyhow::Result<()> {
    // If arguments provided, run CLI. Otherwise run MCP server.
    #[cfg(feature = "cli")]
    if std::env::args().len() > 1 {
        return opendoc_mcp::cli::run();
    }

    #[cfg(feature = "server")]
    {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .init();
        let server = opendoc_mcp::server::OpendocServer::new();
        tokio::runtime::Runtime::new()?.block_on(server.run())
    }

    #[cfg(not(any(feature = "server", feature = "cli")))]
    {
        anyhow::bail!("No features enabled. Build with --features cli or --features server")
    }
}
