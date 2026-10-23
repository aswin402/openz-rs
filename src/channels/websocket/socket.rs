//! WebSocket connection lifecycle, frame reception, and turn dispatching.

use super::approvals::{
    cancel_ws_approvals_for_client, with_ws_approval_context, WsApprovalContext,
};
use super::attachments::{persist_attachments, MAX_WS_MESSAGE_SIZE};
use super::auth::{gateway_token_configured, is_authorized, websocket_origin_allowed};
use super::commands;
use super::commands::memory::cognitive_memory_event;
use super::commands::observability::{logs_event, mcp_servers_event};
use super::commands::sessions::{
    archive_session_event, delete_session_event, fetch_real_session_history,
    fetch_real_sessions_list, fetch_real_sessions_list_paginated, resolve_session_key,
};
use super::commands::system::runtime_inventory_event;
use super::events::normalize_ws_chat_id;
use super::protocol;
use super::turns::{
    cancel_ws_turn_for_client_chat, cancel_ws_turns_for_client, register_ws_turn, WsTurnGuard,
};
use super::WsState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub(crate) const MAX_WS_REQUEST_ID_LEN: usize = 128;
const MAX_CONCURRENT_WS_CLIENT_TURNS: usize = 16;

pub(crate) fn ws_request_id(envelope: &Value) -> Option<String> {
    let request_id = envelope.get("request_id").and_then(Value::as_str)?.trim();
    (!request_id.is_empty() && request_id.len() <= MAX_WS_REQUEST_ID_LEN)
        .then(|| request_id.to_string())
}

pub(crate) fn is_known_ws_command(command: &str) -> bool {
    commands::WsCommand::parse(command).is_some()
}

pub(crate) fn command_ack_event(
    request_id: &str,
    command: &str,
    status: &str,
    detail: Option<&str>,
) -> Value {
    protocol::command_ack(request_id, command, status, detail)
}

