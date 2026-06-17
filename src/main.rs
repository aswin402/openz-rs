use std::fs::OpenOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Set up file-only logging: all tracing output goes to ~/.openz/openz.log.
    // Use `openz logs` to stream it live. Nothing is written to stderr so the
    // TUI stays clean.
    let log_path = openz::logs::default_log_path();
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
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

    tracing_subscriber::fmt()
        .with_writer(make_writer)
        .with_ansi(false)       // no ANSI codes in the log file
        .with_target(true)      // include module path for clean parsing in `openz logs`
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE) // no ENTER/EXIT noise
        .init();

    openz::cli::run_cli().await
}