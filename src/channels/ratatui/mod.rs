pub mod animation;
pub mod app;
pub mod commands;
pub mod diff_viewer;
pub mod markdown;
pub mod modals;
pub mod session;
pub mod theme;
pub mod timeline;
pub mod ui;
pub mod welcome;

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
    ToolStart {
        name: String,
        details: String,
    },
    ToolEnd {
        name: String,
        output: String,
        summary: Option<String>,
        success: bool,
        duration_ms: Option<u64>,
    },
    Activity(animation::AgentActivity),
    Error(String),
}

pub static ACTIVE_RATATUI_SESSION_KEY: std::sync::LazyLock<std::sync::RwLock<Option<String>>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(None));

pub fn set_active_ratatui_session_key(session_key: Option<String>) {
    match ACTIVE_RATATUI_SESSION_KEY.write() {
        Ok(mut guard) => *guard = session_key,
        Err(_) => {}
    }
}

pub fn is_ratatui_session_active(session_key: Option<&str>) -> bool {
    match IS_RATATUI_ACTIVE.load(Ordering::Relaxed) {
        false => false,
        true => match ACTIVE_RATATUI_SESSION_KEY.read() {
            Ok(guard) => match (guard.as_deref(), session_key) {
                (Some(active_key), Some(candidate)) => active_key == candidate,
                (Some(_), None) => true,
                (None, _) => true,
            },
            Err(_) => true,
        },
    }
}

pub static RATATUI_EVENT_CHANNEL: std::sync::LazyLock<(
    tokio::sync::mpsc::UnboundedSender<TurnEvent>,
    tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<TurnEvent>>,
)> = std::sync::LazyLock::new(|| {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    (tx, tokio::sync::Mutex::new(rx))
});

pub fn send_ratatui_turn_event(event: TurnEvent) {
    let current_session = crate::agent::style::spinner::get_current_session_key();
    match is_ratatui_session_active(current_session.as_deref()) {
        true => {
            let _ = RATATUI_EVENT_CHANNEL.0.send(event);
        }
        false => {}
    }
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
    match is_ratatui_session_active(Some(session_key)) {
        false => return Ok(false),
        true => {}
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let req = SecurityApprovalRequest {
        tool_name: tool_name.to_string(),
        description: description.to_string(),
        session_key: session_key.to_string(),
        tx,
    };
    match SECURITY_APPROVAL_CHANNEL.0.send(req) {
        Err(_) => Ok(false),
        Ok(()) => match rx.await {
            Ok(approved) => Ok(approved),
            Err(_) => Ok(false),
        },
    }
}

fn spawn_agent_turn(
    prompt: String,
    agent_loop: Arc<crate::agent::AgentLoop>,
    session_key: String,
    session_manager: Arc<crate::session::SessionManager>,
    workspace: std::path::PathBuf,
    turn_tx: tokio::sync::mpsc::UnboundedSender<TurnEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let run_result = crate::config::loader::ACTIVE_WORKSPACE
            .scope(workspace, async {
                agent_loop.run(&prompt, &session_key).await
            })
            .await;
        match run_result {
            Ok(res) => match session_manager.load(&session_key) {
                Ok(session) => {
                    let msgs = ChatMessage::from_session_messages(&session.messages);
                    let _ = turn_tx.send(TurnEvent::SyncSession(msgs));
                }
                Err(_) => {
                    let _ = turn_tx.send(TurnEvent::SingleMessage(ChatMessage::simple(
                        "assistant",
                        res.content,
                    )));
                }
            },
            Err(err) => {
                let _ = turn_tx.send(TurnEvent::Error(err.to_string()));
            }
        }
    })
}

fn wrap_prev(idx: usize, len: usize) -> usize {
    match idx > 0 {
        true => idx - 1,
        false => len.saturating_sub(1),
    }
}

fn wrap_next(idx: usize, len: usize) -> usize {
    match idx + 1 < len {
        true => idx + 1,
        false => 0,
    }
}

fn send_approval_decision(
    tx: &Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<bool>>>>,
    approved: bool,
) {
    match tx.try_lock() {
        Ok(mut guard) => match guard.take() {
            Some(sender) => {
                let _ = sender.send(approved);
            }
            None => {}
        },
        Err(_) => {}
    }
}