/// Serialize and enqueue one connection-local event while preserving the
/// gateway's existing best-effort send behavior.
pub(crate) async fn send_event(sender: &mpsc::Sender<Message>, event: Value) {
    match serde_json::to_string(&event) {
        Ok(event_str) => {
            let _ = sender.send(Message::Text(event_str)).await;
        }
        Err(_) => {}
    }
}

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    State(state): State<WsState>,
) -> impl IntoResponse {
    let query_token = params.get("token").map(|s| s.as_str());
    let token_configured = gateway_token_configured();
    let origin_allowed = websocket_origin_allowed(
        headers
            .get(axum::http::header::ORIGIN)
            .and_then(|value| value.to_str().ok()),
        &state.config,
        token_configured,
    );
    match (origin_allowed, is_authorized(&headers, query_token)) {
        (false, _) => (StatusCode::FORBIDDEN, "Untrusted WebSocket origin").into_response(),
        (true, false) => (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
        (true, true) => ws
            .max_message_size(MAX_WS_MESSAGE_SIZE)
            .max_frame_size(MAX_WS_MESSAGE_SIZE)
            .on_upgrade(|socket| handle_socket(socket, state)),
    }
}

pub(crate) async fn handle_socket(socket: WebSocket, state: WsState) {
    let client_id = format!("client-{}", crate::core::short_uuid());
    let default_chat_id = uuid::Uuid::new_v4().to_string();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_WS_CLIENT_TURNS));

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match ws_sender.send(msg).await {
                Ok(()) => {}
                Err(_) => break,
            }
        }
    });

    match crate::channels::get_active_ws_senders().lock() {
        Ok(mut senders) => {
            senders.insert(client_id.clone(), tx.clone());
        }
        Err(_) => {}
    }
    match crate::channels::get_active_ws_client_chats().lock() {
        Ok(mut client_chats) => {
            client_chats.insert(client_id.clone(), default_chat_id.clone());
        }
        Err(_) => {}
    }

    fn unregister_ws_client(client_id: &str) {
        cancel_ws_turns_for_client(client_id);
        cancel_ws_approvals_for_client(client_id);
        match crate::channels::get_active_ws_senders().lock() {
            Ok(mut senders) => {
                senders.remove(client_id);
            }
            Err(_) => {}
        }
        match crate::channels::get_active_ws_client_chats().lock() {
            Ok(mut client_chats) => {
                client_chats.remove(client_id);
            }
            Err(_) => {}
        }
    }

    struct WsSenderGuard(String);
    impl Drop for WsSenderGuard {
        fn drop(&mut self) {
            unregister_ws_client(&self.0);
        }
    }
    let _guard = WsSenderGuard(client_id.clone());

    let ready_evt = protocol::ready(&default_chat_id, &client_id);
    send_event(&tx, ready_evt).await;

    while let Some(Ok(msg)) = ws_receiver.next().await {
        let text = match msg {
            Message::Text(text) => text,
            _ => continue,
        };
        match text.len() > MAX_WS_MESSAGE_SIZE {
            true => continue,
            false => {}
        }
        let envelope = match serde_json::from_str::<Value>(&text) {
            Ok(envelope) => envelope,
            Err(_) => continue,
        };
        let msg_type = envelope.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match ws_request_id(&envelope) {
            Some(request_id) => {
                let (status, detail) = match is_known_ws_command(msg_type) {
                    true => ("accepted", None),
                    false => ("rejected", Some("Unknown WebSocket command.")),
                };
                let ack = command_ack_event(&request_id, msg_type, status, detail);
                send_event(&tx, ack).await;
            }
            None => {}
        }
        let chat_id = envelope
            .get("chat_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&default_chat_id)
            .to_string();
        match msg_type != "security_response" {
            true => match crate::channels::get_active_ws_client_chats().lock() {
                Ok(mut client_chats) => {
                    client_chats.insert(client_id.clone(), chat_id.clone());
                }
                Err(_) => {}
            },
            false => {}
        }

        match msg_type {
            "new_chat" => {
                let new_id = uuid::Uuid::new_v4().to_string();
                let attached_evt = protocol::attached(&new_id);
                send_event(&tx, attached_evt).await;
            }
            "attach" => {
                let attached_evt = protocol::attached(&chat_id);
                send_event(&tx, attached_evt).await;
                let history_evt =
                    fetch_real_session_history(&state.agent_loop.session_manager, &chat_id).await;
                send_event(&tx, history_evt).await;
            }
            "list_sessions" => {
                let offset = envelope
                    .get("offset")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize);
                let limit = envelope
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize);
                let evt = match (offset, limit) {
                    (Some(offset), Some(limit)) => {
                        fetch_real_sessions_list_paginated(
                            &state.agent_loop.session_manager,
                            offset,
                            limit,
                        )
                        .await
                    }
                    _ => fetch_real_sessions_list(&state.agent_loop.session_manager).await,
                };
                send_event(&tx, evt).await;
            }
            "load_history" => {
                let evt =
                    fetch_real_session_history(&state.agent_loop.session_manager, &chat_id).await;
                send_event(&tx, evt).await;
            }
            "archive_session" | "delete_session" => {
                let evt = match msg_type == "archive_session" {
                    true => {
                        archive_session_event(&state.agent_loop.session_manager, &envelope).await
                    }
                    false => {
                        delete_session_event(&state.agent_loop.session_manager, &envelope).await
                    }
                };
                send_event(&tx, evt).await;

                let sessions_evt =
                    fetch_real_sessions_list(&state.agent_loop.session_manager).await;
                send_event(&tx, sessions_evt).await;

                let config = state.current_config();
                let inventory_evt =
                    runtime_inventory_event(&config, Some(&state.agent_loop.tools));
                send_event(&tx, inventory_evt).await;
            }
            "get_cognitive_memory" => {
                let evt = cognitive_memory_event().await;
                send_event(&tx, evt).await;
            }
            "get_mcp_servers" => {
                let live = state.current_config();
                let evt = mcp_servers_event(&live).await;
                send_event(&tx, evt).await;
            }
            "get_logs" => {
                let evt = logs_event().await;
                send_event(&tx, evt).await;
            }
            "message" => {
                handle_message_command(
                    &envelope,
                    &chat_id,
                    &client_id,
                    &state,
                    &tx,
                    &semaphore,
                )
                .await;
            }
            _ => {
                commands::handle_post_message_command(
                    msg_type,
                    &envelope,
                    &chat_id,
                    &client_id,
                    &state,
                    &tx,
                )
                .await;
            }
        }
    }

    unregister_ws_client(&client_id);
}

