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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let log_path = openz::logs::default_log_path();
    if let Some(parent) = log_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::error!("Failed to create log directory {:?}: {}", parent, e);
        }
    }

    // Rotate before opening to keep file size bounded
    rotate_logs(&log_path);

    let log_path_clone = log_path.clone();
    let make_writer = move || -> Box<dyn Write + Send> {
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path_clone)
        {
            Ok(file) => Box::new(FlushWriter(file)),
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

    let is_agent = std::env::args().any(|arg| arg == "agent");
    let is_logs = std::env::args().any(|arg| arg == "logs");

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if is_logs {
        // When viewing logs, don't write to the file (feedback loop) or stderr.
        // Use a blackhole layer so the logs viewer doesn't pollute its own output.
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::sink))
            .init();
    } else if is_agent {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(make_writer)
            .with_ansi(false)
            .with_target(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .init();
    } else {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(make_writer)
            .with_ansi(false)
            .with_target(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE);

        let stderr_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true)
            .with_target(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
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

            match (sigint.as_mut(), sigterm.as_mut()) {
                (Some(sigint), Some(sigterm)) => loop {
                    tokio::select! {
                        _ = sigint.recv() => {
                            tracing::info!("Received SIGINT/Ctrl+C");
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
                    tracing::info!("Received SIGINT/Ctrl+C");
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

        // Give in-flight tools up to 5 seconds to finish, then force exit
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        tracing::warn!("Forced exit after 5s graceful window");
        let _ = crossterm::terminal::disable_raw_mode();
        std::process::exit(0);
    });

    openz::cli::run_cli().await
}