fn spawn_background_channels(
    config: &crate::config::schema::Config,
    agent_loop: Arc<crate::agent::AgentLoop>,
) {
    match &config.channels.websocket {
        Some(ws_config) => match ws_config.enabled && ws_config.start_on_tui {
            true => {
                let ws_config_clone = ws_config.clone();
                let shared_loop = agent_loop.clone();
                tokio::spawn(async move {
                    let gateway =
                        crate::channels::WsGateway::with_shared_loop(ws_config_clone, shared_loop);
                    let _ = gateway.start().await;
                });
            }
            false => {}
        },
        None => {}
    }

    match &config.channels.telegram {
        Some(tg_config) => match tg_config.enabled {
            true => {
                let token = match tg_config.bot_token.is_empty() {
                    true => std::env::var("TELEGRAM_BOT_TOKEN").ok(),
                    false => Some(tg_config.bot_token.clone()),
                };
                match token {
                    Some(token) => {
                        let shared_loop = agent_loop.clone();
                        tokio::spawn(async move {
                            let channel = crate::channels::TelegramChannel::with_shared_loop(
                                token,
                                shared_loop,
                            );
                            let _ = channel.start().await;
                        });
                    }
                    None => {}
                }
            }
            false => {}
        },
        None => {}
    }

    match &config.channels.discord {
        Some(dc_config) => match dc_config.enabled {
            true => {
                let token = match dc_config.bot_token.is_empty() {
                    true => std::env::var("DISCORD_BOT_TOKEN").ok(),
                    false => Some(dc_config.bot_token.clone()),
                };
                match token {
                    Some(token) => {
                        let shared_loop = agent_loop.clone();
                        tokio::spawn(async move {
                            let channel = crate::channels::DiscordChannel::with_shared_loop(
                                token,
                                shared_loop,
                            );
                            let _ = channel.start().await;
                        });
                    }
                    None => {}
                }
            }
            false => {}
        },
        None => {}
    }

    match &config.channels.whatsapp {
        Some(wa_config) => match wa_config.enabled {
            true => {
                let wa_config_clone = wa_config.clone();
                let shared_loop = agent_loop.clone();
                tokio::spawn(async move {
                    let channel = crate::channels::WhatsAppChannel::with_shared_loop(
                        wa_config_clone.api_key,
                        wa_config_clone.phone_number_id,
                        shared_loop,
                    );
                    let _ = channel.start().await;
                });
            }
            false => {}
        },
        None => {}
    }

    match &config.channels.email {
        Some(email_config) => match email_config.enabled {
            true => {
                let shared_loop = agent_loop.clone();
                tokio::spawn(async move {
                    let channel = crate::channels::EmailChannel::with_shared_loop(shared_loop);
                    let _ = channel.start().await;
                });
            }
            false => {}
        },
        None => {}
    }
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
        let _ = stdout().execute(crossterm::event::DisableBracketedPaste);
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
        let _ = stdout().execute(crossterm::cursor::Show);
        IS_RATATUI_ACTIVE.store(false, Ordering::SeqCst);
        set_active_ratatui_session_key(None);
    }
}

static RATATUI_PANIC_HOOK_ONCE: std::sync::Once = std::sync::Once::new();

fn install_ratatui_panic_hook() {
    RATATUI_PANIC_HOOK_ONCE.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            match IS_RATATUI_ACTIVE.swap(false, Ordering::SeqCst) {
                true => {
                    let _ = stdout().execute(crossterm::event::DisableBracketedPaste);
                    let _ = stdout().execute(LeaveAlternateScreen);
                    let _ = disable_raw_mode();
                    let _ = stdout().execute(crossterm::cursor::Show);
                    set_active_ratatui_session_key(None);
                }
                false => {}
            }
            default_hook(info);
        }));
    });
}

pub fn handle_paste_into_app(app: &mut RatatuiApp, raw_paste: &str) {
    match app.modal.is_active() {
        true => match &mut app.modal {
            ModalState::CommandCatalog { filter, .. } => {
                let single_line: String = RatatuiApp::sanitize_paste_text(raw_paste)
                    .chars()
                    .map(|ch| match ch {
                        '\n' => ' ',
                        other => other,
                    })
                    .collect();
                filter.push_str(single_line.trim());
                app.modal.update_command_catalog_filter();
            }
            ModalState::ModelSelect {
                filter,
                loading: false,
                ..
            } => {
                let single_line: String = RatatuiApp::sanitize_paste_text(raw_paste)
                    .chars()
                    .map(|ch| match ch {
                        '\n' => ' ',
                        other => other,
                    })
                    .collect();
                filter.push_str(single_line.trim());
                app.modal.update_model_filter();
            }
            _ => {}
        },
        false => {
            app.insert_pasted_text(raw_paste);
        }
    }
}

pub fn coalesce_unbracketed_paste_batch(events: Vec<Event>, modal_active: bool) -> Vec<Event> {
    match (modal_active, events.len() >= 2) {
        (false, true) => {
            let mut coalesced = String::new();
            let mut all_paste_like = true;
            let mut has_enter = false;

            for ev in &events {
                match ev {
                    Event::Key(key) => {
                        let is_press = matches!(key.kind, KeyEventKind::Press);
                        let has_ctrl_or_alt = key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
                        match (is_press, has_ctrl_or_alt, key.code) {
                            (true, false, KeyCode::Char(c)) => coalesced.push(c),
                            (true, false, KeyCode::Tab) => coalesced.push('\t'),
                            (true, false, KeyCode::Enter) => {
                                has_enter = true;
                                coalesced.push('\n');
                            }
                            _ => {
                                all_paste_like = false;
                            }
                        }
                    }
                    _ => {
                        all_paste_like = false;
                    }
                }
            }

            let is_burst_paste = all_paste_like && (events.len() >= 8 || has_enter);
            match is_burst_paste {
                true => vec![Event::Paste(coalesced)],
                false => events,
            }
        }
        _ => events,
    }
}

