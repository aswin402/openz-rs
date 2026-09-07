use crate::agent::style::*;
use crate::agent::AgentLoop;
use crate::config::schema::AgentDefaults;

pub mod commands;
pub mod device;
pub mod input;
pub mod mcp;
pub mod render;

// Re-export mcp progress bar functions/states
pub use mcp::{
    increment_mcp_failed, increment_mcp_loaded, init_mcp_progress, queue_notification,
    send_notification, set_mcp_done, set_mcp_status,
};

// Re-export render custom limit
pub use render::CUSTOM_CONTEXT_LIMIT;

// Re-export device command handler
pub use device::handle_device_command;

#[allow(unused_macros)]
macro_rules! println {
    () => {
        crate::tui_println!()
    };
    ($($arg:tt)*) => {
        crate::tui_println!($($arg)*)
    };
}

#[allow(unused_macros)]
macro_rules! eprintln {
    () => {
        crate::tui_eprintln!()
    };
    ($($arg:tt)*) => {
        crate::tui_eprintln!($($arg)*)
    };
}

pub struct RawModeGuard;

impl RawModeGuard {
    pub fn new() -> anyhow::Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(RawModeGuard)
    }
}

pub fn is_raw_input_active() -> bool {
    input::IS_RAW_INPUT_ACTIVE.load(std::sync::atomic::Ordering::SeqCst)
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableBracketedPaste);
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub struct CliChannel {
    agent_loop: tokio::sync::Mutex<AgentLoop>,
    defaults: tokio::sync::Mutex<AgentDefaults>,
}

impl CliChannel {
    pub fn new(agent_loop: AgentLoop, defaults: AgentDefaults) -> Self {
        static PANIC_HOOK: std::sync::Once = std::sync::Once::new();
        PANIC_HOOK.call_once(|| {
            let default_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic_info| {
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    crossterm::terminal::LeaveAlternateScreen
                );
                default_hook(panic_info);
            }));
        });

        if let Ok(mut guard) = CUSTOM_CONTEXT_LIMIT.lock() {
            *guard = defaults.context_limit;
        }
        CliChannel {
            agent_loop: tokio::sync::Mutex::new(agent_loop),
            defaults: tokio::sync::Mutex::new(defaults),
        }
    }
}

#[async_trait::async_trait]
impl crate::channels::Channel for CliChannel {
    fn name(&self) -> &'static str {
        "cli"
    }

    async fn start(&self) -> anyhow::Result<()> {
        crate::agent::style::spinner::IS_SILENT
            .scope(false, async move { self.start_inner().await })
            .await
    }
}

struct CliActiveGuard;
impl Drop for CliActiveGuard {
    fn drop(&mut self) {
        crate::shutdown::set_cli_active(false);
    }
}

impl CliChannel {
    async fn start_inner(&self) -> anyhow::Result<()> {
        // Derive a unique session key per workspace directory so multiple
        // `openz agent` instances can run in different directories.
        let session_key = crate::config::loader::get_cli_session_key();
        let workspace = crate::config::loader::active_workspace_or_current_dir();
        crate::shutdown::set_cli_active(true);
        let _guard = CliActiveGuard;

        let white = "\x1b[38;2;240;240;240m";
        let slate = "\x1b[38;2;107;122;153m";

        println!(
            "{}     ██████╗ ██████╗ ███████╗███╗   ██╗{}███████╗",
            white, RED_ORANGE
        );
        println!(
            "{}    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║{}╚══███╔╝",
            white, RED_ORANGE
        );
        println!(
            "{}    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║{}  ███╔╝",
            white, RED_ORANGE
        );
        println!(
            "{}    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║{} ███╔╝",
            white, RED_ORANGE
        );
        println!(
            "{}    ╚██████╔╝██║     ███████╗██║ ╚████║{}███████╗",
            white, RED_ORANGE
        );
        println!(
            "{}     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝{}╚══════╝\r",
            white, RED_ORANGE
        );

        println!(
            "{}openz v{}{}",
            COLOR_BOLD,
            env!("CARGO_PKG_VERSION"),
            COLOR_RESET
        );
        {
            let defaults = self.defaults.lock().await;
            println!(
                "{}{}{}",
                slate,
                format!("{} | {}", defaults.provider, defaults.model),
                COLOR_RESET
            );
        }

        if let Ok(current_dir) = std::env::current_dir() {
            let path_str = if let Some(home) = dirs::home_dir() {
                if current_dir == home {
                    "~".to_string()
                } else if let Ok(stripped) = current_dir.strip_prefix(&home) {
                    format!("~/{}", stripped.display())
                } else {
                    current_dir.display().to_string()
                }
            } else {
                current_dir.display().to_string()
            };
            println!("{}{}{}", slate, path_str, COLOR_RESET);
        }

        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );

        let session_manager = {
            let agent_loop = self.agent_loop.lock().await;
            agent_loop.session_manager.clone()
        };
        if let Ok(session) = session_manager.load(&session_key) {
            render::print_session_history(&session);
        }

        loop {
            let (model, provider, session_manager) = {
                let defaults = self.defaults.lock().await;
                let agent_loop = self.agent_loop.lock().await;
                (
                    defaults.model.clone(),
                    defaults.provider.clone(),
                    agent_loop.session_manager.clone(),
                )
            };

            let (input, remote_sender) =
                match input::read_line_raw(&model, &provider, &session_manager, &session_key) {
                    Ok(inp) => inp,
                    Err(e) => {
                        eprintln!("Error reading input: {}", e);
                        continue;
                    }
                };
            let trimmed = input.trim();

            if trimmed.is_empty() {
                continue;
            }

            let prompt_to_run = match commands::handle_slash_command(
                trimmed,
                &self.agent_loop,
                &self.defaults,
                &session_key,
                &workspace,
            )
            .await
            {
                commands::SlashCommandOutcome::Exit => break,
                commands::SlashCommandOutcome::Handled => continue,
                commands::SlashCommandOutcome::ExecuteTurn(query) => query,
                commands::SlashCommandOutcome::Unhandled => trimmed.to_string(),
            };

            let runner = self.agent_loop.lock().await;

            let run_res = {
                let prompt_clone = prompt_to_run.clone();
                let session_key_clone = session_key.clone();
                let run_fut: std::pin::Pin<
                    Box<
                        dyn std::future::Future<Output = anyhow::Result<crate::agent::RunResult>>
                            + Send,
                    >,
                > = Box::pin(runner.run(&prompt_clone, &session_key_clone));

                let tx = crate::shutdown::cli_cancel_tx();
                let mut rx = tx.subscribe();
                let initial_val = *rx.borrow();
                let cancel_fut = async move {
                    while *rx.borrow() == initial_val {
                        if rx.changed().await.is_err() {
                            break;
                        }
                    }
                };

                // Keep raw mode active while a turn is running so Esc/Ctrl+C are delivered
                // as key events to the cancellation watcher instead of being line-buffered.
                let _run_raw_guard = RawModeGuard::new().ok();

                // Use a dedicated OS thread (not a tokio task!) for keyboard polling.
                // crossterm::event::poll/read are blocking calls that would starve
                // the tokio runtime if run inside tokio::task::spawn.
                let keyboard_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let keyboard_done_for_thread = keyboard_done.clone();
                let keyboard_thread = std::thread::spawn(move || {
                    loop {
                        // Check if the main task told us to stop. The main thread only waits
                        // briefly, so cancellation never hangs on terminal polling.
                        if keyboard_done_for_thread.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }
                        if let Ok(true) =
                            crossterm::event::poll(std::time::Duration::from_millis(25))
                        {
                            if keyboard_done_for_thread.load(std::sync::atomic::Ordering::SeqCst) {
                                break;
                            }
                            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read()
                            {
                                if input::is_turn_cancel_key(&key) {
                                    crate::shutdown::trigger_cli_cancel();
                                }
                            }
                        }
                    }
                });

                tokio::pin!(run_fut);
                tokio::pin!(cancel_fut);

                let (heartbeat_stop, heartbeat_task) = if let Some(chat_id) = remote_sender
                    .as_deref()
                    .and_then(|sender| sender.strip_prefix("telegram:"))
                    .and_then(|id| id.parse::<i64>().ok())
                {
                    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
                    let task = tokio::spawn(async move {
                        loop {
                            tokio::select! {
                                _ = &mut stop_rx => break,
                                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                                    crate::channels::telegram::refresh_remote_typing_indicator(chat_id);
                                }
                            }
                        }
                    });
                    (Some(stop_tx), Some(task))
                } else {
                    (None, None)
                };