async fn send_delta_and_turn_end(tx: &mpsc::Sender<Message>, chat_id: &str, response_text: String) {
    let delta_evt = protocol::delta_without_turn(chat_id.to_string(), response_text);
    send_event(tx, delta_evt).await;
    let turn_end_evt = protocol::turn_end(chat_id.to_string(), None);
    send_event(tx, turn_end_evt).await;
}

fn format_background_servers_response() -> String {
    let servers = crate::shutdown::list_registered_children();
    match servers.is_empty() {
        true => "No OpenZ-launched background servers running.".to_string(),
        false => {
            let mut res = "OpenZ background servers:\n".to_string();
            for server in servers {
                res.push_str(&format!(
                    "  #{} pid={} {} - {}\n",
                    server.id, server.pid, server.kind, server.command
                ));
            }
            res.push_str("Use `/stop-server <id>` or `/stop-server all`.");
            res
        }
    }
}

use crate::channels::format_stop_server_response;

fn collect_message_session_overrides(envelope: &Value) -> serde_json::Map<String, Value> {
    let mut overrides = serde_json::Map::new();
    for key in [
        "model",
        "provider",
        "temperature",
        "max_tokens",
        "streaming",
        "caveman_mode",
    ] {
        match envelope.get(key) {
            Some(Value::String(s)) => {
                let trimmed = s.trim();
                match trimmed.is_empty() {
                    true => {}
                    false => {
                        overrides.insert(key.to_string(), Value::String(trimmed.to_string()));
                    }
                }
            }
            Some(Value::Number(n)) => {
                overrides.insert(key.to_string(), Value::Number(n.clone()));
            }
            Some(Value::Bool(b)) => {
                overrides.insert(key.to_string(), Value::Bool(*b));
            }
            _ => {}
        }
    }
    overrides
}

