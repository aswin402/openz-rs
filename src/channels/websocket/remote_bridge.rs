//! Gateway heartbeat & remote control bridge for WebUI (`ws:...`) sessions.
//!
//! Keeps `~/.openz/active_gateway.json` fresh with live browser-attached chat IDs
//! and dispatches remote inbox messages (`inbox_ws_*.json`) into the target WebUI
//! session while streaming live events to connected browser tabs and delivering the
//! completed response back to the remote caller (e.g., Telegram `/remote`).

use super::approvals::{with_ws_approval_context, WsApprovalContext};
use super::commands::sessions::{ensure_webui_session_key, fetch_real_sessions_list};
use super::events::{normalize_ws_chat_id, publish_ws_event};
use super::protocol;
use super::turns::{register_ws_turn, WsTurnGuard};
use crate::agent::AgentLoop;
use crate::config::schema::WebSocketChannelConfig;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::Duration;

pub(crate) struct GatewayRemoteBridgeGuard {
    stop_tx: Option<oneshot::Sender<()>>,
    pid: u32,
}

impl Drop for GatewayRemoteBridgeGuard {
    fn drop(&mut self) {
        match self.stop_tx.take() {
            Some(tx) => {
                let _ = tx.send(());
            }
            None => {}
        }
        crate::agent::activity::remove_active_gateway_status(self.pid);
    }
}

fn collect_open_ws_sessions() -> Vec<String> {
    let mut sessions = Vec::new();
    match crate::channels::get_active_ws_client_chats().lock() {
        Ok(chats) => {
            for chat_id in chats.values() {
                let normalized = ensure_webui_session_key(&normalize_ws_chat_id(chat_id));
                match sessions.iter().any(|existing| existing == &normalized) {
                    true => {}
                    false => sessions.push(normalized),
                }
            }
        }
        Err(_) => {}
    }
    sessions
}

fn find_client_viewing_chat(normalized_chat_id: &str) -> String {
    crate::channels::get_active_ws_client_chats()
        .lock()
        .ok()
        .and_then(|chats| {
            chats.iter().find_map(|(client_id, chat_id)| {
                (normalize_ws_chat_id(chat_id) == normalized_chat_id).then(|| client_id.clone())
            })
        })
        .unwrap_or_else(|| "remote-bridge".to_string())
}

