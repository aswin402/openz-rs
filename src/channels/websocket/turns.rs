use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
pub(crate) struct ActiveWsTurn {
    pub(crate) client_id: String,
    pub(crate) chat_id: String,
    pub(crate) token: crate::tools::subagent::CancellationToken,
}

pub(crate) struct WsTurnGuard(pub(crate) String);

impl Drop for WsTurnGuard {
    fn drop(&mut self) {
        remove_ws_turn(&self.0);
    }
}

static ACTIVE_WS_TURNS: OnceLock<Mutex<HashMap<String, ActiveWsTurn>>> = OnceLock::new();

pub fn register_ws_turn(
    turn_id: String,
    client_id: String,
    chat_id: String,
    token: crate::tools::subagent::CancellationToken,
) {
    let map = ACTIVE_WS_TURNS.get_or_init(Mutex::default);
    if let Ok(mut turns) = map.lock() {
        turns.insert(
            turn_id,
            ActiveWsTurn {
                client_id,
                chat_id,
                token,
            },
        );
    }
}

pub fn remove_ws_turn(turn_id: &str) {
    let map = ACTIVE_WS_TURNS.get_or_init(Mutex::default);
    if let Ok(mut turns) = map.lock() {
        turns.remove(turn_id);
    }
}

pub fn cancel_ws_turn(turn_id: &str, client_id: &str, chat_id: &str) -> bool {
    let map = ACTIVE_WS_TURNS.get_or_init(Mutex::default);
    let pending = map.lock().ok().and_then(|mut turns| {
        let matches = turns.get(turn_id).is_some_and(|turn| {
            turn.client_id == client_id && turn.chat_id == chat_id
        });
        if matches {
            turns.remove(turn_id)
        } else {
            None
        }
    });

    pending
        .map(|turn| {
            turn.token.cancel();
            true
        })
        .unwrap_or(false)
}

pub fn cancel_ws_turn_for_client_chat(
    client_id: &str,
    chat_id: &str,
    requested_turn_id: Option<&str>,
) -> Option<String> {
    let map = ACTIVE_WS_TURNS.get_or_init(Mutex::default);
    let pending = map.lock().ok().and_then(|mut turns| {
        let turn_id = turns
            .iter()
            .find(|(turn_id, turn)| {
                turn.client_id == client_id
                    && turn.chat_id == chat_id
                    && requested_turn_id
                        .map(|requested| requested == turn_id.as_str())
                        .unwrap_or(true)
            })
            .map(|(turn_id, _)| turn_id.clone());
        turn_id.and_then(|turn_id| turns.remove(&turn_id).map(|turn| (turn_id, turn)))
    });

    pending.map(|(turn_id, turn)| {
        turn.token.cancel();
        turn_id
    })
}

pub fn cancel_ws_turns_for_client(client_id: &str) {
    let map = ACTIVE_WS_TURNS.get_or_init(Mutex::default);
    let pending = map
        .lock()
        .ok()
        .map(|mut turns| {
            let turn_ids = turns
                .iter()
                .filter(|(_, turn)| turn.client_id == client_id)
                .map(|(turn_id, _)| turn_id.clone())
                .collect::<Vec<_>>();
            turn_ids
                .into_iter()
                .filter_map(|turn_id| turns.remove(&turn_id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for turn in pending {
        turn.token.cancel();
    }
}