async fn handle_message_command(
    envelope: &Value,
    chat_id: &str,
    client_id: &str,
    state: &WsState,
    tx: &mpsc::Sender<Message>,
    semaphore: &Arc<tokio::sync::Semaphore>,
) {
    let raw_content = envelope
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let attachment_refs =
        persist_attachments(envelope.get("attachments").unwrap_or(&Value::Null)).await;
    let content = match attachment_refs.is_empty() {
        true => raw_content,
        false => format!("{}\n\n{}", attachment_refs.join("\n"), raw_content),
    };

    match crate::channels::is_stop_command(&content) {
        true => {
            let requested_turn_id = envelope
                .get("turn_id")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let normalized_chat_id = normalize_ws_chat_id(chat_id);
            let stopped_turn_id =
                cancel_ws_turn_for_client_chat(client_id, &normalized_chat_id, requested_turn_id);
            let (status, message) = match stopped_turn_id.is_some() {
                true => (
                    "cancelled",
                    "Stop requested. Active OpenZ turn interrupted.",
                ),
                false => ("idle", "No active OpenZ turn for this client and chat."),
            };
            let stopped_evt = protocol::stopped(
                chat_id.to_string(),
                stopped_turn_id,
                status,
                message,
            );
            send_event(tx, stopped_evt).await;
            return;
        }
        false => {}
    }

    match crate::channels::session_command_text_response(
        &state.agent_loop.session_manager,
        chat_id,
        &content,
    )
    .await
    {
        Some(response_text) => {
            send_delta_and_turn_end(tx, chat_id, response_text).await;
            return;
        }
        None => {}
    }

    match crate::channels::model_switch_text_response(&content) {
        Some(response_text) => {
            send_delta_and_turn_end(tx, chat_id, response_text).await;
        }
        None => {}
    }

    let trimmed = content.trim();
    match trimmed == "/servers" {
        true => {
            send_delta_and_turn_end(tx, chat_id, format_background_servers_response()).await;
            return;
        }
        false => {}
    }
    match trimmed.strip_prefix("/stop-server") {
        Some(stripped) => {
            send_delta_and_turn_end(tx, chat_id, format_stop_server_response(stripped.trim()))
                .await;
            return;
        }
        None => {}
    }
    match trimmed.strip_prefix("/device") {
        Some(_) => {
            let response = "Device clipboard and app suggestions are currently managed locally. To audit device details, use the CLI `openz`.".to_string();
            send_delta_and_turn_end(tx, chat_id, response).await;
            return;
        }
        None => {}
    }

    let state_clone = state.clone();
    let tx_clone = tx.clone();
    let chat_id_clone = chat_id.to_string();
    let content_str = content;
    let sem_clone = semaphore.clone();
    let session_overrides = collect_message_session_overrides(envelope);

    tracing::info!(
        "WS message received: chat_id='{}', overrides={:?}",
        chat_id_clone,
        session_overrides
    );

    let turn_id = format!("turn-{}", &uuid::Uuid::new_v4().to_string()[..12]);
    let turn_token = crate::tools::subagent::CancellationToken::new();
    let normalized_chat_id = normalize_ws_chat_id(&chat_id_clone);
    register_ws_turn(
        turn_id.clone(),
        client_id.to_string(),
        normalized_chat_id.clone(),
        turn_token.clone(),
    );
    let turn_started_evt = protocol::turn_started(chat_id_clone.clone(), turn_id.clone());
    send_event(tx, turn_started_evt).await;

    let approval_context = WsApprovalContext {
        client_id: client_id.to_string(),
        chat_id: normalized_chat_id,
    };
    let turn_context = crate::agent::agent_loop::TurnCancellationContext {
        turn_id: turn_id.clone(),
        token: turn_token,
    };
    let turn_id_for_guard = turn_id.clone();
    let turn_id_for_events = turn_id;
    tokio::spawn(async move {
        let _turn_guard = WsTurnGuard(turn_id_for_guard);
        crate::agent::style::spinner::IS_WEBSOCKET
            .scope(true, async move {
                crate::agent::agent_loop::with_turn_cancellation_context(
                    turn_context,
                    async move {
                        with_ws_approval_context(approval_context, async move {
                            let _permit = match sem_clone.try_acquire() {
                                Ok(p) => p,
                                Err(_) => {
                                    let err_evt = protocol::error_for_turn(
                                        chat_id_clone.clone(),
                                        turn_id_for_events.clone(),
                                        "Rate limit exceeded: Too many concurrent turns for this client.",
                                    );
                                    send_event(&tx_clone, err_evt).await;
                                    let turn_end_evt = protocol::turn_end(
                                        chat_id_clone.clone(),
                                        Some(turn_id_for_events.clone()),
                                    );
                                    send_event(&tx_clone, turn_end_evt).await;
                                    return;
                                }
                            };

                            let session_key = resolve_session_key(
                                &state_clone.agent_loop.session_manager,
                                &chat_id_clone,
                            );
                            match session_overrides.is_empty() {
                                true => {}
                                false => {
                                    let _ = crate::agent::agent_loop::save_session_overrides(
                                        &state_clone.agent_loop.session_manager,
                                        &session_key,
                                        &session_overrides,
                                    )
                                    .await;
                                }
                            }
                            let live_config = state_clone.current_config();
                            let workspace =
                                crate::config::loader::workspace_for_agent_turn(&live_config);
                            let run_result = crate::config::loader::ACTIVE_WORKSPACE
                                .scope(workspace, async {
                                    state_clone
                                        .agent_loop
                                        .run(&content_str, &session_key)
                                        .await
                                })
                                .await;

                            match run_result {
                                Ok(res) => {
                                    match res.streamed {
                                        true => {}
                                        false => {
                                            let delta_evt = protocol::delta(
                                                chat_id_clone.clone(),
                                                Some(turn_id_for_events.clone()),
                                                res.content,
                                            );
                                            send_event(&tx_clone, delta_evt).await;
                                        }
                                    }

                                    let turn_end_evt = protocol::turn_end(
                                        chat_id_clone.clone(),
                                        Some(turn_id_for_events.clone()),
                                    );
                                    send_event(&tx_clone, turn_end_evt).await;
                                }
                                Err(e) => {
                                    let err_evt = protocol::error_for_turn(
                                        chat_id_clone.clone(),
                                        turn_id_for_events.clone(),
                                        e.to_string(),
                                    );
                                    send_event(&tx_clone, err_evt).await;
                                    let turn_end_evt = protocol::turn_end(
                                        chat_id_clone.clone(),
                                        Some(turn_id_for_events.clone()),
                                    );
                                    send_event(&tx_clone, turn_end_evt).await;
                                }
                            }
                        })
                        .await
                    },
                )
                .await
            })
            .await;
    });
}

#[cfg(test)]
#[path = "socket_tests.rs"]
mod tests;
