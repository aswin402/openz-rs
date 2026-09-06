//! Session-related WebSocket response builders.

use serde_json::Value;

fn map_summaries_to_json(summaries: Vec<crate::session::SessionSummary>) -> Vec<Value> {
    summaries
        .into_iter()
        .map(|s| {
            let title = s.preview_title(36, &s.key);
            let last_message_at = s.updated_at.timestamp_millis().max(0) as u128;
            let created_at = s.created_at.timestamp_millis().max(0) as u128;
            serde_json::json!({
                "id": s.key,
                "title": title,
                "createdAt": created_at,
                "lastMessageAt": last_message_at,
                "messageCount": s.message_count,
            })
        })
        .collect()
}

pub(crate) async fn fetch_real_sessions_list(
    session_mgr: &crate::session::SessionManager,
) -> Value {
    let summaries = session_mgr.list_summaries_async().await;
    let sessions = map_summaries_to_json(summaries);
    super::super::protocol::sessions_list(sessions)
}

pub(crate) async fn fetch_real_sessions_list_paginated(
    session_mgr: &crate::session::SessionManager,
    offset: usize,
    limit: usize,
) -> Value {
    let (summaries, total) = session_mgr.list_summaries_paginated_async(offset, limit).await;
    let sessions = map_summaries_to_json(summaries);
    super::super::protocol::sessions_list_paginated(sessions, total, offset, limit)
}

pub(crate) fn resolve_session_key(
    session_mgr: &crate::session::SessionManager,
    chat_id: &str,
) -> String {
    if chat_id.contains(':') {
        return chat_id.to_string();
    }
    let path = session_mgr.file_path(chat_id);
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&content) {
                if let Some(key) = value.get("key").and_then(Value::as_str) {
                    return key.to_string();
                }
            }
        }
        return chat_id.to_string();
    }
    if let Some(rest) = chat_id.strip_prefix("cli_") {
        return format!("cli:{}", rest);
    }
    if let Some(rest) = chat_id.strip_prefix("subagent_") {
        return format!("subagent:{}", rest);
    }
    if let Some(rest) = chat_id.strip_prefix("telegram_") {
        return format!("telegram:{}", rest);
    }
    if let Some(rest) = chat_id.strip_prefix("ws_") {
        return format!("ws:{}", rest);
    }
    format!("ws:{}", chat_id)
}

pub(crate) async fn fetch_real_session_history(
    session_mgr: &crate::session::SessionManager,
    chat_id: &str,
) -> Value {
    let mut messages = Vec::new();
    let load_key = resolve_session_key(session_mgr, chat_id);
    if let Ok(session) = session_mgr.load(&load_key) {
        for (index, message) in session.messages.iter().enumerate() {
            let timestamp = message
                .timestamp
                .as_deref()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|datetime| datetime.timestamp_millis())
                .unwrap_or_else(|| (index as i64) * 1000);

            messages.push(serde_json::json!({
                "id": format!("msg-{}-{}", index, timestamp),
                "role": message.role,
                "content": message.content,
                "timestamp": timestamp,
                "extra": message.extra,
            }));
        }
    }
    super::super::protocol::session_history(chat_id, messages)
}

pub(crate) fn requested_session_id(envelope: &Value) -> &str {
    envelope
        .get("chat_id")
        .or_else(|| envelope.get("session_key"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
}

pub(crate) async fn archive_session_event(
    session_mgr: &crate::session::SessionManager,
    envelope: &Value,
) -> Value {
    let requested_chat_id = requested_session_id(envelope);

    if requested_chat_id.is_empty() {
        return super::super::protocol::session_archived(
            "error",
            None,
            None,
            None,
            None,
            Some("archive_session requires chat_id or session_key".to_string()),
        );
    }

    let session_key = resolve_session_key(session_mgr, requested_chat_id);
    match session_mgr.archive_async(&session_key).await {
        Ok(Some(archived)) => super::super::protocol::session_archived(
            "success",
            Some(requested_chat_id.to_string()),
            Some(session_key),
            Some(true),
            Some(archived.path.display().to_string()),
            None,
        ),
        Ok(None) => super::super::protocol::session_archived(
            "success",
            Some(requested_chat_id.to_string()),
            Some(session_key),
            Some(false),
            None,
            None,
        ),
        Err(err) => super::super::protocol::session_archived(
            "error",
            Some(requested_chat_id.to_string()),
            Some(session_key),
            None,
            None,
            Some(format!("Failed to archive session: {err}")),
        ),
    }
}

pub(crate) async fn delete_session_event(
    session_mgr: &crate::session::SessionManager,
    envelope: &Value,
) -> Value {
    let requested_chat_id = requested_session_id(envelope);

    if requested_chat_id.is_empty() {
        return super::super::protocol::session_deleted(
            "error",
            None,
            None,
            None,
            Some("delete_session requires chat_id or session_key".to_string()),
        );
    }

    let session_key = resolve_session_key(session_mgr, requested_chat_id);
    match session_mgr.delete_async(&session_key).await {
        Ok(deleted) => super::super::protocol::session_deleted(
            "success",
            Some(requested_chat_id.to_string()),
            Some(session_key),
            Some(deleted),
            None,
        ),
        Err(err) => super::super::protocol::session_deleted(
            "error",
            Some(requested_chat_id.to_string()),
            Some(session_key),
            None,
            Some(format!("Failed to delete session: {err}")),
        ),
    }
}
