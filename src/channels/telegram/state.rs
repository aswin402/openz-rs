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
    if let Ok(mut guard) = map.lock() {
        guard.insert(req_id.to_string(), tx);
    }
}

pub fn unregister_approval(req_id: &str) {
    let map = APPROVAL_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        guard.remove(req_id);
    }
}

pub fn start_typing_indicator(chat_id: i64, token: String, client: Client) {
    let map = ACTIVE_TYPING_LOOPS.get_or_init(|| Mutex::new(HashMap::new()));
    let (tx, rx) = oneshot::channel::<()>();

    let mut got_inserted = false;
    if let Ok(mut guard) = map.lock() {
        if let Some(old_tx) = guard.remove(&chat_id) {
            let _ = old_tx.send(());
        }
        guard.insert(chat_id, tx);
        got_inserted = true;
    }
    REMOTE_TYPING_HEARTBEATS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .map(|mut guard| guard.insert(chat_id, Instant::now()));

    if got_inserted {
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
}

pub fn stop_typing_indicator(chat_id: i64) {
    let map = ACTIVE_TYPING_LOOPS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        if let Some(tx) = guard.remove(&chat_id) {
            let _ = tx.send(());
        }
    }
    if let Some(heartbeats) = REMOTE_TYPING_HEARTBEATS.get() {
        if let Ok(mut guard) = heartbeats.lock() {
            guard.remove(&chat_id);
        }
    }
}

pub fn refresh_remote_typing_indicator(chat_id: i64) {
    if let Some(heartbeats) = REMOTE_TYPING_HEARTBEATS.get() {
        if let Ok(mut guard) = heartbeats.lock() {
            if guard.contains_key(&chat_id) {
                guard.insert(chat_id, Instant::now());
            }
        }
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
            if !typing_indicator_active(chat_id) {
                return;
            }

            let stale = REMOTE_TYPING_HEARTBEATS
                .get()
                .and_then(|heartbeats| heartbeats.lock().ok())
                .and_then(|guard| guard.get(&chat_id).copied())
                .map(|last_seen| last_seen.elapsed() >= timeout)
                .unwrap_or(true);
            if !stale {
                continue;
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
    if let Ok(mut guard) = map.lock() {
        guard.insert(chat_id, session_key);
    }
}

pub(crate) fn clear_remote_session(chat_id: i64) {
    let map = REMOTE_CONTROL_TARGETS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        guard.remove(&chat_id);
    }
}

pub(crate) fn remote_session_button_label(session: &crate::agent::activity::ActiveTuiSession) -> String {
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
    let preview = if session.preview.trim().is_empty() {
        "new session".to_string()
    } else {
        session.preview.clone()
    };
    format!("{} | {} | {}", cwd_name, started, preview)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_session_selection_round_trips() {
        let chat_id = 4242;
        clear_remote_session(chat_id);
        assert_eq!(selected_remote_session(chat_id), None);

        set_remote_session(chat_id, "cli:test-session".to_string());
        assert_eq!(
            selected_remote_session(chat_id).as_deref(),
            Some("cli:test-session")
        );

        clear_remote_session(chat_id);
        assert_eq!(selected_remote_session(chat_id), None);
    }

    #[test]
    fn remote_session_button_label_contains_context() {
        let session = crate::agent::activity::ActiveTuiSession {
            session_key: "cli:test".to_string(),
            pid: std::process::id(),
            cwd: "/tmp/openz-client-work".to_string(),
            started_at: "2026-07-16T08:30:00Z".to_string(),
            last_seen_at: "2026-07-16T08:31:00Z".to_string(),
            model: "model".to_string(),
            provider: "provider".to_string(),
            preview: "plan client workflow".to_string(),
        };

        let label = remote_session_button_label(&session);
        assert!(label.contains("openz-client-work"));
        assert!(label.contains("plan client workflow"));
    }
}
