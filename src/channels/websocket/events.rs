//! WebSocket event delivery and orchestration lifecycle buffering.
//!
//! Connection-local command responses remain in the connection handler. This
//! module owns events that originate outside a single socket, including the
//! best-effort broadcast path and the bounded orchestration broker.

use axum::extract::ws::Message;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

const KNOWN_WS_CHAT_PREFIXES: &[&str] = &["cli_", "subagent_", "telegram_", "ws_"];

/// Broadcast a JSON event to every connected WebSocket client.
pub fn publish_ws_event(event: Value) {
    match (
        crate::channels::get_active_ws_senders().lock().ok(),
        serde_json::to_string(&event).ok(),
    ) {
        (Some(senders), Some(evt_str)) => {
            for sender in senders.values() {
                let _ = sender.try_send(Message::Text(evt_str.clone()));
            }
        }
        _ => {}
    }
}

/// Broadcast a JSON event to every connected WebSocket client except the specified client ID.
pub fn publish_ws_event_except(except_client_id: &str, event: Value) {
    match (
        crate::channels::get_active_ws_senders().lock().ok(),
        serde_json::to_string(&event).ok(),
    ) {
        (Some(senders), Some(evt_str)) => {
            for (id, sender) in senders.iter().filter(|(id, _)| id.as_str() != except_client_id) {
                let _ = id;
                let _ = sender.try_send(Message::Text(evt_str.clone()));
            }
        }
        _ => {}
    }
}