fn spawn_remote_ws_turn(
    agent_loop: Arc<AgentLoop>,
    msg: crate::agent::activity::InboxMessage,
) {
    tokio::spawn(async move {
        let session_key = ensure_webui_session_key(&msg.session_id);
        let normalized_chat_id = normalize_ws_chat_id(&session_key);
        let turn_id = format!("turn-{}", &uuid::Uuid::new_v4().to_string()[..12]);
        let turn_token = crate::tools::subagent::CancellationToken::new();
        let turn_token_for_err = turn_token.clone();
        let client_id = find_client_viewing_chat(&normalized_chat_id);

        let user_evt = protocol::remote_user_message(
            normalized_chat_id.clone(),
            msg.sender.clone(),
            msg.message.clone(),
            chrono::Utc::now().timestamp_millis(),
        );
        publish_ws_event(user_evt);

        register_ws_turn(
            turn_id.clone(),
            client_id.clone(),
            normalized_chat_id.clone(),
            turn_token.clone(),
        );
        publish_ws_event(protocol::turn_started(
            normalized_chat_id.clone(),
            turn_id.clone(),
        ));

        let (hb_stop_tx, mut hb_stop_rx) = oneshot::channel::<()>();
        let sender_for_hb = msg.sender.clone();
        tokio::spawn(async move {
            let tg_chat_id = sender_for_hb
                .strip_prefix("telegram:")
                .and_then(|s| s.trim().parse::<i64>().ok());
            let chat_id = match tg_chat_id {
                Some(id) => id,
                None => return,
            };
            loop {
                tokio::select! {
                    biased;
                    _ = &mut hb_stop_rx => break,
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {
                        crate::agent::activity::touch_remote_turn_pending(chat_id);
                        crate::channels::telegram::refresh_remote_typing_indicator(chat_id);
                    }
                }
            }
        });

        let approval_context = WsApprovalContext {
            client_id,
            chat_id: normalized_chat_id.clone(),
        };
        let turn_context = crate::agent::agent_loop::TurnCancellationContext {
            turn_id: turn_id.clone(),
            token: turn_token,
        };
        let _turn_guard = WsTurnGuard(turn_id.clone());

        let live_config = crate::config::loader::load_config()
            .unwrap_or_else(|_| agent_loop.config.clone())
            .with_ui_surface("webui");
        let workspace = crate::config::loader::workspace_for_agent_turn(&live_config);
        let prompt_text = msg.message.clone();
        let session_key_for_run = session_key.clone();
        let agent_for_run = agent_loop.clone();

        let run_result = crate::agent::style::spinner::IS_WEBSOCKET
            .scope(true, async move {
                crate::agent::agent_loop::with_turn_cancellation_context(
                    turn_context,
                    async move {
                        with_ws_approval_context(approval_context, async move {
                            crate::config::loader::ACTIVE_WORKSPACE
                                .scope(workspace, async move {
                                    agent_for_run
                                        .run(&prompt_text, &session_key_for_run)
                                        .await
                                })
                                .await
                        })
                        .await
                    },
                )
                .await
            })
            .await;

        let _ = hb_stop_tx.send(());

        match run_result {
            Ok(res) => {
                match res.streamed {
                    true => {}
                    false => {
                        publish_ws_event(protocol::delta(
                            normalized_chat_id.clone(),
                            Some(turn_id.clone()),
                            res.content.clone(),
                        ));
                    }
                }
                publish_ws_event(protocol::turn_end(
                    normalized_chat_id.clone(),
                    Some(turn_id),
                ));
                let sessions = fetch_real_sessions_list(&agent_loop.session_manager).await;
                publish_ws_event(sessions);
                crate::channels::telegram::messages::deliver_remote_reply(
                    &msg.sender,
                    "WebUI",
                    Ok(&res.content),
                )
                .await;
            }
            Err(err) => {
                let err_str = err.to_string();
                let was_cancelled =
                    turn_token_for_err.is_cancelled() || err_str.contains("Cancelled by user");
                match was_cancelled {
                    true => {}
                    false => {
                        publish_ws_event(protocol::error_for_turn(
                            normalized_chat_id.clone(),
                            turn_id.clone(),
                            err_str.clone(),
                        ));
                    }
                }
                publish_ws_event(protocol::turn_end(
                    normalized_chat_id.clone(),
                    Some(turn_id),
                ));
                crate::channels::telegram::messages::deliver_remote_reply(
                    &msg.sender,
                    "WebUI",
                    Err(&err_str),
                )
                .await;
            }
        }
    });
}

pub(crate) fn spawn_gateway_remote_bridge(
    ws_config: WebSocketChannelConfig,
    agent_loop: Arc<AgentLoop>,
) -> GatewayRemoteBridgeGuard {
    let pid = std::process::id();
    let started_at = chrono::Utc::now().to_rfc3339();
    let initial_cfg = crate::config::loader::load_config()
        .unwrap_or_else(|_| agent_loop.config.clone())
        .with_ui_surface("webui");
    let initial_status = crate::agent::activity::make_active_gateway_status(
        &ws_config.host,
        ws_config.port,
        &started_at,
        &initial_cfg.agents.defaults.model,
        &initial_cfg.agents.defaults.provider,
        collect_open_ws_sessions(),
    );
    let _ = crate::agent::activity::upsert_active_gateway_status(&initial_status);

    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    let fallback_cfg = agent_loop.config.clone();
    tokio::spawn(async move {
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(5));
        let mut inbox_interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                biased;
                _ = &mut stop_rx => {
                    crate::agent::activity::remove_active_gateway_status(pid);
                    break;
                }
                _ = heartbeat_interval.tick() => {
                    let effective = crate::config::loader::load_config()
                        .unwrap_or_else(|_| fallback_cfg.clone())
                        .with_ui_surface("webui");
                    let status = crate::agent::activity::make_active_gateway_status(
                        &ws_config.host,
                        ws_config.port,
                        &started_at,
                        &effective.agents.defaults.model,
                        &effective.agents.defaults.provider,
                        collect_open_ws_sessions(),
                    );
                    let _ = crate::agent::activity::upsert_active_gateway_status(&status);
                }
                _ = inbox_interval.tick() => {
                    while let Some(inbox_msg) = crate::agent::activity::pop_any_ws_inbox_message() {
                        spawn_remote_ws_turn(agent_loop.clone(), inbox_msg);
                    }
                }
            }
        }
    });

    GatewayRemoteBridgeGuard {
        stop_tx: Some(stop_tx),
        pid,
    }
}
