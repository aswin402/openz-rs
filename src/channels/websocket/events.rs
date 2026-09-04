//! WebSocket event delivery and orchestration lifecycle buffering.
//!
//! Connection-local command responses remain in the connection handler. This
//! module owns events that originate outside a single socket, including the
//! best-effort broadcast path and the bounded orchestration broker.

use axum::extract::ws::Message;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Broadcast a JSON event to every connected WebSocket client.
pub fn publish_ws_event(event: Value) {
    if let Ok(senders) = crate::channels::get_active_ws_senders().lock() {
        if let Ok(evt_str) = serde_json::to_string(&event) {
            for sender in senders.values() {
                let _ = sender.try_send(Message::Text(evt_str.clone()));
            }
        }
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
                .filter(|(id, _)| {
                    normalized_chat_id
                        .as_deref()
                        .and_then(|chat_id| {
                            client_chats
                                .get(*id)
                                .map(|client_chat| normalize_ws_chat_id(client_chat) == chat_id)
                        })
                        .unwrap_or(true)
                })
                .map(|(id, sender)| (id.clone(), sender.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn normalize_ws_chat_id(chat_id: &str) -> String {
    if chat_id.contains(':') {
        ws_chat_id(chat_id).unwrap_or_else(|| chat_id.to_string())
    } else if chat_id.starts_with("cli_")
        || chat_id.starts_with("subagent_")
        || chat_id.starts_with("telegram_")
        || chat_id.starts_with("ws_")
    {
        chat_id.to_string()
    } else {
        format!("ws_{chat_id}")
    }
}

pub(crate) fn remove_active_ws_sender(client_id: &str) {
    if let Ok(mut senders) = crate::channels::get_active_ws_senders().lock() {
        senders.remove(client_id);
    }
    if let Ok(mut client_chats) = crate::channels::get_active_ws_client_chats().lock() {
        client_chats.remove(client_id);
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
    let Ok(mut queue) = broker.queue.lock() else {
        return None;
    };

    if queue.len() >= ORCHESTRATION_EVENT_QUEUE_CAPACITY {
        if !is_critical_orchestration_event(&event) {
            return None;
        }

        let event_type = orchestration_event_type(&event);
        let event_run_id = orchestration_event_run_id(&event);

        if let Some(index) = queue
            .iter()
            .position(|queued| !is_critical_orchestration_event(queued))
        {
            queue.remove(index);
        } else if event_type == Some("run_finished") {
            if let Some(index) = queue
                .iter()
                .position(|queued| orchestration_event_run_id(queued) == event_run_id)
            {
                queue[index] = event;
                broker.notify.notify_one();
                return None;
            }

            if let Some(index) = queue
                .iter()
                .position(|queued| orchestration_event_type(queued) == Some("run_started"))
            {
                queue.remove(index);
            } else {
                return Some(event);
            }
        } else if queue
            .iter()
            .any(|queued| orchestration_event_run_id(queued) == event_run_id)
        {
            return None;
        } else {
            return Some(event);
        }
    }

    queue.push_back(event);
    broker.notify.notify_one();
    None
}

async fn publish_ws_event_with_timeout(event: Value) {
    let Ok(evt_str) = serde_json::to_string(&event) else {
        return;
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

                if !matches!(send_result, Ok(Ok(()))) {
                    remove_active_ws_sender(&client_id);
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

    if let Some(broker) = BROKER.get() {
        return Some(broker.clone());
    }

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

/// Publish a lifecycle event without dropping it when a client queue is full.
///
/// Progress broadcasts remain best-effort; workflow lifecycle events are
/// routed through one ordered bounded broker. Under saturation, run-finished
/// events coalesce queued lifecycle state; uncoalescable critical events bypass
/// the broker with timeout delivery; non-critical step progress may be dropped.
fn publish_reliable_ws_event(event: Value) {
    let Some(broker) = orchestration_event_broker() else {
        return;
    };

    if let Some(overflow_event) = enqueue_orchestration_event(&broker, event) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(publish_ws_event_with_timeout(overflow_event));
        }
    }
}

/// Extract the chat id from a session key (mapping colons to underscores to
/// match WebUI format).
pub fn ws_chat_id(session_key: &str) -> Option<String> {
    if session_key.is_empty() {
        return None;
    }
    Some(session_key.replace(':', "_"))
}

/// Publish a structured activity notice to WebUI clients for a chat/session.
pub fn publish_activity_notice(
    session_key: &str,
    kind: &str,
    title: impl Into<String>,
    detail: impl Into<String>,
) {
    if let Some(chat_id) = ws_chat_id(session_key) {
        publish_ws_event(crate::channels::websocket::protocol::activity_notice(
            chat_id,
            kind,
            title,
            detail,
            chrono::Utc::now().timestamp_millis(),
        ));
    }
}

/// Publish a structured orchestration event to WebUI clients for a chat/session.
pub fn publish_orchestration_event(chat_id: &str, payload: Value) {
    publish_reliable_ws_event(crate::channels::websocket::protocol::orchestration_event(
        chat_id,
        payload,
    ));
}