                let result = tokio::select! {
                    biased;
                    _ = &mut cancel_fut => {
                        crate::shutdown::trigger_cli_cancel();
                        crate::tui_println!("\r\n{}▲ Execution cancelled by user (Ctrl+C / Esc).{}", AURA_GOLD, COLOR_RESET);
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            &mut run_fut,
                        )
                        .await;
                        None
                    }
                    res = &mut run_fut => {
                        Some(res)
                    }
                };

                if let Some(stop_tx) = heartbeat_stop {
                    let _ = stop_tx.send(());
                }
                if let Some(task) = heartbeat_task {
                    let _ = task.await;
                }

                // Signal the keyboard thread to stop. Give it a very short cleanup window so it
                // does not keep consuming keys from the next input prompt or approval menu.
                keyboard_done.store(true, std::sync::atomic::Ordering::SeqCst);
                for _ in 0..10 {
                    if keyboard_thread.is_finished() {
                        let _ = keyboard_thread.join();
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }

                result
            };

            if let Some(ref sender) = remote_sender {
                if sender.starts_with("telegram:") {
                    if let Some(chat_id_str) = sender.strip_prefix("telegram:") {
                        if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                            crate::channels::telegram::stop_typing_indicator(chat_id);
                        }
                    }
                }
            }

            match run_res {
                Some(Ok(res)) => {
                    if !res.streamed {
                        println!();
                        render::print_colored_markdown(&res.content);
                    }
                    println!();
                    println!(
                        "{}────────────────────────────────────────────────────────────{}",
                        LIGHT_WHITE, COLOR_RESET
                    );

                    if let Some(ref sender) = remote_sender {
                        if sender.starts_with("telegram:") {
                            if let Some(chat_id_str) = sender.strip_prefix("telegram:") {
                                if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                                    let _ = crate::channels::telegram::send_text_message(
                                        chat_id,
                                        &format!("🔌 [Remote Control Output]\n{}", res.content),
                                    )
                                    .await;
                                }
                            }
                        }
                    }
                }
                Some(Err(e)) => {
                    eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET);
                    println!(
                        "{}────────────────────────────────────────────────────────────{}",
                        LIGHT_WHITE, COLOR_RESET
                    );
                    if let Some(ref sender) = remote_sender {
                        if let Some(chat_id_str) = sender.strip_prefix("telegram:") {
                            if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                                let _ = crate::channels::telegram::send_text_message(
                                    chat_id,
                                    &format!("🔌 [Remote Control Error]\n{}", e),
                                )
                                .await;
                            }
                        }
                    }
                }
                None => {
                    println!(
                        "\r\n{}✕ Conversation interrupted by user.{}",
                        ERROR_RED, COLOR_RESET
                    );
                    println!(
                        "{}────────────────────────────────────────────────────────────{}",
                        LIGHT_WHITE, COLOR_RESET
                    );
                    if let Some(ref sender) = remote_sender {
                        if let Some(chat_id_str) = sender.strip_prefix("telegram:") {
                            if let Ok(chat_id) = chat_id_str.parse::<i64>() {
                                let _ = crate::channels::telegram::send_text_message(
                                    chat_id,
                                    "🔌 [Remote Control Cancelled]\nThe remote request was interrupted.",
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