pub(crate) fn active_ws_sender_snapshot_for_chat(
    chat_id: Option<&str>,
) -> Vec<(String, mpsc::Sender<Message>)> {
    let normalized_chat_id = chat_id.map(normalize_ws_chat_id);
    let client_chats = crate::channels::get_active_ws_client_chats()
        .lock()
        .map(|chats| chats.clone())
        .unwrap_or_default();

    crate::channels::get_active_ws_senders()
        .lock()
        .map(|senders| {
            senders
                .iter()
                .filter(|(id, _)| match normalized_chat_id.as_deref() {
                    Some(target_chat) => client_chats
                        .get(*id)
                        .map(|client_chat| normalize_ws_chat_id(client_chat) == target_chat)
                        .unwrap_or(false),
                    None => true,
                })
                .map(|(id, sender)| (id.clone(), sender.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn normalize_ws_chat_id(chat_id: &str) -> String {
    match chat_id.contains(':') {
        true => ws_chat_id(chat_id).unwrap_or_else(|| chat_id.to_string()),
        false => match KNOWN_WS_CHAT_PREFIXES
            .iter()
            .any(|prefix| chat_id.starts_with(prefix))
        {
            true => chat_id.to_string(),
            false => format!("ws_{chat_id}"),
        },
    }
}

pub(crate) fn remove_active_ws_sender(client_id: &str) {
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

const ORCHESTRATION_EVENT_SEND_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(500);
const ORCHESTRATION_EVENT_QUEUE_CAPACITY: usize = 1024;

struct OrchestrationEventBroker {
    queue: std::sync::Mutex<std::collections::VecDeque<Value>>,
    notify: tokio::sync::Notify,
}

fn orchestration_event_type(event: &Value) -> Option<&str> {
    event
        .get("payload")
        .and_then(|payload| payload.get("type"))
        .and_then(Value::as_str)
}

fn orchestration_event_run_id(event: &Value) -> Option<&str> {
    event
        .get("payload")
        .and_then(|payload| payload.get("run_id"))
        .and_then(Value::as_str)
}

fn is_critical_orchestration_event(event: &Value) -> bool {
    matches!(
        orchestration_event_type(event),
        Some("run_started" | "run_finished")
    )
}

fn enqueue_orchestration_event(
    broker: &OrchestrationEventBroker,
    event: Value,
) -> Option<Value> {
    let mut queue = match broker.queue.lock() {
        Ok(queue) => queue,
        Err(_) => return None,
    };

    match queue.len() >= ORCHESTRATION_EVENT_QUEUE_CAPACITY {
        false => {
            queue.push_back(event);
            broker.notify.notify_one();
            None
        }
        true => {
            match is_critical_orchestration_event(&event) {
                false => return None,
                true => {}
            }

            let event_type = orchestration_event_type(&event);
            let event_run_id = orchestration_event_run_id(&event);
            let non_critical_index = queue
                .iter()
                .position(|queued| !is_critical_orchestration_event(queued));

            match non_critical_index {
                Some(index) => {
                    queue.remove(index);
                    queue.push_back(event);
                    broker.notify.notify_one();
                    None
                }
                None => match event_type == Some("run_finished") {
                    true => {
                        let matching_run_index = queue
                            .iter()
                            .position(|queued| orchestration_event_run_id(queued) == event_run_id);
                        match matching_run_index {
                            Some(index) => {
                                queue[index] = event;
                                broker.notify.notify_one();
                                None
                            }
                            None => {
                                let started_index = queue.iter().position(|queued| {
                                    orchestration_event_type(queued) == Some("run_started")
                                });
                                match started_index {
                                    Some(index) => {
                                        queue.remove(index);
                                        queue.push_back(event);
                                        broker.notify.notify_one();
                                        None
                                    }
                                    None => Some(event),
                                }
                            }
                        }
                    }
                    false => {
                        let duplicate_run = queue
                            .iter()
                            .any(|queued| orchestration_event_run_id(queued) == event_run_id);
                        match duplicate_run {
                            true => None,
                            false => Some(event),
                        }
                    }
                },
            }
        }
    }
}

async fn publish_ws_event_with_timeout(event: Value) {
    let evt_str = match serde_json::to_string(&event) {
        Ok(s) => s,
        Err(_) => return,
    };

    let chat_id = event.get("chat_id").and_then(Value::as_str);
    let sends = active_ws_sender_snapshot_for_chat(chat_id)
        .into_iter()
        .map(|(client_id, sender)| {
            let evt_str = evt_str.clone();
            async move {
                let send_result = tokio::time::timeout(
                    ORCHESTRATION_EVENT_SEND_TIMEOUT,
                    sender.send(Message::Text(evt_str)),
                )
                .await;

                match send_result {
                    Ok(Ok(())) => {}
                    _ => remove_active_ws_sender(&client_id),
                }
            }
        })
        .collect::<Vec<_>>();

    futures_util::future::join_all(sends).await;
}

async fn run_orchestration_event_broker(broker: Arc<OrchestrationEventBroker>) {
    loop {
        while let Some(event) = broker
            .queue
            .lock()
            .ok()
            .and_then(|mut queue| queue.pop_front())
        {
            publish_ws_event_with_timeout(event).await;
        }

        broker.notify.notified().await;
    }
}

fn orchestration_event_broker() -> Option<Arc<OrchestrationEventBroker>> {
    static BROKER: std::sync::OnceLock<Arc<OrchestrationEventBroker>> = std::sync::OnceLock::new();

    match BROKER.get() {
        Some(broker) => Some(broker.clone()),
        None => {
            let handle = tokio::runtime::Handle::try_current().ok()?;
            Some(
                BROKER
                    .get_or_init(|| {
                        let broker = Arc::new(OrchestrationEventBroker {
                            queue: std::sync::Mutex::new(std::collections::VecDeque::new()),
                            notify: tokio::sync::Notify::new(),
                        });
                        handle.spawn(run_orchestration_event_broker(broker.clone()));
                        broker
                    })
                    .clone(),
            )
        }
    }
}

/// Publish a lifecycle event without dropping it when a client queue is full.
fn publish_reliable_ws_event(event: Value) {
    let broker = match orchestration_event_broker() {
        Some(broker) => broker,
        None => return,
    };

    match (
        enqueue_orchestration_event(&broker, event),
        tokio::runtime::Handle::try_current().ok(),
    ) {
        (Some(overflow_event), Some(handle)) => {
            handle.spawn(publish_ws_event_with_timeout(overflow_event));
        }
        _ => {}
    }
}

/// Extract the chat id from a session key (mapping colons to underscores to
/// match WebUI format).
pub fn ws_chat_id(session_key: &str) -> Option<String> {
    (!session_key.is_empty()).then(|| session_key.replace(':', "_"))
}

/// Publish a structured activity notice to WebUI clients for a chat/session.
pub fn publish_activity_notice(
    session_key: &str,
    kind: &str,
    title: impl Into<String>,
    detail: impl Into<String>,
) {
    match ws_chat_id(session_key) {
        Some(chat_id) => {
            publish_ws_event(crate::channels::websocket::protocol::activity_notice(
                chat_id,
                kind,
                title,
                detail,
                chrono::Utc::now().timestamp_millis(),
            ));
        }
        None => {}
    }
}

/// Publish a structured orchestration event to WebUI clients for a chat/session.
pub fn publish_orchestration_event(chat_id: &str, payload: Value) {
    publish_reliable_ws_event(crate::channels::websocket::protocol::orchestration_event(
        chat_id,
        payload,
    ));
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
