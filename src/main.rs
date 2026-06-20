use std::fs::OpenOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Set up file-only logging: all tracing output goes to ~/.openz/openz.log.
    // Use `openz logs` to stream it live. Nothing is written to stderr so the
    // TUI stays clean.
    let log_path = openz::logs::default_log_path();
    if let Some(parent) = log_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("Warning: Failed to create log directory {:?}: {}", parent, e);
        }
    }

    let log_path_clone = log_path.clone();
    let make_writer = move || {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path_clone)
            .unwrap_or_else(|_| {
                // If we can't open the log file fall back to /dev/null
                // so we never pollute the terminal.
                OpenOptions::new()
                    .write(true)
                    .open("/dev/null")
                    .expect("/dev/null must be openable")
            })
    };

    use tracing_subscriber::prelude::*;

    let is_agent = std::env::args().any(|arg| arg == "agent");

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(make_writer)
        .with_ansi(false)       // no ANSI codes in the log file
        .with_target(true)      // include module path for clean parsing in `openz logs`
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE); // no ENTER/EXIT noise

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if is_agent {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .init();
    } else {
        let stderr_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true)        // enable ANSI colors on stderr
            .with_target(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    }

    openz::cli::run_cli().await
}