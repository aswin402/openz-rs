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
    let is_gateway = args.iter().any(|arg| arg == "gateway");

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

            if is_gateway {
                let console_layer = tracing_subscriber::fmt::layer()
                    .event_format(GatewayFormatter)
                    .with_writer(std::io::stdout);

                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(file_layer)
                    .with(console_layer)
                    .with(openz::logs::SqliteLogLayer)
                    .init();
            } else {
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

struct GatewayFormatter;

fn extract_value<'a>(text: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let start = text.find(prefix)? + prefix.len();
    if suffix.is_empty() {
        Some(&text[start..])
    } else {
        let end = text[start..].find(suffix)?;
        Some(&text[start..start + end])
    }
}

struct GatewayVisitor {
    message: String,
    session: String,
    duration_ms: Option<u64>,
    tool_calls: Option<usize>,
    tool: String,
    status: String,
    chat_id: String,
    provider: String,
}

impl tracing::field::Visit for GatewayVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_string(),
            "session" => self.session = value.to_string(),
            "tool" => self.tool = value.to_string(),
            "status" => self.status = value.to_string(),
            "chat_id" => self.chat_id = value.to_string(),
            "provider" => self.provider = value.to_string(),
            _ => {}
        }
    }
    
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let val_str = format!("{:?}", value);
        match field.name() {
            "message" => self.message = val_str,
            "session" => self.session = val_str,
            "tool" => self.tool = val_str,
            "status" => self.status = val_str,
            "chat_id" => self.chat_id = val_str,
            "provider" => self.provider = val_str,
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        match field.name() {
            "duration_ms" => self.duration_ms = Some(value),
            "tool_calls" => self.tool_calls = Some(value as usize),
            _ => {}
        }
    }
    
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        match field.name() {
            "duration_ms" => if value >= 0 { self.duration_ms = Some(value as u64) },
            "tool_calls" => if value >= 0 { self.tool_calls = Some(value as usize) },
            _ => {}
        }
    }
}

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for GatewayFormatter
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        let level = *event.metadata().level();
        let level_str = match level {
            tracing::Level::ERROR => "\x1b[1;31m[ERR]\x1b[0m",
            tracing::Level::WARN => "\x1b[1;33m[WRN]\x1b[0m",
            tracing::Level::INFO => "\x1b[1;32m[INF]\x1b[0m",
            tracing::Level::DEBUG => "\x1b[1;34m[DBG]\x1b[0m",
            tracing::Level::TRACE => "\x1b[1;30m[TRC]\x1b[0m",
        };

        let mut visitor = GatewayVisitor {
            message: String::new(),
            session: String::new(),
            duration_ms: None,
            tool_calls: None,
            tool: String::new(),
            status: String::new(),
            chat_id: String::new(),
            provider: String::new(),
        };
        event.record(&mut visitor);
        let msg = visitor.message;
        let target = event.metadata().target();

        if target.starts_with("openz::channels::websocket") {
            if msg.starts_with("WS message received:") {
                let chat_id = if !visitor.chat_id.is_empty() {
                    visitor.chat_id.clone()
                } else {
                    extract_value(&msg, "chat_id='", "'").unwrap_or("unknown").to_string()
                };
                let short_id = if chat_id.len() > 11 { &chat_id[..11] } else { &chat_id };
                let model = extract_value(&msg, "msg_model=Some(\"", "\")")
                    .or_else(|| extract_value(&msg, "msg_model=", ","))
                    .unwrap_or("default");
                let provider = if !visitor.provider.is_empty() {
                    visitor.provider.clone()
                } else {
                    extract_value(&msg, "msg_provider=Some(\"", "\")")
                        .or_else(|| extract_value(&msg, "msg_provider=", ""))
                        .unwrap_or("auto")
                        .to_string()
                };

                write!(
                    writer,
                    "  \x1b[1;32m-->\x1b[0m \x1b[1;36mWS_MSG\x1b[0m chat_id=\x1b[35m{}\x1b[0m model=\x1b[1;33m{}\x1b[0m \x1b[1;30m({})\x1b[0m\n",
                    short_id, model, provider
                )?;
            } else {
                write!(writer, "  \x1b[1;30m...\x1b[0m \x1b[1;36mWS\x1b[0m  {}\n", msg)?;
            }
        } else if target.starts_with("openz::agent::agent_loop::run") {
            if msg.starts_with("Sending completion request to LLM") {
                let model = extract_value(&msg, "(model: ", ")").unwrap_or("unknown");
                let iteration = extract_value(&msg, "iteration=", "").unwrap_or("0");
                write!(
                    writer,
                    "  \x1b[1;33m⚡\x1b[0m \x1b[1;35mLLM_REQ\x1b[0m model=\x1b[1;33m{}\x1b[0m \x1b[1;30m[iter: {}]\x1b[0m\n",
                    model, iteration
                )?;
            } else if msg.starts_with("Received LLM response") {
                let reason = extract_value(&msg, "(finish_reason: ", ")").unwrap_or("stop");
                let duration = visitor.duration_ms.unwrap_or(0);
                write!(
                    writer,
                    "  \x1b[1;32m⚡\x1b[0m \x1b[1;36mLLM_RESP\x1b[0m status=\x1b[1;32m{}\x1b[0m \x1b[1;30m({}ms)\x1b[0m\n",
                    reason, duration
                )?;
            } else if msg.starts_with("Executing tool call") {
                let tool = if !visitor.tool.is_empty() {
                    visitor.tool.clone()
                } else {
                    extract_value(&msg, "tool=", " ").unwrap_or("unknown").to_string()
                };
                write!(
                    writer,
                    "  \x1b[1;33m⚙️\x1b[0m \x1b[1;33mTOOL_CALL\x1b[0m tool=\x1b[1;34m{}\x1b[0m\n",
                    tool
                )?;
            } else {
                write!(writer, "  \x1b[1;30m...\x1b[0m \x1b[1;33mLOOP\x1b[0m {}\n", msg)?;
            }
        } else if target.starts_with("openz::agent::agent_loop::tool_execution") {
            if msg.starts_with("Tool call completed") {
                let tool = if !visitor.tool.is_empty() {
                    visitor.tool.clone()
                } else {
                    extract_value(&msg, "tool=", " ").unwrap_or("unknown").to_string()
                };
                let status = if !visitor.status.is_empty() {
                    visitor.status.clone()
                } else {
                    extract_value(&msg, "status=\"", "\"").unwrap_or("success").to_string()
                };
                let status_colored = if status == "success" {
                    "\x1b[1;32mSUCCESS\x1b[0m"
                } else {
                    "\x1b[1;31mFAILED\x1b[0m"
                };
                write!(
                    writer,
                    "  \x1b[1;32m✓\x1b[0m \x1b[1;32mTOOL_RES\x1b[0m tool=\x1b[1;34m{}\x1b[0m status={}\n",
                    tool, status_colored
                )?;
            } else {
                write!(writer, "  \x1b[1;30m...\x1b[0m \x1b[1;33mTOOL\x1b[0m {}\n", msg)?;
            }
        } else if target.starts_with("openz::agent::agent_loop::restore") {
            if msg.starts_with("Restored session history") {
                let count = extract_value(&msg, "Restored session history (", " messages)").unwrap_or("0");
                let prompt = extract_value(&msg, "User prompt: \"", "\"").unwrap_or("");
                write!(
                    writer,
                    "  \x1b[1;34m◇\x1b[0m \x1b[1;32mRESTORED\x1b[0m ({} messages) \x1b[1;30mPrompt:\x1b[0m \"{}\"\n",
                    count, prompt
                )?;
            } else {
                write!(writer, "  \x1b[1;30m...\x1b[0m \x1b[1;32mREST\x1b[0m {}\n", msg)?;
            }
        } else if target.starts_with("openz::agent::agent_loop::save") {
            if msg.starts_with("Session saved successfully") {
                write!(
                    writer,
                    "  \x1b[1;32m✓\x1b[0m \x1b[1;32mTURN_COMPLETE\x1b[0m\n"
                )?;
            } else {
                write!(writer, "  \x1b[1;30m...\x1b[0m \x1b[1;35mSAVE\x1b[0m {}\n", msg)?;
            }
        } else if target.starts_with("openz::providers::resolver") && msg.contains("No API key configured") {
            let provider = extract_value(&msg, "provider '", "'").unwrap_or("unknown");
            write!(
                writer,
                "  \x1b[1;33m⚠️\x1b[0m \x1b[1;31mNO_KEY\x1b[0m provider=\x1b[1;31m{}\x1b[0m\n",
                provider
            )?;
        } else if target.starts_with("openz::cron") || target.starts_with("openz::config") {
            // Silence startup setup warning logs to keep output clean and minimalist
        } else {
            write!(writer, "  {} {}\n", level_str, msg)?;
        }
        Ok(())
    }
}
