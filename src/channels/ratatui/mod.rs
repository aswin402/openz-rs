pub mod app;
pub mod commands;
pub mod markdown;
pub mod modals;
pub mod session;
pub mod theme;
pub mod timeline;
pub mod ui;

pub use session::*;

use anyhow::Result;
pub use app::IS_RATATUI_ACTIVE;
use app::{ChatMessage, ModalState, RatatuiApp};
use crate::channels::Channel;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub enum TurnEvent {
    SyncSession(Vec<ChatMessage>),
    SingleMessage(ChatMessage),
    Error(String),
}

pub struct SecurityApprovalRequest {
    pub tool_name: String,
    pub description: String,
    pub session_key: String,
    pub tx: tokio::sync::oneshot::Sender<bool>,
}

pub static SECURITY_APPROVAL_CHANNEL: std::sync::LazyLock<(
    tokio::sync::mpsc::UnboundedSender<SecurityApprovalRequest>,
    tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<SecurityApprovalRequest>>,
)> = std::sync::LazyLock::new(|| {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    (tx, tokio::sync::Mutex::new(rx))
});

pub async fn request_ratatui_security_approval(
    tool_name: &str,
    description: &str,
    session_key: &str,
) -> anyhow::Result<bool> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let req = SecurityApprovalRequest {
        tool_name: tool_name.to_string(),
        description: description.to_string(),
        session_key: session_key.to_string(),
        tx,
    };
    if SECURITY_APPROVAL_CHANNEL.0.send(req).is_err() {
        return Ok(false);
    }
    match rx.await {
        Ok(approved) => Ok(approved),
        Err(_) => Ok(false),
    }
}

fn spawn_agent_turn(
    prompt: String,
    agent_loop: Arc<tokio::sync::Mutex<crate::agent::AgentLoop>>,
    session_key: String,
    session_manager: Arc<crate::session::SessionManager>,
    workspace: std::path::PathBuf,
    turn_tx: tokio::sync::mpsc::UnboundedSender<TurnEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let loop_guard = agent_loop.lock().await;
        let run_result = crate::config::loader::ACTIVE_WORKSPACE
            .scope(workspace, async {
                loop_guard.run(&prompt, &session_key).await
            })
            .await;
        match run_result {
            Ok(_res) => {
                if let Ok(session) = session_manager.load(&session_key) {
                    let mut msgs = Vec::new();
                    for m in session.messages {
                        msgs.push(ChatMessage::from_session_message(&m));
                    }
                    let _ = turn_tx.send(TurnEvent::SyncSession(msgs));
                } else {
                    let _ = turn_tx.send(TurnEvent::SingleMessage(
                        ChatMessage::simple("assistant", _res.content),
                    ));
                }
            }
            Err(err) => {
                let _ = turn_tx.send(TurnEvent::Error(err.to_string()));
            }
        }
    })
}

// ── Scroll / render tuning ──────────────────────────────────────────────────
const TICK_MS: u64 = 50;
/// Mouse wheel and modifier-arrow scroll amount
const SCROLL_STEP: u32 = 3;
/// Plain Up/Down when input is empty or scrolled up
const SCROLL_JUMP: u32 = 3;
/// Ctrl+P / Ctrl+N line scroll
const SCROLL_STEP_EM: u32 = 4;
/// PageUp / PageDown
const SCROLL_PAGE: u32 = 8;
/// Ctrl+U / Ctrl+D half-page jump
const SCROLL_HALF: u32 = 10;

struct RatatuiGuard;

impl Drop for RatatuiGuard {
    fn drop(&mut self) {
        let _ = stdout().execute(crossterm::event::DisableMouseCapture);
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
        let _ = stdout().execute(crossterm::cursor::Show);
        IS_RATATUI_ACTIVE.store(false, Ordering::SeqCst);
    }
}

