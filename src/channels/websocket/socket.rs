//! WebSocket connection lifecycle, frame reception, and turn dispatching.

use super::approvals::{cancel_ws_approvals_for_client, with_ws_approval_context, WsApprovalContext};
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
use std::sync::Arc;
use tokio::sync::mpsc;

pub(crate) const MAX_WS_REQUEST_ID_LEN: usize = 128;

pub(crate) fn ws_request_id(envelope: &Value) -> Option<String> {
    let request_id = envelope.get("request_id").and_then(Value::as_str)?.trim();
    if request_id.is_empty() || request_id.len() > MAX_WS_REQUEST_ID_LEN {
        return None;
    }
    Some(request_id.to_string())
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
    let Ok(event_str) = serde_json::to_string(&event) else {
        return;
    };
    let _ = sender.send(Message::Text(event_str)).await;
}

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(state): State<WsState>,
) -> impl IntoResponse {
    let query_token = params.get("token").map(|s| s.as_str());
    let token_configured = gateway_token_configured();
    if !websocket_origin_allowed(
        headers.get(axum::http::header::ORIGIN).and_then(|value| value.to_str().ok()),
        &state.config,
        token_configured,
    ) {
        return (StatusCode::FORBIDDEN, "Untrusted WebSocket origin").into_response();
    }
    if !is_authorized(&headers, query_token) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    ws.max_message_size(MAX_WS_MESSAGE_SIZE)
        .max_frame_size(MAX_WS_MESSAGE_SIZE)
        .on_upgrade(|socket| handle_socket(socket, state))
}