pub async fn handle_ratatui_tui() -> Result<()> {
    let config = crate::config::loader::load_config().unwrap_or_default();
    let mut model = config.agents.defaults.model.clone();
    let mut provider = config.agents.defaults.provider.clone();
    let marker_dir = tui_marker_dir();
    let current_pid = std::process::id();
    let default_base_key = crate::config::loader::get_cli_session_key();
    let base_session_key =
        allocate_tui_session_key_in_dir(&marker_dir, current_pid, &default_base_key);
    let session_key = Arc::new(tokio::sync::RwLock::new(base_session_key.clone()));
    let workspace = crate::config::loader::active_workspace_or_current_dir();

    let sessions_dir = crate::config::loader::sessions_dir();
    let session_manager = Arc::new(crate::session::SessionManager::new(sessions_dir));

    let agent_loop = Arc::new(crate::cli::builder::build_agent_loop(config.clone()).await?);

    crate::cron::scheduler::start_scheduler(config.clone());

    let history = crate::cli::load_session_history()?;
    match history.is_empty() {
        true => {
            match is_session_claimed_by_other_live_tui_in_dir(
                &marker_dir,
                current_pid,
                &base_session_key,
            ) {
                false => {
                    crate::cli::archive_current_session(&session_manager, &base_session_key)
                        .await?;
                }
                true => {}
            }
        }
        false => {
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
            reset_active_session(&session_key, &session_manager, &base_session_key).await;
            match selected {
                0 => {}
                idx => {
                    let chosen_key = history[idx - 1].key.clone();
                    let actual_key = allocate_tui_session_key_in_dir(
                        &marker_dir,
                        current_pid,
                        &chosen_key,
                    );
                    *session_key.write().await = actual_key;
                }
            }
        }
    }

    let defaults = config.agents.defaults.clone();
    let tui_started_at = chrono::Utc::now().to_rfc3339();
    let tui_cwd = std::env::current_dir().unwrap_or_default();
    let initial_active_key = session_key.read().await.clone();
    let initial_preview = session_manager
        .load(&initial_active_key)
        .ok()
        .map(|session| crate::agent::activity::session_preview_from_messages(&session.messages))
        .unwrap_or_else(|| "No user prompt yet".to_string());
    let active_tui = crate::agent::activity::make_active_tui_session(
        &initial_active_key,
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

    crate::cli::set_silent_mode(true);
    spawn_background_channels(&config, agent_loop.clone());

    install_ratatui_panic_hook();
    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(crossterm::event::EnableBracketedPaste)?;
    stdout.execute(crossterm::cursor::Show)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let active_key = session_key.read().await.clone();
    set_active_ratatui_session_key(Some(active_key.clone()));
    IS_RATATUI_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = RatatuiGuard;

    let mut session_config = config.clone();
    match session_manager.load(&active_key) {
        Ok(session) => {
            crate::agent::agent_loop::apply_session_overrides(
                &mut session_config,
                &session.metadata,
            );
            model = session_config.agents.defaults.model.clone();
            provider = session_config.agents.defaults.provider.clone();
        }
        Err(_) => {}
    }

    let mut app = RatatuiApp::new(model, provider, active_key.clone());
    let _ = write_tui_marker_in_dir(
        &marker_dir,
        current_pid,
        &active_key,
        &app.model,
        &app.provider,
    );

    match session_manager.load(&active_key) {
        Ok(session) => {
            app.messages = ChatMessage::from_session_messages(&session.messages);
            app.update_approx_tokens();
        }
        Err(_) => {}
    }

    let (turn_tx, mut turn_rx) = tokio::sync::mpsc::unbounded_channel::<TurnEvent>();
    let (model_tx, mut model_rx) =
        tokio::sync::mpsc::unbounded_channel::<(String, String, Vec<String>)>();
    let mut current_turn_handle: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        match SECURITY_APPROVAL_CHANNEL.1.try_lock() {
            Ok(mut rx_lock) => match rx_lock.try_recv() {
                Ok(req) => {
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
                Err(_) => {}
            },
            Err(_) => {}
        }

        while let Ok((prov_name, prov_display, fetched_models)) = model_rx.try_recv() {
            let matches_provider = match &app.modal {
                ModalState::ModelSelect { provider_name, .. } => provider_name == &prov_name,
                _ => false,
            };
            match matches_provider {
                true => {
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
                false => {}
            }
        }

        let mut turn_events = Vec::new();
        while let Ok(event) = turn_rx.try_recv() {
            turn_events.push(event);
        }
        match RATATUI_EVENT_CHANNEL.1.try_lock() {
            Ok(mut rx_lock) => {
                while let Ok(event) = rx_lock.try_recv() {
                    turn_events.push(event);
                }
            }
            Err(_) => {}
        }

        for event in turn_events {
            match event {
                TurnEvent::Activity(act) => {
                    app.set_activity(act);
                }
                TurnEvent::ToolStart { name, details } => {
                    app.handle_tool_start(name, details);
                }
                TurnEvent::ToolEnd {
                    name,
                    output,
                    summary,
                    success,
                    duration_ms,
                } => {
                    app.handle_tool_end(&name, output, summary, success, duration_ms);
                }
                TurnEvent::SyncSession(msgs) => {
                    app.is_thinking = false;
                    app.work_start = None;
                    app.clear_activity();
                    current_turn_handle = None;
                    app.apply_sync_session(msgs);
                }
                TurnEvent::SingleMessage(msg) => {
                    let is_live_thought = msg.reasoning.is_some() && msg.content.is_empty();
                    match is_live_thought {
                        true => {
                            app.messages.push(msg);
                            app.update_approx_tokens();
                        }
                        false => {
                            app.is_thinking = false;
                            app.work_start = None;
                            app.clear_activity();
                            current_turn_handle = None;
                            app.messages.push(msg);
                            app.update_approx_tokens();
                        }
                    }
                }
                TurnEvent::Error(err) => {
                    app.is_thinking = false;
                    app.work_start = None;
                    app.clear_activity();
                    current_turn_handle = None;
                    app.messages
                        .push(ChatMessage::notice(format!("⚠ Error: {}", err)));
                }
            }
            app.scroll_to_bottom();

            match app.is_thinking {
                false => match app.pop_next_prompt() {
                    Some(next_prompt) => {
                        app.is_thinking = true;
                        app.work_start = Some(Instant::now());
                        app.set_activity(animation::AgentActivity::from_user_prompt(&next_prompt));
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
                    None => {}
                },
                true => {}
            }
        }

        app.spinner_idx = app.spinner_idx.wrapping_add(1);
        terminal.draw(|f| ui::render_ratatui_ui(f, &mut app))?;

        match event::poll(Duration::from_millis(TICK_MS))? {
            false => {}
            true => {
                let mut raw_events = vec![event::read()?];
                while raw_events.len() < 4096 && event::poll(Duration::ZERO)? {
                    raw_events.push(event::read()?);
                }
                let batch_events =
                    coalesce_unbracketed_paste_batch(raw_events, app.modal.is_active());

                for ev in batch_events {
                    match ev {
                        Event::Paste(pasted_text) => {
                            handle_paste_into_app(&mut app, &pasted_text);
                        }
                        Event::Mouse(mouse) => match (app.modal.is_active(), mouse.kind) {
                            (false, MouseEventKind::ScrollUp) => app.scroll_up(SCROLL_STEP),
                            (false, MouseEventKind::ScrollDown) => app.scroll_down(SCROLL_STEP),
                            _ => {}
                        },
                        Event::Key(key) => match key.kind {
                            KeyEventKind::Press => {
                                let is_ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                                    && key.code == KeyCode::Char('c');
                                match is_ctrl_c {
                                    true => {
                                        match (&app.modal, app.modal.is_active(), app.is_thinking)
                                        {
                                            (ModalState::ExitConfirm { .. }, _, _) => {
                                                app.should_exit = true;
                                                break;
                                            }
                                            (ModalState::SecurityApproval { tx, .. }, _, _) => {
                                                send_approval_decision(tx, false);
                                                app.modal = ModalState::None;
                                                continue;
                                            }
                                            (_, true, _) => {
                                                app.modal = ModalState::None;
                                                continue;
                                            }
                                            (_, false, true) => {
                                                match current_turn_handle.take() {
                                                    Some(handle) => handle.abort(),
                                                    None => {}
                                                }
                                                crate::shutdown::trigger_cli_cancel();
                                                app.is_thinking = false;
                                                app.work_start = None;
                                                app.messages.push(ChatMessage::notice(
                                                    "Turn cancelled by user.".to_string(),
                                                ));
                                                continue;
                                            }
                                            (_, false, false) => {
                                                app.modal =
                                                    ModalState::ExitConfirm { selected_yes: false };
                                                continue;
                                            }
                                        }
                                    }
                                    false => {}
                                }

                                // ── 1. Modal Key Interception ───────────────────────────
                                match app.modal.is_active() {
                                    true => {
                                        match &mut app.modal {
                                            ModalState::ExitConfirm { selected_yes } => {
                                                match key.code {
                                                    KeyCode::Left
                                                    | KeyCode::Right
                                                    | KeyCode::Tab
                                                    | KeyCode::BackTab => {
                                                        *selected_yes = !*selected_yes;
                                                    }
                                                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                                                        app.should_exit = true;
                                                        break;
                                                    }
                                                    KeyCode::Char('n')
                                                    | KeyCode::Char('N')
                                                    | KeyCode::Esc => {
                                                        app.modal = ModalState::None;
                                                    }
                                                    KeyCode::Enter => match *selected_yes {
                                                        true => {
                                                            app.should_exit = true;
                                                            break;
                                                        }
                                                        false => app.modal = ModalState::None,
                                                    },
                                                    _ => {}
                                                }
                                            }
                                    ModalState::CommandCatalog {
                                        filtered_indices,
                                        selected_index,
                                        filter,
                                    } => match key.code {
                                        KeyCode::Up => {
                                            *selected_index =
                                                wrap_prev(*selected_index, filtered_indices.len());
                                        }
                                        KeyCode::Down => {
                                            *selected_index =
                                                wrap_next(*selected_index, filtered_indices.len());
                                        }
                                        KeyCode::Esc => {
                                            app.modal = ModalState::None;
                                        }
                                        KeyCode::Backspace => {
                                            filter.pop();
                                            app.modal.update_command_catalog_filter();
                                        }
                                        KeyCode::Char(c) => {
                                            filter.push(c);
                                            app.modal.update_command_catalog_filter();
                                        }
                                        KeyCode::Enter => {
                                            match filtered_indices.get(*selected_index) {
                                                Some(&cmd_idx) => {
                                                    let cmd = &app::PALETTE_COMMANDS[cmd_idx];
                                                    app.typed_input =
                                                        cmd.slash_name.chars().collect();
                                                    app.cursor_idx = app.typed_input.len();
                                                }
                                                None => {}
                                            }
                                            app.modal = ModalState::None;
                                        }
                                        _ => {}
                                    },
                                    ModalState::ProviderSelect {
                                        providers,
                                        selected_idx,
                                    } => match key.code {
                                        KeyCode::Up => {
                                            *selected_idx =
                                                wrap_prev(*selected_idx, providers.len());
                                        }
                                        KeyCode::Down => {
                                            *selected_idx =
                                                wrap_next(*selected_idx, providers.len());
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
                                                let mut models =
                                                    crate::channels::fetch_provider_models(
                                                        &fetch_prov,
                                                        &config,
                                                    )
                                                    .await
                                                    .unwrap_or_default();
                                                for m in app::curated_models_for(&fetch_prov) {
                                                    let exists = models.iter().any(|existing| {
                                                        existing.eq_ignore_ascii_case(&m)
                                                    });
                                                    match exists {
                                                        false => models.push(m),
                                                        true => {}
                                                    }
                                                }
                                                models =
                                                    crate::channels::model_menu_options_with_prefs(
                                                        &fetch_prov,
                                                        models,
                                                    );
                                                let _ = fetch_tx
                                                    .send((fetch_prov, fetch_display, models));
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
                                    } => match (*loading, key.code) {
                                        (true, KeyCode::Esc) => {
                                            app.modal = ModalState::None;
                                        }
                                        (true, _) => {}
                                        (false, KeyCode::Up) => {
                                            *selected_idx =
                                                wrap_prev(*selected_idx, filtered_indices.len());
                                        }
                                        (false, KeyCode::Down) => {
                                            *selected_idx =
                                                wrap_next(*selected_idx, filtered_indices.len());
                                        }
                                        (false, KeyCode::Esc) => {
                                            app.modal = ModalState::None;
                                        }
                                        (false, KeyCode::Backspace) => {
                                            filter.pop();
                                            app.modal.update_model_filter();
                                        }
                                        (false, KeyCode::Char(c)) => {
                                            filter.push(c);
                                            app.modal.update_model_filter();
                                        }
                                        (false, KeyCode::Enter) => {
                                            match filtered_indices.is_empty() {
                                                false => {
                                                    let real_idx = filtered_indices[*selected_idx];
                                                    let chosen_model =
                                                        crate::channels::model_menu_model_name(
                                                            &models[real_idx],
                                                        )
                                                        .to_string();
                                                    let prov = provider_name.clone();

                                                    match apply_session_model_selection(
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
                                                true => {}
                                            }
                                            app.modal = ModalState::None;
                                        }
                                        (false, _) => {}
                                    },
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
                                            *selected_idx =
                                                wrap_prev(*selected_idx, sessions.len());
                                        }
                                        KeyCode::Down => {
                                            *selected_idx =
                                                wrap_next(*selected_idx, sessions.len());
                                        }
                                        KeyCode::Esc => {
                                            app.modal = ModalState::None;
                                        }
                                        KeyCode::Enter => {
                                            match (app.is_thinking, sessions.is_empty()) {
                                                (true, _) => {
                                                    app.messages.push(ChatMessage::notice(
                                                        "⏳ Cannot switch sessions while a turn is running — Ctrl+C to cancel first.".to_string(),
                                                    ));
                                                }
                                                (false, false) => {
                                                    let (target_key, title, _) =
                                                        sessions[*selected_idx].clone();
                                                    match session_manager.load(&target_key) {
                                                        Ok(loaded) => {
                                                            reset_active_session(
                                                                &session_key,
                                                                &session_manager,
                                                                &base_session_key,
                                                            )
                                                            .await;
                                                            let restored_key =
                                                                allocate_tui_session_key_in_dir(
                                                                    &marker_dir,
                                                                    current_pid,
                                                                    &target_key,
                                                                );
                                                            *session_key.write().await =
                                                                restored_key.clone();
                                                            set_active_ratatui_session_key(Some(
                                                                restored_key.clone(),
                                                            ));

                                                            let mut restored_cfg = config.clone();
                                                            crate::agent::agent_loop::apply_session_overrides(
                                                                &mut restored_cfg,
                                                                &loaded.metadata,
                                                            );
                                                            app.model = restored_cfg
                                                                .agents
                                                                .defaults
                                                                .model
                                                                .clone();
                                                            app.provider = restored_cfg
                                                                .agents
                                                                .defaults
                                                                .provider
                                                                .clone();
                                                            app.messages =
                                                                ChatMessage::from_session_messages(
                                                                    &loaded.messages,
                                                                );
                                                            app.update_approx_tokens();
                                                            app.session_key = restored_key.clone();
                                                            app.scroll_to_bottom();
                                                            app.messages.push(ChatMessage::notice(
                                                                format!(
                                                                    "✓ Restored session: {}",
                                                                    title
                                                                ),
                                                            ));
                                                            let _ = write_tui_marker_in_dir(
                                                                &marker_dir,
                                                                current_pid,
                                                                &restored_key,
                                                                &app.model,
                                                                &app.provider,
                                                            );
                                                        }
                                                        Err(_) => {}
                                                    }
                                                }
                                                (false, true) => {}
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
                                            *selected_idx =
                                                wrap_prev(*selected_idx, options.len());
                                        }
                                        KeyCode::Down => {
                                            *selected_idx =
                                                wrap_next(*selected_idx, options.len());
                                        }
                                        KeyCode::Esc => {
                                            send_approval_decision(tx, false);
                                            app.modal = ModalState::None;
                                        }
                                        KeyCode::Enter => {
                                            let approved =
                                                *selected_idx == 0 || *selected_idx == 1;
                                            match *selected_idx {
                                                1 => crate::agent::security::trust_tool_for_session(
                                                    approval_session,
                                                    tool_name,
                                                ),
                                                _ => {}
                                            }
                                            send_approval_decision(tx, approved);
                                            app.modal = ModalState::None;
                                        }
                                        _ => {}
                                    },
                                    ModalState::None => {}
                                }
                                continue;
                            }
                            false => {}
                        }

                        // ── Global Function & Shortcut Keys ────────────────────
                        let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                        match (has_ctrl, key.code) {
                            (true, KeyCode::Char('l')) => {
                                let configured =
                                    crate::channels::build_configured_providers(&config);
                                app.modal = ModalState::ProviderSelect {
                                    providers: configured,
                                    selected_idx: 0,
                                };
                                continue;
                            }
                            (true, KeyCode::Char('h')) => {
                                let sessions =
                                    crate::cli::load_session_history().unwrap_or_default();
                                let items: Vec<(String, String, String)> = sessions
                                    .into_iter()
                                    .map(|s| {
                                        (
                                            s.key,
                                            s.display_title,
                                            s.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                                        )
                                    })
                                    .collect();
                                app.modal = ModalState::History {
                                    sessions: items,
                                    selected_idx: 0,
                                };
                                continue;
                            }
                            (true, KeyCode::Char('n')) => {
                                match app.is_thinking {
                                    false => {
                                        reset_active_session(
                                            &session_key,
                                            &session_manager,
                                            &base_session_key,
                                        )
                                        .await;
                                        let new_key = format!("cli:{}", uuid::Uuid::new_v4());
                                        *session_key.write().await = new_key.clone();
                                        set_active_ratatui_session_key(Some(new_key.clone()));
                                        app.messages.clear();
                                        app.update_approx_tokens();
                                        app.session_key = new_key.clone();
                                        app.messages.push(ChatMessage::notice(
                                            "✓ Started a fresh conversation session.".to_string(),
                                        ));
                                        let _ = write_tui_marker_in_dir(
                                            &marker_dir,
                                            current_pid,
                                            &new_key,
                                            &app.model,
                                            &app.provider,
                                        );
                                    }
                                    true => {}
                                }
                                continue;
                            }
                            (_, KeyCode::F(1)) => {
                                app.modal = ModalState::new_command_catalog();
                                continue;
                            }
                            (_, KeyCode::F(3)) => {
                                let current_key = session_key.read().await.clone();
                                match commands::handle_slash_command(
                                    "/settings",
                                    &mut app,
                                    &agent_loop,
                                    &current_key,
                                    &session_manager,
                                    &config,
                                )
                                .await
                                {
                                    commands::SlashResult::Message(msg) => {
                                        app.messages.push(msg);
                                        app.scroll_to_bottom();
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                            _ => {}
                        }

                        // ── Spotlight Command Palette Navigation ─────────────────
                        match app.has_active_slash_query() {
                            true => match key.code {
                                KeyCode::Esc => {
                                    app.typed_input.clear();
                                    app.cursor_idx = 0;
                                    app.slash_selected_idx = 0;
                                    continue;
                                }
                                KeyCode::Tab => {
                                    app.cycle_slash_category(true);
                                    continue;
                                }
                                KeyCode::BackTab => {
                                    app.cycle_slash_category(false);
                                    continue;
                                }
                                KeyCode::Up => {
                                    let matches = app.matching_palette_commands();
                                    match matches.is_empty() {
                                        false => {
                                            app.slash_selected_idx = wrap_prev(
                                                app.slash_selected_idx,
                                                matches.len(),
                                            );
                                        }
                                        true => {}
                                    }
                                    continue;
                                }
                                KeyCode::Down => {
                                    let matches = app.matching_palette_commands();
                                    match matches.is_empty() {
                                        false => {
                                            app.slash_selected_idx = wrap_next(
                                                app.slash_selected_idx,
                                                matches.len(),
                                            );
                                        }
                                        true => {}
                                    }
                                    continue;
                                }
                                _ => {}
                            },
                            false => {}
                        }

                        // ── 2. Standard View & Input Key Events ─────────────────
                        let matches = app.matching_slash_commands();
                        let has_matches = !matches.is_empty();
                        let has_any_mod = key.modifiers.intersects(
                            KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT,
                        );

                        match key.code {
                            KeyCode::Esc => match app.is_thinking {
                                true => {
                                    match current_turn_handle.take() {
                                        Some(handle) => handle.abort(),
                                        None => {}
                                    }
                                    crate::shutdown::trigger_cli_cancel();
                                    app.is_thinking = false;
                                    app.work_start = None;
                                    app.clear_activity();
                                    app.messages.push(ChatMessage::notice(
                                        "Turn interrupted by user.".to_string(),
                                    ));
                                }
                                false => {
                                    app.typed_input.clear();
                                    app.cursor_idx = 0;
                                    app.selected_index = None;
                                    app.history_idx = None;
                                }
                            },
                            KeyCode::Tab => {
                                let idx = app.selected_index.unwrap_or(0);
                                match has_matches && idx < matches.len() {
                                    true => {
                                        let (cmd, _) = &matches[idx];
                                        app.typed_input = cmd.chars().collect();
                                        app.cursor_idx = app.typed_input.len();
                                        app.selected_index = None;
                                    }
                                    false => {}
                                }
                            }
                            KeyCode::PageUp => app.scroll_up(SCROLL_PAGE),
                            KeyCode::PageDown => app.scroll_down(SCROLL_PAGE),
                            KeyCode::Up => {
                                let can_browse_history = !app.prompt_history.is_empty()
                                    && (app.history_idx.is_some() || !app.typed_input.is_empty());
                                match (has_any_mod, has_matches, app.auto_scroll, can_browse_history) {
                                    (true, _, _, _) => app.scroll_up(SCROLL_STEP),
                                    (false, true, _, _) => {
                                        let next = match app.selected_index {
                                            Some(idx) => wrap_prev(idx, matches.len()),
                                            None => matches.len().saturating_sub(1),
                                        };
                                        app.selected_index = Some(next);
                                    }
                                    (false, false, false, _) => app.scroll_up(SCROLL_JUMP),
                                    (false, false, true, true) => {
                                        let next_idx = match app.history_idx {
                                            None => app.prompt_history.len().saturating_sub(1),
                                            Some(i) => i.saturating_sub(1),
                                        };
                                        app.history_idx = Some(next_idx);
                                        match app.prompt_history.get(next_idx) {
                                            Some(hist_str) => {
                                                app.typed_input = hist_str.chars().collect();
                                                app.cursor_idx = app.typed_input.len();
                                            }
                                            None => {}
                                        }
                                    }
                                    (false, false, true, false) => app.scroll_up(SCROLL_JUMP),
                                }
                            }
                            KeyCode::Down => match (
                                has_any_mod,
                                has_matches,
                                app.auto_scroll,
                                app.history_idx,
                            ) {
                                (true, _, _, _) => app.scroll_down(SCROLL_STEP),
                                (false, true, _, _) => {
                                    let next = match app.selected_index {
                                        Some(idx) => wrap_next(idx, matches.len()),
                                        None => 0,
                                    };
                                    app.selected_index = Some(next);
                                }
                                (false, false, false, _) => app.scroll_down(SCROLL_JUMP),
                                (false, false, true, Some(i)) => {
                                    match i + 1 < app.prompt_history.len() {
                                        true => {
                                            let next_idx = i + 1;
                                            app.history_idx = Some(next_idx);
                                            match app.prompt_history.get(next_idx) {
                                                Some(hist_str) => {
                                                    app.typed_input = hist_str.chars().collect();
                                                    app.cursor_idx = app.typed_input.len();
                                                }
                                                None => {}
                                            }
                                        }
                                        false => {
                                            app.history_idx = None;
                                            app.typed_input.clear();
                                            app.cursor_idx = 0;
                                        }
                                    }
                                }
                                (false, false, true, None) => app.scroll_down(SCROLL_JUMP),
                            },
                            KeyCode::Char(c) => match (has_ctrl, c) {
                                (true, 'p') => app.scroll_up(SCROLL_STEP_EM),
                                (true, 'n') => app.scroll_down(SCROLL_STEP_EM),
                                (true, 'u') => app.scroll_up(SCROLL_HALF),
                                (true, 'd') => app.scroll_down(SCROLL_HALF),
                                (true, 'b') => app.scroll_up(SCROLL_PAGE),
                                (true, 'f') => app.scroll_down(SCROLL_PAGE),
                                (true, 'v') | (true, 'V') => {
                                    match arboard::Clipboard::new()
                                        .and_then(|mut cb| cb.get_text())
                                    {
                                        Ok(clip_text) => {
                                            handle_paste_into_app(&mut app, &clip_text);
                                        }
                                        Err(_) => {}
                                    }
                                }
                                _ => {
                                    app.insert_char(c);
                                }
                            },
                            KeyCode::Left => {
                                app.cursor_idx = app.cursor_idx.saturating_sub(1);
                            }
                            KeyCode::Right => match app.cursor_idx < app.typed_input.len() {
                                true => app.cursor_idx += 1,
                                false => {}
                            },
                            KeyCode::Home => {
                                let scroll_top = key
                                    .modifiers
                                    .intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL)
                                    || app.typed_input.is_empty();
                                match scroll_top {
                                    true => app.scroll_to_top(),
                                    false => app.cursor_idx = 0,
                                }
                            }
                            KeyCode::End => {
                                let scroll_bottom = key
                                    .modifiers
                                    .intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL)
                                    || app.typed_input.is_empty();
                                match scroll_bottom {
                                    true => app.scroll_to_bottom(),
                                    false => app.cursor_idx = app.typed_input.len(),
                                }
                            }
                            KeyCode::Backspace => {
                                app.delete_char_before_cursor();
                            }
                            KeyCode::Delete => {
                                app.delete_char_at_cursor();
                            }
                            KeyCode::Enter => {
                                let insert_newline = key
                                    .modifiers
                                    .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT);
                                match insert_newline {
                                    true => {
                                        app.insert_char('\n');
                                        continue;
                                    }
                                    false => {}
                                }

                                let typed_raw: String = app.typed_input.iter().collect();
                                let input_str = match (
                                    app.has_active_slash_query(),
                                    typed_raw.contains(' '),
                                    app.selected_palette_command(),
                                    app.selected_index,
                                ) {
                                    (true, false, Some(cmd), _) => cmd.slash_name.to_string(),
                                    (true, _, _, _) => typed_raw,
                                    (false, _, _, Some(idx)) => match matches.get(idx) {
                                        Some((cmd_str, _)) => cmd_str.clone(),
                                        None => typed_raw,
                                    },
                                    (false, _, _, None) => typed_raw,
                                };

                                let trimmed = input_str.trim();
                                match trimmed.is_empty() {
                                    true => {}
                                    false => {
                                        app.prompt_history.push(input_str.clone());
                                        let is_slash =
                                            RatatuiApp::is_slash_command_input(trimmed);

                                        match (is_slash, app.is_thinking) {
                                            (true, _) => {
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
                                                        app.messages.push(ChatMessage::simple(
                                                            "user",
                                                            input_str.clone(),
                                                        ));
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
                                                        match app.is_thinking {
                                                            true => {
                                                                app.messages.push(ChatMessage::notice(
                                                                    "⏳ Cannot start a new session while a turn is running — Ctrl+C to cancel first.".to_string(),
                                                                ));
                                                                continue;
                                                            }
                                                            false => {
                                                                reset_active_session(
                                                                    &session_key,
                                                                    &session_manager,
                                                                    &base_session_key,
                                                                )
                                                                .await;
                                                                set_active_ratatui_session_key(
                                                                    Some(base_session_key.clone()),
                                                                );
                                                                app.session_key =
                                                                    base_session_key.clone();
                                                                let _ = write_tui_marker_in_dir(
                                                                    &marker_dir,
                                                                    current_pid,
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
                                                        }
                                                    }
                                                    commands::SlashResult::Exit => {
                                                        app.should_exit = true;
                                                        break;
                                                    }
                                                    commands::SlashResult::Unhandled => {
                                                        app.messages.push(ChatMessage::simple(
                                                            "user",
                                                            input_str.clone(),
                                                        ));
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
                                            }
                                            (false, true) => {
                                                app.messages.push(ChatMessage::simple(
                                                    "user",
                                                    input_str.clone(),
                                                ));
                                                app.queue_prompt(input_str.clone());
                                                let count = app.queued_prompts.len();
                                                app.messages.push(ChatMessage::notice(format!(
                                                    "⏳ Queued as next turn ({} waiting) — will run once current step finishes.",
                                                    count
                                                )));
                                                app.scroll_to_bottom();
                                            }
                                            (false, false) => {
                                                app.messages.push(ChatMessage::simple(
                                                    "user",
                                                    input_str.clone(),
                                                ));
                                                app.is_thinking = true;
                                                app.work_start = Some(Instant::now());
                                                app.set_activity(
                                                    animation::AgentActivity::from_user_prompt(
                                                        &input_str,
                                                    ),
                                                );
                                                app.scroll_to_bottom();

                                                let turn_session_key =
                                                    session_key.read().await.clone();
                                                current_turn_handle = Some(spawn_agent_turn(
                                                    input_str.clone(),
                                                    agent_loop.clone(),
                                                    turn_session_key,
                                                    session_manager.clone(),
                                                    workspace.clone(),
                                                    turn_tx.clone(),
                                                ));
                                            }
                                        }

                                        app.typed_input.clear();
                                        app.cursor_idx = 0;
                                        app.selected_index = None;
                                        app.slash_selected_idx = 0;
                                        app.history_idx = None;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
                }
            }
        }

        match app.should_exit {
            true => break,
            false => {}
        }
    }

    remove_tui_marker_in_dir(&marker_dir, current_pid);
    let exit_session_key = session_key.read().await.clone();
    crate::agent::activity::remove_active_tui_session(&exit_session_key);
    match is_last_live_tui_in_dir(&marker_dir, current_pid) {
        true => {
            let _ = crate::channels::save_default_model_selection(&config, &app.provider, &app.model);
            crate::shutdown::trigger();
            crate::channels::shutdown_gateways_bounded(&config).await;
        }
        false => {
            crate::shutdown::trigger();
        }
    }

    Ok(())
}