pub async fn handle_ratatui_tui() -> Result<()> {
    let config = crate::config::loader::load_config().unwrap_or_default();
    let mut model = config.agents.defaults.model.clone();
    let mut provider = config.agents.defaults.provider.clone();
    let base_session_key = crate::config::loader::get_cli_session_key();
    // Active session key is mutable: /history restore and /new-session swap it.
    let session_key = Arc::new(tokio::sync::RwLock::new(base_session_key.clone()));
    let workspace = crate::config::loader::active_workspace_or_current_dir();

    let sessions_dir = crate::config::loader::sessions_dir();
    let session_manager = Arc::new(crate::session::SessionManager::new(sessions_dir));

    // Build AgentLoop instance wrapped in a Mutex so it can be updated dynamically
    let agent_loop = Arc::new(tokio::sync::Mutex::new(
        crate::cli::builder::build_agent_loop(config.clone()).await?,
    ));

    // Start cron scheduler
    crate::cron::scheduler::start_scheduler(config.clone());

    // Interactive Session History Menu on startup if history exists
    let history = crate::cli::load_session_history()?;
    if history.is_empty() {
        crate::cli::archive_current_session(&session_manager, &base_session_key).await?;
    } else {
        let selected = match crate::agent::style::select_menu_with_history(
            "Welcome to OpenZ! Select an option:",
            &history,
        ) {
            Ok(s) => s,
            Err(_) => {
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
                return Ok(());
            }
        };
        if selected == 0 {
            reset_active_session(&session_key, &session_manager, &base_session_key).await;
        } else {
            // Adopt the chosen session in place — no copy, so /history shows no duplicates
            reset_active_session(&session_key, &session_manager, &base_session_key).await;
            *session_key.write().await = history[selected - 1].key.clone();
        }
    }

    // Register active TUI session for activity and background heartbeat
    let defaults = config.agents.defaults.clone();
    let tui_started_at = chrono::Utc::now().to_rfc3339();
    let tui_cwd = std::env::current_dir().unwrap_or_default();
    let initial_preview = session_manager
        .load(&base_session_key)
        .ok()
        .map(|session| crate::agent::activity::session_preview_from_messages(&session.messages))
        .unwrap_or_else(|| "No user prompt yet".to_string());
    let active_tui = crate::agent::activity::make_active_tui_session(
        &base_session_key,
        &tui_cwd,
        &tui_started_at,
        &defaults.model,
        &defaults.provider,
        &initial_preview,
    );
    let _ = crate::agent::activity::upsert_active_tui_session(&active_tui);

    let heartbeat_session_key = session_key.clone();
    let heartbeat_cwd = tui_cwd.clone();
    let heartbeat_started_at = tui_started_at.clone();
    let heartbeat_model = defaults.model.clone();
    let heartbeat_provider = defaults.provider.clone();
    let heartbeat_session_manager = session_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            let current_key = heartbeat_session_key.read().await.clone();
            let preview = heartbeat_session_manager
                .load(&current_key)
                .ok()
                .map(|session| {
                    crate::agent::activity::session_preview_from_messages(&session.messages)
                })
                .unwrap_or_else(|| "No user prompt yet".to_string());
            let active_tui = crate::agent::activity::make_active_tui_session(
                &current_key,
                &heartbeat_cwd,
                &heartbeat_started_at,
                &heartbeat_model,
                &heartbeat_provider,
                &preview,
            );
            let _ = crate::agent::activity::upsert_active_tui_session(&active_tui);
        }
    });

    // Mark silent mode for background channels via thread-safe AtomicBool
    crate::cli::set_silent_mode(true);

    // Auto-start WebSocket gateway in the background if enabled and configured to start on TUI
    if let Some(ws_config) = &config.channels.websocket {
        if ws_config.enabled && ws_config.start_on_tui {
            let config_clone = config.clone();
            let ws_config_clone = ws_config.clone();
            tokio::spawn(async move {
                if let Ok(agent_loop) = crate::cli::builder::build_agent_loop(config_clone).await {
                    let gateway = crate::channels::WsGateway::new(ws_config_clone, agent_loop);
                    let _ = gateway.start().await;
                }
            });
        }
    }

    // Auto-start Telegram channel in the background if enabled
    if let Some(tg_config) = &config.channels.telegram {
        if tg_config.enabled {
            let token = if tg_config.bot_token.is_empty() {
                std::env::var("TELEGRAM_BOT_TOKEN").ok()
            } else {
                Some(tg_config.bot_token.clone())
            };
            if let Some(token) = token {
                let config_clone = config.clone();
                tokio::spawn(async move {
                    if let Ok(agent_loop) = crate::cli::builder::build_agent_loop(config_clone).await {
                        let channel = crate::channels::TelegramChannel::new(token, agent_loop);
                        let _ = channel.start().await;
                    }
                });
            }
        }
    }

    // Auto-start Discord channel in the background if enabled
    if let Some(dc_config) = &config.channels.discord {
        if dc_config.enabled {
            let token = if dc_config.bot_token.is_empty() {
                std::env::var("DISCORD_BOT_TOKEN").ok()
            } else {
                Some(dc_config.bot_token.clone())
            };
            if let Some(token) = token {
                let config_clone = config.clone();
                tokio::spawn(async move {
                    if let Ok(agent_loop) = crate::cli::builder::build_agent_loop(config_clone).await {
                        let channel = crate::channels::DiscordChannel::new(token, agent_loop);
                        let _ = channel.start().await;
                    }
                });
            }
        }
    }

    // Auto-start WhatsApp channel in the background if enabled
    if let Some(wa_config) = &config.channels.whatsapp {
        if wa_config.enabled {
            let config_clone = config.clone();
            let wa_config_clone = wa_config.clone();
            tokio::spawn(async move {
                if let Ok(agent_loop) = crate::cli::builder::build_agent_loop(config_clone).await {
                    let channel = crate::channels::WhatsAppChannel::new(
                        wa_config_clone.api_key,
                        wa_config_clone.phone_number_id,
                        agent_loop,
                    );
                    let _ = channel.start().await;
                }
            });
        }
    }

    // Auto-start Email channel in the background if enabled
    if let Some(email_config) = &config.channels.email {
        if email_config.enabled {
            let config_clone = config.clone();
            tokio::spawn(async move {
                if let Ok(agent_loop) = crate::cli::builder::build_agent_loop(config_clone).await {
                    let channel = crate::channels::EmailChannel::new(agent_loop);
                    let _ = channel.start().await;
                }
            });
        }
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(crossterm::event::EnableMouseCapture)?;
    stdout.execute(crossterm::cursor::Show)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    IS_RATATUI_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = RatatuiGuard;

    let mut session_config = config.clone();
    let active_key = session_key.read().await.clone();
    if let Ok(session) = session_manager.load(&active_key) {
        crate::agent::agent_loop::apply_session_overrides(&mut session_config, &session.metadata);
        model = session_config.agents.defaults.model.clone();
        provider = session_config.agents.defaults.provider.clone();
        if session_config.agents.defaults.model != config.agents.defaults.model
            || session_config.agents.defaults.provider != config.agents.defaults.provider
        {
            if let Ok(resolved) = crate::providers::resolver::resolve_provider_full(
                &session_config,
                &session_config.agents.defaults.model,
            ) {
                if let Ok(mut loop_lock) = agent_loop.try_lock() {
                    loop_lock.update_model_and_provider(session_config.clone(), resolved.instance);
                }
            }
        }
    }

    let mut app = RatatuiApp::new(model, provider, active_key.clone());
    let marker_dir = tui_marker_dir();
    let _ = write_tui_marker_in_dir(
        &marker_dir,
        std::process::id(),
        &active_key,
        &app.model,
        &app.provider,
    );

    // Load selected session history into Ratatui conversation stream
    if let Ok(session) = session_manager.load(&active_key) {
        for msg in session.messages {
            app.messages.push(ChatMessage::from_session_message(&msg));
        }
    }

    let (turn_tx, mut turn_rx) = tokio::sync::mpsc::unbounded_channel::<TurnEvent>();
    let (model_tx, mut model_rx) =
        tokio::sync::mpsc::unbounded_channel::<(String, String, Vec<String>)>();
    let mut current_turn_handle: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        // Drain any security approval requests
        if let Ok(mut rx_lock) = SECURITY_APPROVAL_CHANNEL.1.try_lock() {
            if let Ok(req) = rx_lock.try_recv() {
                app.modal = ModalState::SecurityApproval {
                    tool_name: req.tool_name,
                    description: req.description,
                    options: vec![
                        "Approve (Allow once)".to_string(),
                        "Approve & Trust for this session".to_string(),
                        "Deny (Abort tool)".to_string(),
                    ],
                    selected_idx: 0,
                    session_key: req.session_key,
                    tx: Arc::new(tokio::sync::Mutex::new(Some(req.tx))),
                };
            }
        }

        // Drain any async model fetch results
        while let Ok((prov_name, prov_display, fetched_models)) = model_rx.try_recv() {
            if matches!(&app.modal, ModalState::ModelSelect { provider_name, .. } if provider_name == &prov_name)
            {
                let count = fetched_models.len();
                app.modal = ModalState::ModelSelect {
                    provider_name: prov_name,
                    provider_display: prov_display,
                    models: fetched_models,
                    filtered_indices: (0..count).collect(),
                    selected_idx: 0,
                    filter: String::new(),
                    loading: false,
                };
            }
        }

        // Drain any incoming background responses from AgentLoop
        while let Ok(event) = turn_rx.try_recv() {
            app.is_thinking = false;
            app.work_start = None;
            current_turn_handle = None;
            match event {
                TurnEvent::SyncSession(msgs) => {
                    app.apply_sync_session(msgs);
                }
                TurnEvent::SingleMessage(msg) => {
                    app.messages.push(msg);
                }
                TurnEvent::Error(err) => {
                    app.messages
                        .push(ChatMessage::notice(format!("⚠ Error: {}", err)));
                }
            }
            app.scroll_to_bottom();

            // If turn completed and there are queued prompts, dispatch next queued prompt!
            if !app.is_thinking {
                if let Some(next_prompt) = app.pop_next_prompt() {
                    app.is_thinking = true;
                    app.work_start = Some(Instant::now());
                    app.scroll_to_bottom();
                    let turn_session_key = session_key.read().await.clone();
                    current_turn_handle = Some(spawn_agent_turn(
                        next_prompt,
                        agent_loop.clone(),
                        turn_session_key,
                        session_manager.clone(),
                        workspace.clone(),
                        turn_tx.clone(),
                    ));
                }
            }
        }

        // Tick spinner animation
        app.spinner_idx = app.spinner_idx.wrapping_add(1);

        terminal.draw(|f| ui::render_ratatui_ui(f, &mut app))?;

        if event::poll(Duration::from_millis(TICK_MS))? {
            match event::read()? {
                Event::Mouse(mouse) => {
                    if !app.modal.is_active() {
                        match mouse.kind {
                            MouseEventKind::ScrollUp => {
                                app.scroll_up(SCROLL_STEP);
                            }
                            MouseEventKind::ScrollDown => {
                                app.scroll_down(SCROLL_STEP);
                            }
                            _ => {}
                        }
                    }
                }
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        // Global interrupt: Ctrl+C
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('c')
                        {
                            if let ModalState::SecurityApproval { tx, .. } = &app.modal {
                                if let Ok(mut guard) = tx.try_lock() {
                                    if let Some(sender) = guard.take() {
                                        let _ = sender.send(false);
                                    }
                                }
                                app.modal = ModalState::None;
                                continue;
                            }
                            if app.modal.is_active() {
                                app.modal = ModalState::None;
                                continue;
                            }
                            if app.is_thinking {
                                if let Some(handle) = current_turn_handle.take() {
                                    handle.abort();
                                }
                                crate::shutdown::trigger_cli_cancel();
                                app.is_thinking = false;
                                app.work_start = None;
                                app.messages.push(ChatMessage::notice(
                                    "Turn cancelled by user.".to_string(),
                                ));
                                continue;
                            }
                            break;
                        }

                        // ── 1. Modal Key Interception ───────────────────────────
                        if app.modal.is_active() {
                            match &mut app.modal {
                                ModalState::ProviderSelect {
                                    providers,
                                    selected_idx,
                                } => match key.code {
                                    KeyCode::Up => {
                                        if *selected_idx > 0 {
                                            *selected_idx -= 1;
                                        } else {
                                            *selected_idx = providers.len().saturating_sub(1);
                                        }
                                    }
                                    KeyCode::Down => {
                                        if *selected_idx + 1 < providers.len() {
                                            *selected_idx += 1;
                                        } else {
                                            *selected_idx = 0;
                                        }
                                    }
                                    KeyCode::Esc => {
                                        app.modal = ModalState::None;
                                    }
                                    KeyCode::Enter => {
                                        let (prov_name, prov_display) =
                                            providers[*selected_idx].clone();

                                        app.modal = ModalState::ModelSelect {
                                            provider_name: prov_name.clone(),
                                            provider_display: prov_display.clone(),
                                            models: Vec::new(),
                                            filtered_indices: Vec::new(),
                                            selected_idx: 0,
                                            filter: String::new(),
                                            loading: true,
                                        };

                                        let fetch_tx = model_tx.clone();
                                        let fetch_prov = prov_name.clone();
                                        let fetch_display = prov_display.clone();

                                        tokio::spawn(async move {
                                            let config = crate::config::loader::load_config()
                                                .unwrap_or_default();
                                            let mut models = Vec::new();
                                            if let Some(api_models) =
                                                crate::channels::fetch_provider_models(
                                                    &fetch_prov,
                                                    &config,
                                                )
                                                .await
                                            {
                                                models = api_models;
                                            }
                                            let curated = app::curated_models_for(&fetch_prov);
                                            for m in curated {
                                                if !models.iter().any(|existing| {
                                                    existing.eq_ignore_ascii_case(&m)
                                                }) {
                                                    models.push(m);
                                                }
                                            }
                                            models = crate::channels::model_menu_options_with_prefs(
                                                &fetch_prov,
                                                models,
                                            );
                                            let _ =
                                                fetch_tx.send((fetch_prov, fetch_display, models));
                                        });
                                    }
                                    _ => {}
                                },
                                ModalState::ModelSelect {
                                    provider_name,
                                    models,
                                    filtered_indices,
                                    selected_idx,
                                    filter,
                                    loading,
                                    ..
                                } => {
                                    if *loading {
                                        if key.code == KeyCode::Esc {
                                            app.modal = ModalState::None;
                                        }
                                        continue;
                                    }

                                    match key.code {
                                        KeyCode::Up => {
                                            if *selected_idx > 0 {
                                                *selected_idx -= 1;
                                            } else {
                                                *selected_idx =
                                                    filtered_indices.len().saturating_sub(1);
                                            }
                                        }
                                        KeyCode::Down => {
                                            if *selected_idx + 1 < filtered_indices.len() {
                                                *selected_idx += 1;
                                            } else {
                                                *selected_idx = 0;
                                            }
                                        }
                                        KeyCode::Esc => {
                                            app.modal = ModalState::None;
                                        }
                                        KeyCode::Backspace => {
                                            filter.pop();
                                            app.modal.update_model_filter();
                                        }
                                        KeyCode::Char(c) => {
                                            filter.push(c);
                                            app.modal.update_model_filter();
                                        }
                                        KeyCode::Enter => {
                                            if !filtered_indices.is_empty() {
                                                let real_idx = filtered_indices[*selected_idx];
                                                let chosen_model =
                                                    crate::channels::model_menu_model_name(
                                                        &models[real_idx],
                                                    )
                                                    .to_string();
                                                let prov = provider_name.clone();

                                                match apply_session_model_selection(
                                                    &agent_loop,
                                                    &session_manager,
                                                    &session_key.read().await.clone(),
                                                    &marker_dir,
                                                    &prov,
                                                    &chosen_model,
                                                )
                                                .await
                                                {
                                                    Ok(()) => {
                                                        app.model = chosen_model.clone();
                                                        app.provider = prov.clone();
                                                        app.messages.push(ChatMessage::notice(
                                                            format!(
                                                            "✓ Switched this session to: {} ({})",
                                                            chosen_model, prov
                                                        ),
                                                        ));
                                                    }
                                                    Err(e) => {
                                                        app.messages.push(ChatMessage::notice(
                                                            format!(
                                                                "⚠ Failed to switch model: {}",
                                                                e
                                                            ),
                                                        ));
                                                    }
                                                }
                                            }
                                            app.modal = ModalState::None;
                                        }
                                        _ => {}
                                    }
                                }
                                ModalState::Help => match key.code {
                                    KeyCode::Esc | KeyCode::Enter => {
                                        app.modal = ModalState::None;
                                    }
                                    _ => {}
                                },
                                ModalState::History {
                                    sessions,
                                    selected_idx,
                                } => match key.code {
                                    KeyCode::Up => {
                                        if *selected_idx > 0 {
                                            *selected_idx -= 1;
                                        } else {
                                            *selected_idx = sessions.len().saturating_sub(1);
                                        }
                                    }
                                    KeyCode::Down => {
                                        if *selected_idx + 1 < sessions.len() {
                                            *selected_idx += 1;
                                        } else {
                                            *selected_idx = 0;
                                        }
                                    }
                                    KeyCode::Esc => {
                                        app.modal = ModalState::None;
                                    }
                                    KeyCode::Enter => {
                                        if app.is_thinking {
                                            app.messages.push(ChatMessage::notice(
                                                "⏳ Cannot switch sessions while a turn is running — Ctrl+C to cancel first.".to_string(),
                                            ));
                                            app.modal = ModalState::None;
                                            continue;
                                        }
                                        if !sessions.is_empty() {
                                            let (target_key, title, _) =
                                                sessions[*selected_idx].clone();
                                            if let Ok(loaded) = session_manager.load(&target_key) {
                                                // Archive whatever we're leaving, then adopt the
                                                // target key so future prompts append to it
                                                reset_active_session(
                                                    &session_key,
                                                    &session_manager,
                                                    &base_session_key,
                                                )
                                                .await;
                                                *session_key.write().await = target_key.clone();

                                                app.messages.clear();
                                                app.session_key = target_key.clone();
                                                for msg in loaded.messages {
                                                    app.messages.push(
                                                        ChatMessage::from_session_message(&msg),
                                                    );
                                                }
                                                app.scroll_to_bottom();
                                                app.messages.push(ChatMessage::notice(format!(
                                                    "✓ Restored session: {}",
                                                    title
                                                )));
                                                let _ = write_tui_marker_in_dir(
                                                    &marker_dir,
                                                    std::process::id(),
                                                    &target_key,
                                                    &app.model,
                                                    &app.provider,
                                                );
                                            }
                                        }
                                        app.modal = ModalState::None;
                                    }
                                    _ => {}
                                },
                                ModalState::SecurityApproval {
                                    tool_name,
                                    options,
                                    selected_idx,
                                    session_key: approval_session,
                                    tx,
                                    ..
                                } => match key.code {
                                    KeyCode::Up => {
                                        if *selected_idx > 0 {
                                            *selected_idx -= 1;
                                        } else {
                                            *selected_idx = options.len().saturating_sub(1);
                                        }
                                    }
                                    KeyCode::Down => {
                                        if *selected_idx + 1 < options.len() {
                                            *selected_idx += 1;
                                        } else {
                                            *selected_idx = 0;
                                        }
                                    }
                                    KeyCode::Esc => {
                                        if let Ok(mut guard) = tx.try_lock() {
                                            if let Some(sender) = guard.take() {
                                                let _ = sender.send(false);
                                            }
                                        }
                                        app.modal = ModalState::None;
                                    }
                                    KeyCode::Enter => {
                                        let approved = *selected_idx == 0 || *selected_idx == 1;
                                        if *selected_idx == 1 {
                                            crate::agent::security::trust_tool_for_session(
                                                approval_session,
                                                tool_name,
                                            );
                                        }
                                        if let Ok(mut guard) = tx.try_lock() {
                                            if let Some(sender) = guard.take() {
                                                let _ = sender.send(approved);
                                            }
                                        }
                                        app.modal = ModalState::None;
                                    }
                                    _ => {}
                                },
                                _ => {}
                            }
                            continue;
                        }

                        // ── 2. Standard View & Input Key Events ─────────────────
                        let matches = app.matching_slash_commands();
                        let has_matches = !matches.is_empty();

                        match key.code {
                            KeyCode::Esc => {
                                if app.is_thinking {
                                    if let Some(handle) = current_turn_handle.take() {
                                        handle.abort();
                                    }
                                    crate::shutdown::trigger_cli_cancel();
                                    app.is_thinking = false;
                                    app.work_start = None;
                                    app.messages.push(ChatMessage::notice(
                                        "Turn interrupted by user.".to_string(),
                                    ));
                                } else {
                                    app.typed_input.clear();
                                    app.cursor_idx = 0;
                                    app.selected_index = None;
                                    app.history_idx = None;
                                }
                            }
                            KeyCode::Tab => {
                                if has_matches {
                                    let idx = app.selected_index.unwrap_or(0);
                                    if idx < matches.len() {
                                        let (cmd, _) = &matches[idx];
                                        app.typed_input = cmd.chars().collect();
                                        app.cursor_idx = app.typed_input.len();
                                        app.selected_index = None;
                                    }
                                }
                            }
                            KeyCode::PageUp => {
                                app.scroll_up(SCROLL_PAGE);
                            }
                            KeyCode::PageDown => {
                                app.scroll_down(SCROLL_PAGE);
                            }
                            KeyCode::Up => {
                                let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);
                                let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                                let has_alt = key.modifiers.contains(KeyModifiers::ALT);

                                if has_shift || has_ctrl || has_alt {
                                    app.scroll_up(SCROLL_STEP);
                                } else if has_matches {
                                    if let Some(idx) = app.selected_index {
                                        if idx > 0 {
                                            app.selected_index = Some(idx - 1);
                                        } else {
                                            app.selected_index =
                                                Some(matches.len().saturating_sub(1));
                                        }
                                    } else {
                                        app.selected_index = Some(matches.len().saturating_sub(1));
                                    }
                                } else if !app.auto_scroll {
                                    // When scrolled up in history, plain Up continues scrolling up
                                    app.scroll_up(SCROLL_JUMP);
                                } else if !app.prompt_history.is_empty()
                                    && (app.history_idx.is_some() || !app.typed_input.is_empty())
                                {
                                    let next_idx = match app.history_idx {
                                        None => app.prompt_history.len().saturating_sub(1),
                                        Some(i) => i.saturating_sub(1),
                                    };
                                    app.history_idx = Some(next_idx);
                                    if let Some(hist_str) = app.prompt_history.get(next_idx) {
                                        app.typed_input = hist_str.chars().collect();
                                        app.cursor_idx = app.typed_input.len();
                                    }
                                } else {
                                    // When input is empty at the bottom, plain Up scrolls the timeline
                                    app.scroll_up(SCROLL_JUMP);
                                }
                            }
                            KeyCode::Down => {
                                let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);
                                let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                                let has_alt = key.modifiers.contains(KeyModifiers::ALT);

                                if has_shift || has_ctrl || has_alt {
                                    app.scroll_down(SCROLL_STEP);
                                } else if has_matches {
                                    if let Some(idx) = app.selected_index {
                                        if idx + 1 < matches.len() {
                                            app.selected_index = Some(idx + 1);
                                        } else {
                                            app.selected_index = Some(0);
                                        }
                                    } else {
                                        app.selected_index = Some(0);
                                    }
                                } else if !app.auto_scroll {
                                    // When scrolled up, plain Down continues scrolling down toward the bottom
                                    app.scroll_down(SCROLL_JUMP);
                                } else if let Some(i) = app.history_idx {
                                    if i + 1 < app.prompt_history.len() {
                                        let next_idx = i + 1;
                                        app.history_idx = Some(next_idx);
                                        if let Some(hist_str) = app.prompt_history.get(next_idx) {
                                            app.typed_input = hist_str.chars().collect();
                                            app.cursor_idx = app.typed_input.len();
                                        }
                                    } else {
                                        app.history_idx = None;
                                        app.typed_input.clear();
                                        app.cursor_idx = 0;
                                    }
                                } else {
                                    // When input is empty, plain Down scrolls the timeline
                                    app.scroll_down(SCROLL_JUMP);
                                }
                            }
                            KeyCode::Char(c) => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'p' {
                                    app.scroll_up(SCROLL_STEP_EM);
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'n'
                                {
                                    app.scroll_down(SCROLL_STEP_EM);
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'u'
                                {
                                    app.scroll_up(SCROLL_HALF);
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'd'
                                {
                                    app.scroll_down(SCROLL_HALF);
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'b'
                                {
                                    app.scroll_up(SCROLL_PAGE);
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'f'
                                {
                                    app.scroll_down(SCROLL_PAGE);
                                } else {
                                    app.typed_input.insert(app.cursor_idx, c);
                                    app.cursor_idx += 1;
                                    app.selected_index = None;
                                    app.history_idx = None;
                                }
                            }
                            KeyCode::Left => {
                                app.cursor_idx = app.cursor_idx.saturating_sub(1);
                            }
                            KeyCode::Right => {
                                if app.cursor_idx < app.typed_input.len() {
                                    app.cursor_idx += 1;
                                }
                            }
                            KeyCode::Home => {
                                if key.modifiers.contains(KeyModifiers::SHIFT)
                                    || key.modifiers.contains(KeyModifiers::CONTROL)
                                    || app.typed_input.is_empty()
                                {
                                    app.scroll_to_top();
                                } else {
                                    app.cursor_idx = 0;
                                }
                            }
                            KeyCode::End => {
                                if key.modifiers.contains(KeyModifiers::SHIFT)
                                    || key.modifiers.contains(KeyModifiers::CONTROL)
                                    || app.typed_input.is_empty()
                                {
                                    app.scroll_to_bottom();
                                } else {
                                    app.cursor_idx = app.typed_input.len();
                                }
                            }
                            KeyCode::Backspace => {
                                if app.cursor_idx > 0 {
                                    app.typed_input.remove(app.cursor_idx - 1);
                                    app.cursor_idx -= 1;
                                }
                                app.selected_index = None;
                                app.history_idx = None;
                            }
                            KeyCode::Enter => {
                                let input_str = if let Some(idx) = app.selected_index {
                                    if idx < matches.len() {
                                        matches[idx].0.to_string()
                                    } else {
                                        app.typed_input.iter().collect::<String>()
                                    }
                                } else {
                                    app.typed_input.iter().collect::<String>()
                                };

                                let trimmed = input_str.trim();

                                if !trimmed.is_empty() {
                                    app.prompt_history.push(input_str.clone());

                                    if trimmed.starts_with('/') || trimmed == "exit" || trimmed == "quit" {
                                        let current_key = session_key.read().await.clone();
                                        match commands::handle_slash_command(
                                            trimmed,
                                            &mut app,
                                            &agent_loop,
                                            &current_key,
                                            &session_manager,
                                            &config,
                                        )
                                        .await
                                        {
                                            commands::SlashResult::Message(msg) => {
                                                app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                                app.messages.push(msg);
                                                app.scroll_to_bottom();
                                            }
                                            commands::SlashResult::OpenModal(m) => {
                                                app.modal = m;
                                            }
                                            commands::SlashResult::ClearTimeline => {
                                                app.messages.clear();
                                                app.scroll_to_top();
                                            }
                                            commands::SlashResult::NewSession => {
                                                if app.is_thinking {
                                                    app.messages.push(ChatMessage::notice(
                                                        "⏳ Cannot start a new session while a turn is running — Ctrl+C to cancel first.".to_string(),
                                                    ));
                                                    continue;
                                                }
                                                reset_active_session(
                                                    &session_key,
                                                    &session_manager,
                                                    &base_session_key,
                                                )
                                                .await;
                                                app.session_key = base_session_key.clone();
                                                let _ = write_tui_marker_in_dir(
                                                    &marker_dir,
                                                    std::process::id(),
                                                    &base_session_key,
                                                    &app.model,
                                                    &app.provider,
                                                );
                                                app.messages.clear();
                                                app.scroll_to_top();
                                                app.messages.push(ChatMessage::notice(
                                                    "Started a fresh conversation session.".to_string(),
                                                ));
                                            }
                                            commands::SlashResult::Exit => {
                                                break;
                                            }
                                            commands::SlashResult::Unhandled => {
                                                app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                                app.messages.push(ChatMessage::simple(
                                                    "assistant",
                                                    format!(
                                                        "Command `{}` not recognized. Type `/help` for available commands.",
                                                        trimmed
                                                    ),
                                                ));
                                                app.scroll_to_bottom();
                                            }
                                        }
                                    } else if app.is_thinking {
                                        // A turn is already running — queue it non-blockingly!
                                        app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                        app.queue_prompt(input_str.clone());
                                        let count = app.queued_prompts.len();
                                        app.messages.push(ChatMessage::notice(format!(
                                            "⏳ Queued as next turn ({} waiting) — will run once current step finishes.",
                                            count
                                        )));
                                        app.scroll_to_bottom();
                                    } else {
                                        // Standard User Prompt -> Dispatch to AgentLoop
                                        app.messages.push(ChatMessage::simple("user", input_str.clone()));
                                        app.is_thinking = true;
                                        app.work_start = Some(Instant::now());
                                        app.scroll_to_bottom();

                                        let turn_session_key = session_key.read().await.clone();
                                        current_turn_handle = Some(spawn_agent_turn(
                                            input_str.clone(),
                                            agent_loop.clone(),
                                            turn_session_key,
                                            session_manager.clone(),
                                            workspace.clone(),
                                            turn_tx.clone(),
                                        ));
                                    }

                                    app.typed_input.clear();
                                    app.cursor_idx = 0;
                                    app.selected_index = None;
                                    app.history_idx = None;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        if app.should_exit {
            break;
        }
    }

    remove_tui_marker_in_dir(&marker_dir, std::process::id());
    if is_last_live_tui_in_dir(&marker_dir, std::process::id()) {
        let _ = save_default_model_selection(&app.provider, &app.model);
    }

    let exit_session_key = session_key.read().await.clone();
    crate::agent::activity::remove_active_tui_session(&exit_session_key);
    crate::shutdown::trigger();
    crate::channels::shutdown_gateways_bounded(&config).await;

    Ok(())
}
