use crate::channels::notifications::telegram_api_url;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tokio::sync::oneshot;
use tokio::time::Duration;

static TELEGRAM_BOT_INFO: OnceLock<(String, Client)> = OnceLock::new();
pub(crate) static APPROVAL_CALLBACKS: OnceLock<Mutex<HashMap<String, oneshot::Sender<bool>>>> =
    OnceLock::new();
static REMOTE_CONTROL_TARGETS: OnceLock<Mutex<HashMap<i64, String>>> = OnceLock::new();
static ACTIVE_TYPING_LOOPS: OnceLock<Mutex<HashMap<i64, oneshot::Sender<()>>>> = OnceLock::new();
static REMOTE_TYPING_HEARTBEATS: OnceLock<Mutex<HashMap<i64, Instant>>> = OnceLock::new();

pub(crate) fn set_telegram_bot_info(token: String, client: Client) {
    let _ = TELEGRAM_BOT_INFO.set((token, client));
}

pub fn get_telegram_bot_info() -> Option<(String, Client)> {
    TELEGRAM_BOT_INFO.get().cloned()
}

pub fn register_approval(req_id: &str, tx: oneshot::Sender<bool>) {
    let map = APPROVAL_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
    match map.lock() {
        Ok(mut guard) => {
            guard.insert(req_id.to_string(), tx);
        }
        Err(_) => {}
    }
}

pub fn unregister_approval(req_id: &str) {
    let map = APPROVAL_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
    match map.lock() {
        Ok(mut guard) => {
            guard.remove(req_id);
        }
        Err(_) => {}
    }
}

pub fn start_typing_indicator(chat_id: i64, token: String, client: Client) {
    let map = ACTIVE_TYPING_LOOPS.get_or_init(|| Mutex::new(HashMap::new()));
    let (tx, rx) = oneshot::channel::<()>();

    let got_inserted = match map.lock() {
        Ok(mut guard) => {
            match guard.remove(&chat_id) {
                Some(old_tx) => {
                    let _ = old_tx.send(());
                }
                None => {}
            }
            guard.insert(chat_id, tx);
            true
        }
        Err(_) => false,
    };
    REMOTE_TYPING_HEARTBEATS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .map(|mut guard| guard.insert(chat_id, Instant::now()));

    match got_inserted {
        true => {
            tokio::spawn(async move {
                let send_action_url = telegram_api_url(&token, "sendChatAction");
                let payload = serde_json::json!({
                    "chat_id": chat_id,
                    "action": "typing"
                });
                let _ = client.post(&send_action_url).json(&payload).send().await;

                let mut rx = rx;
                loop {
                    tokio::select! {
                        biased;
                        _ = &mut rx => {
                            break;
                        }
                        _ = tokio::time::sleep(Duration::from_secs(4)) => {
                            let _ = client.post(&send_action_url).json(&payload).send().await;
                        }
                    }
                }
            });
        }
        false => {}
    }
}

pub fn stop_typing_indicator(chat_id: i64) {
    let map = ACTIVE_TYPING_LOOPS.get_or_init(|| Mutex::new(HashMap::new()));
    match map.lock() {
        Ok(mut guard) => match guard.remove(&chat_id) {
            Some(tx) => {
                let _ = tx.send(());
            }
            None => {}
        },
        Err(_) => {}
    }
    match REMOTE_TYPING_HEARTBEATS.get().and_then(|h| h.lock().ok()) {
        Some(mut guard) => {
            guard.remove(&chat_id);
        }
        None => {}
    }
}

pub fn refresh_remote_typing_indicator(chat_id: i64) {
    match REMOTE_TYPING_HEARTBEATS.get().and_then(|h| h.lock().ok()) {
        Some(mut guard) => match guard.contains_key(&chat_id) {
            true => {
                guard.insert(chat_id, Instant::now());
            }
            false => {}
        },
        None => {}
    }
}

pub(crate) fn typing_indicator_active(chat_id: i64) -> bool {
    ACTIVE_TYPING_LOOPS
        .get()
        .and_then(|map| map.lock().ok())
        .map(|guard| guard.contains_key(&chat_id))
        .unwrap_or(false)
}

pub(crate) fn spawn_remote_typing_watchdog(chat_id: i64, token: String, client: Client) {
    tokio::spawn(async move {
        let timeout = Duration::from_secs(super::messages::remote_timeout_secs());
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            match typing_indicator_active(chat_id) {
                false => return,
                true => {}
            }

            let stale = REMOTE_TYPING_HEARTBEATS
                .get()
                .and_then(|heartbeats| heartbeats.lock().ok())
                .and_then(|guard| guard.get(&chat_id).copied())
                .map(|last_seen| last_seen.elapsed() >= timeout)
                .unwrap_or(true);
            match stale {
                false => continue,
                true => {}
            }

            stop_typing_indicator(chat_id);
            let send_url = telegram_api_url(&token, "sendMessage");
            let payload = serde_json::json!({
                "chat_id": chat_id,
                "text": "Remote TUI request timed out. The selected TUI session may have stopped. Use /remote to select an active session again."
            });
            let _ = client.post(&send_url).json(&payload).send().await;
            return;
        }
    });
}

pub(crate) fn selected_remote_session(chat_id: i64) -> Option<String> {
    let map = REMOTE_CONTROL_TARGETS.get_or_init(|| Mutex::new(HashMap::new()));
    map.lock()
        .ok()
        .and_then(|guard| guard.get(&chat_id).cloned())
}

pub(crate) fn set_remote_session(chat_id: i64, session_key: String) {
    let map = REMOTE_CONTROL_TARGETS.get_or_init(|| Mutex::new(HashMap::new()));
    match map.lock() {
        Ok(mut guard) => {
            guard.insert(chat_id, session_key);
        }
        Err(_) => {}
    }
}

pub(crate) fn clear_remote_session(chat_id: i64) {
    let map = REMOTE_CONTROL_TARGETS.get_or_init(|| Mutex::new(HashMap::new()));
    match map.lock() {
        Ok(mut guard) => {
            guard.remove(&chat_id);
        }
        Err(_) => {}
    }
}

pub(crate) fn remote_session_button_label(
    session: &crate::agent::activity::ActiveTuiSession,
) -> String {
    let cwd_name = std::path::Path::new(&session.cwd)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&session.cwd);
    let started = chrono::DateTime::parse_from_rfc3339(&session.started_at)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%b %d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| "unknown time".to_string());
    let preview = match session.preview.trim().is_empty() {
        true => "new session".to_string(),
        false => session.preview.clone(),
    };
    format!("{} | {} | {}", cwd_name, started, preview)
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