pub(crate) async fn handle_socket(socket: WebSocket, state: WsState) {
    let client_id = format!("client-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let default_chat_id = uuid::Uuid::new_v4().to_string();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(1));

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    // Spawn dedicated write loop
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Register sender for the global notification broker
    if let Ok(mut senders) = crate::channels::get_active_ws_senders().lock() {
        senders.insert(client_id.clone(), tx.clone());
    }
    if let Ok(mut client_chats) = crate::channels::get_active_ws_client_chats().lock() {
        client_chats.insert(client_id.clone(), default_chat_id.clone());
    }

    struct WsSenderGuard(String);
    impl Drop for WsSenderGuard {
        fn drop(&mut self) {
            cancel_ws_turns_for_client(&self.0);
            cancel_ws_approvals_for_client(&self.0);
            if let Ok(mut senders) = crate::channels::get_active_ws_senders().lock() {
                senders.remove(&self.0);
            }
            if let Ok(mut client_chats) = crate::channels::get_active_ws_client_chats().lock() {
                client_chats.remove(&self.0);
            }
        }
    }
    let _guard = WsSenderGuard(client_id.clone());

    // Send ready event
    let ready_evt = protocol::ready(&default_chat_id, &client_id);
    if let Ok(ready_str) = serde_json::to_string(&ready_evt) {
        let _ = tx.send(Message::Text(ready_str)).await;
    }

    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            if text.len() > MAX_WS_MESSAGE_SIZE {
                continue;
            }
            let parsed: Result<Value, _> = serde_json::from_str(&text);
            if let Ok(envelope) = parsed {
                let msg_type = envelope.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(request_id) = ws_request_id(&envelope) {
                    let (status, detail) = if is_known_ws_command(msg_type) {
                        ("accepted", None)
                    } else {
                        ("rejected", Some("Unknown WebSocket command."))
                    };
                    let ack = command_ack_event(&request_id, msg_type, status, detail);
                    send_event(&tx, ack).await;
                }
                let chat_id = envelope
                    .get("chat_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&default_chat_id)
                    .to_string();
                if msg_type != "security_response" {
                    if let Ok(mut client_chats) = crate::channels::get_active_ws_client_chats().lock() {
                        client_chats.insert(client_id.clone(), chat_id.clone());
                    }
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
                            fetch_real_session_history(&state.agent_loop.session_manager, &chat_id)
                                .await;
                        send_event(&tx, history_evt).await;
                    }
                    "list_sessions" => {
                        let offset = envelope.get("offset").and_then(Value::as_u64).map(|v| v as usize);
                        let limit = envelope.get("limit").and_then(Value::as_u64).map(|v| v as usize);
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
                            fetch_real_session_history(&state.agent_loop.session_manager, &chat_id)
                                .await;
                        send_event(&tx, evt).await;
                    }
                    "archive_session" | "delete_session" => {
                        let evt = if msg_type == "archive_session" {
                            archive_session_event(&state.agent_loop.session_manager, &envelope)
                                .await
                        } else {
                            delete_session_event(&state.agent_loop.session_manager, &envelope).await
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
                        let content = envelope
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let attachment_refs = persist_attachments(
                            envelope.get("attachments").unwrap_or(&Value::Null),
                        )
                        .await;
                        let content = if attachment_refs.is_empty() {
                            content
                        } else {
                            format!("{}\n\n{}", attachment_refs.join("\n"), content)
                        };

                        if crate::channels::is_stop_command(&content) {
                            let requested_turn_id = envelope
                                .get("turn_id")
                                .and_then(|value| value.as_str())
                                .map(str::trim)
                                .filter(|value| !value.is_empty());
                            let normalized_chat_id = normalize_ws_chat_id(&chat_id);
                            let stopped_turn_id = cancel_ws_turn_for_client_chat(
                                &client_id,
                                &normalized_chat_id,
                                requested_turn_id,
                            );
                            let had_active_turn = stopped_turn_id.is_some();
                            let stopped_evt = protocol::stopped(
                                chat_id.clone(),
                                stopped_turn_id,
                                if had_active_turn { "cancelled" } else { "idle" },
                                if had_active_turn {
                                    "Stop requested. Active OpenZ turn interrupted."
                                } else {
                                    "No active OpenZ turn for this client and chat."
                                },
                            );
                            send_event(&tx, stopped_evt).await;
                            continue;
                        }

                        if let Some(response_text) = crate::channels::session_command_text_response(
                            &state.agent_loop.session_manager,
                            &chat_id,
                            &content,
                        )
                        .await
                        {
                            let delta_evt =
                                protocol::delta_without_turn(chat_id.clone(), response_text);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                            continue;
                        }

                        if let Some(response_text) =
                            crate::channels::model_switch_text_response(&content)
                        {
                            let delta_evt =
                                protocol::delta_without_turn(chat_id.clone(), response_text);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                        }
                        if content.trim() == "/servers" {
                            let servers = crate::shutdown::list_registered_children();
                            let response = if servers.is_empty() {
                                "No OpenZ-launched background servers running.".to_string()
                            } else {
                                let mut res = "OpenZ background servers:\n".to_string();
                                for server in servers {
                                    res.push_str(&format!(
                                        "  #{} pid={} {} - {}\n",
                                        server.id, server.pid, server.kind, server.command
                                    ));
                                }
                                res.push_str("Use `/stop-server <id>` or `/stop-server all`.");
                                res
                            };
                            let delta_evt = protocol::delta_without_turn(chat_id.clone(), response);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                            continue;
                        }
                        if let Some(stripped) = content.trim().strip_prefix("/stop-server") {
                            let target = stripped.trim();
                            let response = if target.is_empty() {
                                "Usage: /stop-server <id|all>".to_string()
                            } else {
                                match crate::shutdown::stop_registered_child(target) {
                                    Ok(0) => "No matching background server found.".to_string(),
                                    Ok(count) => format!("✓ Stopped {count} background server(s)."),
                                    Err(e) => format!("✕ Failed to stop server: {e}"),
                                }
                            };
                            let delta_evt = protocol::delta_without_turn(chat_id.clone(), response);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                            continue;
                        }
                        if let Some(_stripped) = content.trim().strip_prefix("/device") {
                            let response = "Device clipboard and app suggestions are currently managed locally. To audit device details, use the CLI `openz agent`.".to_string();
                            let delta_evt = protocol::delta_without_turn(chat_id.clone(), response);
                            send_event(&tx, delta_evt).await;
                            let turn_end_evt = protocol::turn_end(chat_id.clone(), None);
                            send_event(&tx, turn_end_evt).await;
                            continue;
                        }
                        let state_clone = state.clone();
                        let tx_clone = tx.clone();
                        let chat_id_clone = chat_id.clone();
                        let content_str = content.to_string();
                        let sem_clone = semaphore.clone();
                        let msg_model = envelope
                            .get("model")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let msg_provider = envelope
                            .get("provider")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        tracing::info!(
                            "WS message received: chat_id='{}', msg_model={:?}, msg_provider={:?}",
                            chat_id_clone,
                            msg_model,
                            msg_provider
                        );

                        let turn_id = format!(
                            "turn-{}",
                            &uuid::Uuid::new_v4().to_string()[..12],
                        );
                        let turn_token = crate::tools::subagent::CancellationToken::new();
                        let normalized_chat_id = normalize_ws_chat_id(&chat_id_clone);
                        register_ws_turn(
                            turn_id.clone(),
                            client_id.clone(),
                            normalized_chat_id.clone(),
                            turn_token.clone(),
                        );
                        let turn_started_evt = protocol::turn_started(
                            chat_id_clone.clone(),
                            turn_id.clone(),
                        );
                        send_event(&tx, turn_started_evt).await;

                        let approval_context = WsApprovalContext {
                            client_id: client_id.clone(),
                            chat_id: normalized_chat_id,
                        };
                        let turn_context = crate::agent::agent_loop::TurnCancellationContext {
                            turn_id: turn_id.clone(),
                            token: turn_token,
                        };
                        let turn_id_for_guard = turn_id.clone();
                        let turn_id_for_events = turn_id.clone();
                        tokio::spawn(async move {
                            let _turn_guard = WsTurnGuard(turn_id_for_guard);
                            crate::agent::style::spinner::IS_WEBSOCKET.scope(true, async move {
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
                                                        "Rate limit exceeded: Only one message can be processed at a time.",
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

                                            let mut config = state_clone.current_config();
                                            // Apply model/provider overrides from the message payload
                                            if let Some(model) = msg_model {
                                                config.agents.defaults.model = model;
                                            }
                                            if let Some(provider) = msg_provider {
                                                config.agents.defaults.provider = provider;
                                            }
                                            let agent_loop = match crate::cli::build_agent_loop(config).await {
                                                Ok(al) => al,
                                                Err(e) => {
                                                    let err_evt = protocol::error_for_turn(
                                                        chat_id_clone.clone(),
                                                        turn_id_for_events.clone(),
                                                        format!("Failed to build agent loop: {}", e),
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

                                            let session_key = resolve_session_key(&agent_loop.session_manager, &chat_id_clone);
                                            let workspace = crate::config::loader::workspace_for_agent_turn(&agent_loop.config);
                                            let run_result = crate::config::loader::ACTIVE_WORKSPACE
                                                .scope(workspace, async { agent_loop.run(&content_str, &session_key).await })
                                                .await;

                                            match run_result {
                                                Ok(res) => {
                                                    if !res.streamed {
                                                        let delta_evt = protocol::delta(
                                                            chat_id_clone.clone(),
                                                            Some(turn_id_for_events.clone()),
                                                            res.content,
                                                        );
                                                        send_event(&tx_clone, delta_evt).await;
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
        }
    }

    // Clean up sender on disconnect
    if let Ok(mut senders) = crate::channels::get_active_ws_senders().lock() {
        senders.remove(&client_id);
    }
}
