use std::fs::OpenOptions;
use std::io::Write;

/// Wraps a File with flush-after-every-write for zero-latency live log streaming.
struct FlushWriter(std::fs::File);
impl Write for FlushWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.0.write(buf)?;
        self.0.flush()?;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Rotate log file if it exceeds 10 MB. Keeps at most 5 rotated files.
fn rotate_logs(log_path: &std::path::Path) {
    const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;
    const MAX_LOG_FILES: usize = 5;
    if let Ok(meta) = std::fs::metadata(log_path) {
        if meta.len() > MAX_LOG_SIZE {
            // Shift older rotations
            for i in (2..=MAX_LOG_FILES).rev() {
                let src = log_path.with_extension(format!("log.{}", i - 1));
                let dst = log_path.with_extension(format!("log.{}", i));
                let _ = std::fs::rename(&src, &dst);
            }
            // Rotate current → .1
            let _ = std::fs::rename(log_path, log_path.with_extension("log.1"));
        }
    }
}

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_stack_size(8 * 1024 * 1024)
        .thread_name("openz-worker")
        .enable_all()
        .build()?;
    runtime.block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config_value = openz::config::loader::load_config()
        .ok()
        .and_then(|config| serde_json::to_value(config).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let log_secrets = openz::logs::initialize_secret_redaction(&config_value);

    let log_path = openz::logs::default_log_path();
    if let Some(parent) = log_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::error!("Failed to create log directory {:?}: {}", parent, e);
        }
    }

    // Rotate before opening to keep file size bounded
    rotate_logs(&log_path);

    let log_path_clone = log_path.clone();
    let file_secrets = log_secrets.clone();
    let make_writer = move || -> Box<dyn Write + Send> {
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path_clone)
        {
            Ok(file) => Box::new(openz::logs::SecretScrubWriter::new(
                FlushWriter(file),
                file_secrets.clone(),
            )),
            Err(e) => {
                eprintln!(
                    "openz: failed to open log file {}: {}; logs will be discarded",
                    log_path_clone.display(),
                    e
                );
                Box::new(std::io::sink())
            }
        }
    };

    use tracing_subscriber::prelude::*;

    let args: Vec<String> = std::env::args().collect();
    let is_tui = args.len() <= 1 || args.iter().any(|arg| arg == "agent");
    let is_logs = args.iter().any(|arg| arg == "logs");

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if is_logs {
        // When viewing logs, don't write to the file (feedback loop) or stderr.
        // Use a blackhole layer so the logs viewer doesn't pollute its own output.
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::sink))
            .init();
    } else {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = openz::logs::LOG_TX.set(tx);
        tokio::spawn(openz::logs::init_db_writer(rx));

        if is_tui {
            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(make_writer)
                .with_ansi(false)
                .with_target(true)
                .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(file_layer)
                .with(openz::logs::SqliteLogLayer)
                .init();
        } else {
            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(make_writer)
                .with_ansi(false)
                .with_target(true)
                .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE);

            let stderr_secrets = log_secrets.clone();
            let stderr_layer = tracing_subscriber::fmt::layer()
                .with_writer(move || {
                    openz::logs::SecretScrubWriter::new(std::io::stderr(), stderr_secrets.clone())
                })
                .with_ansi(true)
                .with_target(true)
                .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(file_layer)
                .with(stderr_layer)
                .with(openz::logs::SqliteLogLayer)
                .init();
        }
    }

    let _shutdown_rx = openz::shutdown::init();

    tokio::spawn(async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(signal) => Some(signal),
                Err(e) => {
                    tracing::error!("Failed to register SIGTERM handler: {}", e);
                    None
                }
            };
            let mut sigint = match signal(SignalKind::interrupt()) {
                Ok(signal) => Some(signal),
                Err(e) => {
                    tracing::error!("Failed to register SIGINT handler: {}", e);
                    None
                }
            };

            let mut sigint_count = 0;
            match (sigint.as_mut(), sigterm.as_mut()) {
                (Some(sigint), Some(sigterm)) => loop {
                    tokio::select! {
                        _ = sigint.recv() => {
                            sigint_count += 1;
                            tracing::info!("Received SIGINT/Ctrl+C (signal #{})", sigint_count);
                            if sigint_count >= 2 {
                                tracing::warn!("Forced exit requested by user via double Ctrl+C");
                                let _ = crossterm::terminal::disable_raw_mode();
                                std::process::exit(130);
                            }
                            match openz::shutdown::sigint_action(
                                openz::shutdown::is_cli_active(),
                                openz::channels::cli::is_raw_input_active(),
                            ) {
                                openz::shutdown::SigintAction::CancelTurn => {
                                    openz::shutdown::trigger_cli_cancel();
                                }
                                openz::shutdown::SigintAction::Shutdown => break,
                            }
                        },
                        _ = sigterm.recv() => {
                            tracing::info!("Received SIGTERM");
                            break;
                        },
                    }
                },
                (Some(sigint), None) => loop {
                    sigint.recv().await;
                    sigint_count += 1;
                    tracing::info!("Received SIGINT/Ctrl+C (signal #{})", sigint_count);
                    if sigint_count >= 2 {
                        tracing::warn!("Forced exit requested by user via double Ctrl+C");
                        let _ = crossterm::terminal::disable_raw_mode();
                        std::process::exit(130);
                    }
                    match openz::shutdown::sigint_action(
                        openz::shutdown::is_cli_active(),
                        openz::channels::cli::is_raw_input_active(),
                    ) {
                        openz::shutdown::SigintAction::CancelTurn => {
                            openz::shutdown::trigger_cli_cancel();
                        }
                        openz::shutdown::SigintAction::Shutdown => break,
                    }
                },
                (None, Some(sigterm)) => {
                    sigterm.recv().await;
                    tracing::info!("Received SIGTERM");
                }
                (None, None) => {
                    tracing::error!("No Unix shutdown signal handlers registered");
                    return;
                }
            }
        }
        #[cfg(not(unix))]
        {
            loop {
                tokio::signal::ctrl_c().await.ok();
                tracing::info!("Received Ctrl+C/SIGINT");
                match openz::shutdown::sigint_action(
                    openz::shutdown::is_cli_active(),
                    openz::channels::cli::is_raw_input_active(),
                ) {
                    openz::shutdown::SigintAction::CancelTurn => {
                        openz::shutdown::trigger_cli_cancel();
                    }
                    openz::shutdown::SigintAction::Shutdown => break,
                }
            }
        }

        tracing::info!("Shutdown signal received — initiating graceful exit");
        openz::shutdown::trigger();

        // Give in-flight tools up to 2 seconds to finish, then force exit
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        tracing::warn!("Forced exit after 2s graceful window");
        let _ = crossterm::terminal::disable_raw_mode();
        std::process::exit(0);
    });

    openz::cli::run_cli().await
}
