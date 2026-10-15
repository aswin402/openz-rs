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

// Re-export render custom limit and streaming renderer
pub use render::{StreamingMarkdownRenderer, CUSTOM_CONTEXT_LIMIT};

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

        match CUSTOM_CONTEXT_LIMIT.lock() {
            Ok(mut guard) => {
                *guard = defaults.context_limit;
            }
            Err(_) => {}
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

fn format_current_dir_display(current_dir: &std::path::Path) -> String {
    match dirs::home_dir() {
        Some(home) => match current_dir == home {
            true => "~".to_string(),
            false => match current_dir.strip_prefix(&home) {
                Ok(stripped) => format!("~/{}", stripped.display()),
                Err(_) => current_dir.display().to_string(),
            },
        },
        None => current_dir.display().to_string(),
    }
}

fn parse_telegram_chat_id(remote_sender: Option<&str>) -> Option<i64> {
    remote_sender
        .and_then(|sender| sender.strip_prefix("telegram:"))
        .and_then(|id| id.parse::<i64>().ok())
}

impl CliChannel {
    async fn start_inner(&self) -> anyhow::Result<()> {
        // Derive a unique session key per workspace directory so multiple
        // `openz agent` instances can run in different directories.
        let session_key = crate::config::loader::get_cli_session_key();
        let workspace = crate::config::loader::active_workspace_or_current_dir();
        crate::shutdown::set_cli_active(true);
        let _guard = CliActiveGuard;

        for (white_part, orange_part) in crate::agent::style::markdown::OPENZ_BANNER_PARTS {
            println!(
                "    {}{}{}{}{}",
                BRAND_WHITE, white_part, RED_ORANGE, orange_part, COLOR_RESET
            );
        }

        println!(
            "{}openz v{}{}",
            COLOR_BOLD,
            env!("CARGO_PKG_VERSION"),
            COLOR_RESET
        );
        {
            let defaults = self.defaults.lock().await;
            println!(
                "{}{} | {}{}",
                AURA_SLATE, defaults.provider, defaults.model, COLOR_RESET
            );
        }

        match std::env::current_dir() {
            Ok(current_dir) => {
                let path_str = format_current_dir_display(&current_dir);
                println!("{}{}{}", AURA_SLATE, path_str, COLOR_RESET);
            }
            Err(_) => {}
        }

        println!(
            "{}────────────────────────────────────────────────────────────{}",
            LIGHT_WHITE, COLOR_RESET
        );

        let session_manager = {
            let agent_loop = self.agent_loop.lock().await;
            agent_loop.session_manager.clone()
        };
        match session_manager.load(&session_key) {
            Ok(session) => render::print_session_history(&session),
            Err(_) => {}
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

            match trimmed.is_empty() {
                true => continue,
                false => {}
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
            let telegram_chat_id = parse_telegram_chat_id(remote_sender.as_deref());

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
                        match rx.changed().await.is_err() {
                            true => break,
                            false => {}
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
                let keyboard_thread = std::thread::spawn(move || loop {
                    match keyboard_done_for_thread.load(std::sync::atomic::Ordering::SeqCst) {
                        true => break,
                        false => {}
                    }
                    match crossterm::event::poll(std::time::Duration::from_millis(25)) {
                        Ok(true) => {
                            match keyboard_done_for_thread.load(std::sync::atomic::Ordering::SeqCst)
                            {
                                true => break,
                                false => {}
                            }
                            match crossterm::event::read() {
                                Ok(crossterm::event::Event::Key(key)) => {
                                    match input::is_turn_cancel_key(&key) {
                                        true => crate::shutdown::trigger_cli_cancel(),
                                        false => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                });

                tokio::pin!(run_fut);
                tokio::pin!(cancel_fut);

                let (heartbeat_stop, heartbeat_task) = match telegram_chat_id {
                    Some(chat_id) => {
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
                    }
                    None => (None, None),
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

                match heartbeat_stop {
                    Some(stop_tx) => {
                        let _ = stop_tx.send(());
                    }
                    None => {}
                }
                match heartbeat_task {
                    Some(task) => {
                        let _ = task.await;
                    }
                    None => {}
                }

                // Signal the keyboard thread to stop. Give it a very short cleanup window so it
                // does not keep consuming keys from the next input prompt or approval menu.
                keyboard_done.store(true, std::sync::atomic::Ordering::SeqCst);
                for _ in 0..10 {
                    match keyboard_thread.is_finished() {
                        true => {
                            let _ = keyboard_thread.join();
                            break;
                        }
                        false => std::thread::sleep(std::time::Duration::from_millis(20)),
                    }
                }

                result
            };

            match telegram_chat_id {
                Some(chat_id) => crate::channels::telegram::stop_typing_indicator(chat_id),
                None => {}
            }

            match run_res {
                Some(Ok(res)) => {
                    match res.streamed {
                        true => {}
                        false => {
                            println!();
                            render::print_colored_markdown(&res.content);
                        }
                    }
                    println!();
                    println!(
                        "{}────────────────────────────────────────────────────────────{}",
                        LIGHT_WHITE, COLOR_RESET
                    );

                    match telegram_chat_id {
                        Some(chat_id) => {
                            let _ = crate::channels::telegram::send_text_message(
                                chat_id,
                                &format!("🔌 [Remote Control Output]\n{}", res.content),
                            )
                            .await;
                        }
                        None => {}
                    }
                }
                Some(Err(e)) => {
                    eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET);
                    println!(
                        "{}────────────────────────────────────────────────────────────{}",
                        LIGHT_WHITE, COLOR_RESET
                    );
                    match telegram_chat_id {
                        Some(chat_id) => {
                            let _ = crate::channels::telegram::send_text_message(
                                chat_id,
                                &format!("🔌 [Remote Control Error]\n{}", e),
                            )
                            .await;
                        }
                        None => {}
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
                    match telegram_chat_id {
                        Some(chat_id) => {
                            let _ = crate::channels::telegram::send_text_message(
                                chat_id,
                                "🔌 [Remote Control Cancelled]\nThe remote request was interrupted.",
                            )
                            .await;
                        }
                        None => {}
                    }
                }
            }
        }

        Ok(())
    }
}
