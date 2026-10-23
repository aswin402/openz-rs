//! Typed server events shared by the WebSocket gateway and agent loop.
//!
//! The transport still accepts `serde_json::Value` so existing routing and
//! delivery code remains untouched. These constructors make the event shape
//! explicit at the producer boundary while preserving the current JSON names.

use serde::Serialize;
use serde_json::{json, Value};

#[inline]
fn to_value_safe<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|err| {
        tracing::error!("WebSocket payload failed to serialize: {err}");
        json!({
            "event": "error",
            "error": format!("Serialization error: {err}")
        })
    })
}

#[derive(Debug, Serialize)]
pub(crate) struct WsModelProvider {
    name: String,
    display: String,
    models: Vec<String>,
    available: bool,
    full: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct WsSlashCommand {
    cmd: String,
    desc: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct WsManagedServer {
    id: u64,
    pid: u32,
    kind: String,
    command: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct WsChannelStatus {
    name: String,
    enabled: bool,
    status: &'static str,
    token_configured: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct WsMcpServer {
    name: String,
    command: String,
    status: String,
    enabled: bool,
    args: Vec<String>,
    #[serde(rename = "toolsCount")]
    tools_count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct WsLogEntry {
    id: String,
    timestamp: String,
    level: String,
    target: String,
    message: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn capabilities(
    providers: Vec<Value>,
    security_modes: Vec<Value>,
    channels: Vec<Value>,
    firefox_webdriver_port: u16,
    firefox_attach_port: u16,
    max_count: usize,
    max_file_bytes: usize,
    max_total_bytes: usize,
    max_message_bytes: usize,
    ttl_seconds: u64,
    allowed_mime_types: &'static [&'static str],
) -> Value {
    json!({
        "version": 1,
        "providers": providers,
        "securityModes": security_modes,
        "channels": channels,
        "browser": {
            "firefoxWebdriverPort": firefox_webdriver_port,
            "firefoxAttachPort": firefox_attach_port,
        },
        "attachments": {
            "maxCount": max_count,
            "maxFileBytes": max_file_bytes,
            "maxTotalBytes": max_total_bytes,
            "maxMessageBytes": max_message_bytes,
            "ttlSeconds": ttl_seconds,
            "allowedMimeTypes": allowed_mime_types,
        },
    })
}

pub(crate) fn runtime_inventory(inventory: impl Serialize) -> Value {
    json!({
        "event": "runtime_inventory",
        "inventory": to_value_safe(inventory),
    })
}

pub(crate) fn sessions_list(sessions: Vec<Value>) -> Value {
    json!({
        "event": "sessions_list",
        "sessions": sessions,
    })
}

pub(crate) fn sessions_list_paginated(
    sessions: Vec<Value>,
    total: usize,
    offset: usize,
    limit: usize,
) -> Value {
    json!({
        "event": "sessions_list",
        "sessions": sessions,
        "total": total,
        "offset": offset,
        "limit": limit,
    })
}

pub(crate) fn session_history(chat_id: impl Into<String>, messages: Vec<Value>) -> Value {
    json!({
        "event": "session_history",
        "chat_id": chat_id.into(),
        "messages": messages,
    })
}

pub(crate) fn ready(chat_id: impl Into<String>, client_id: impl Into<String>) -> Value {
    json!({
        "event": "ready",
        "chat_id": chat_id.into(),
        "client_id": client_id.into(),
    })
}

pub(crate) fn attached(chat_id: impl Into<String>) -> Value {
    json!({
        "event": "attached",
        "chat_id": chat_id.into(),
    })
}

pub(crate) fn activity_notice(
    chat_id: impl Into<String>,
    kind: impl Into<String>,
    title: impl Into<String>,
    detail: impl Into<String>,
    timestamp: i64,
) -> Value {
    json!({
        "event": "activity_notice",
        "chat_id": chat_id.into(),
        "kind": kind.into(),
        "title": title.into(),
        "detail": detail.into(),
        "timestamp": timestamp,
    })
}

pub(crate) fn orchestration_event(chat_id: impl Into<String>, payload: Value) -> Value {
    let run_id = payload
        .get("run_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    json!({
        "event": "orchestration_event",
        "chat_id": chat_id.into(),
        "run_id": run_id,
        "payload": payload,
    })
}

pub(crate) fn model_provider(
    name: impl Into<String>,
    display: impl Into<String>,
    models: Vec<String>,
    available: bool,
    full: bool,
) -> WsModelProvider {
    WsModelProvider {
        name: name.into(),
        display: display.into(),
        models,
        available,
        full,
    }
}

pub(crate) fn models_list(
    providers: Vec<WsModelProvider>,
    partial: bool,
    prefs: crate::channels::ModelPrefs,
    active_provider: impl Into<String>,
    active_model: impl Into<String>,
) -> Value {
    json!({
        "event": "models_list",
        "providers": to_value_safe(providers),
        "partial": partial,
        "recent_models": to_value_safe(prefs.recent),
        "favorite_models": to_value_safe(prefs.favorites),
        "active_provider": active_provider.into(),
        "active_model": active_model.into(),
    })
}

pub(crate) fn model_prefs(prefs: crate::channels::ModelPrefs) -> Value {
    json!({
        "event": "model_prefs",
        "recent_models": to_value_safe(prefs.recent),
        "favorite_models": to_value_safe(prefs.favorites),
    })
}

pub(crate) fn slash_command(cmd: impl Into<String>, desc: impl Into<String>) -> WsSlashCommand {
    WsSlashCommand {
        cmd: cmd.into(),
        desc: desc.into(),
    }
}

pub(crate) fn slash_commands(commands: Vec<WsSlashCommand>) -> Value {
    json!({
        "event": "slash_commands",
        "commands": to_value_safe(commands),
    })
}

pub(crate) fn gateway_status(loaded: u32, failed: u32, total: u32) -> Value {
    json!({
        "event": "status",
        "version": env!("CARGO_PKG_VERSION"),
        "mcp": {
            "loaded": loaded,
            "failed": failed,
            "total": total,
        },
    })
}

pub(crate) fn managed_server(
    id: u64,
    pid: u32,
    kind: impl Into<String>,
    command: impl Into<String>,
) -> WsManagedServer {
    WsManagedServer {
        id,
        pid,
        kind: kind.into(),
        command: command.into(),
    }
}

pub(crate) fn channel_status(
    name: impl Into<String>,
    enabled: bool,
    token_configured: bool,
) -> WsChannelStatus {
    let status = match enabled {
        true => "configured",
        false => "disabled",
    };
    WsChannelStatus {
        name: name.into(),
        enabled,
        status,
        token_configured,
    }
}

pub(crate) fn servers_list(
    servers: Vec<WsManagedServer>,
    channels: Vec<WsChannelStatus>,
) -> Value {
    json!({
        "event": "servers_list",
        "servers": to_value_safe(servers),
        "channels": to_value_safe(channels),
    })
}

pub(crate) fn server_stopped(target: impl Into<String>, result: impl Into<String>) -> Value {
    json!({
        "event": "server_stopped",
        "target": target.into(),
        "result": result.into(),
    })
}

#[allow(clippy::too_many_arguments)]
fn session_lifecycle_event(
    event: &'static str,
    status: &'static str,
    chat_id: Option<String>,
    session_key: Option<String>,
    flag_key: &'static str,
    flag_val: Option<bool>,
    archive_path: Option<String>,
    detail: Option<String>,
) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("event".to_string(), Value::String(event.to_string()));
    map.insert("status".to_string(), Value::String(status.to_string()));
    let _ = chat_id.map(|v| map.insert("chat_id".to_string(), Value::String(v)));
    let _ = session_key.map(|v| map.insert("session_key".to_string(), Value::String(v)));
    let _ = flag_val.map(|v| map.insert(flag_key.to_string(), Value::Bool(v)));
    let _ = archive_path.map(|v| map.insert("archive_path".to_string(), Value::String(v)));
    let _ = detail.map(|v| map.insert("detail".to_string(), Value::String(v)));
    Value::Object(map)
}

pub(crate) fn session_archived(
    status: &'static str,
    chat_id: Option<String>,
    session_key: Option<String>,
    archived: Option<bool>,
    archive_path: Option<String>,
    detail: Option<String>,
) -> Value {
    session_lifecycle_event(
        "session_archived",
        status,
        chat_id,
        session_key,
        "archived",
        archived,
        archive_path,
        detail,
    )
}

pub(crate) fn session_deleted(
    status: &'static str,
    chat_id: Option<String>,
    session_key: Option<String>,
    deleted: Option<bool>,
    detail: Option<String>,
) -> Value {
    session_lifecycle_event(
        "session_deleted",
        status,
        chat_id,
        session_key,
        "deleted",
        deleted,
        None,
        detail,
    )
}

fn named_collection_updated_event<T: Serialize>(
    event: &'static str,
    collection_key: &'static str,
    items: Vec<T>,
    status: &'static str,
    name: Option<String>,
) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("event".to_string(), Value::String(event.to_string()));
    map.insert(collection_key.to_string(), to_value_safe(items));
    map.insert("status".to_string(), Value::String(status.to_string()));
    let _ = name.map(|n| map.insert("name".to_string(), Value::String(n)));
    Value::Object(map)
}

pub(crate) fn skills_updated<T: Serialize>(
    skills: Vec<T>,
    status: &'static str,
    name: Option<String>,
) -> Value {
    named_collection_updated_event("skills_updated", "skills", skills, status, name)
}

pub(crate) fn subagents_updated(
    subagents: Vec<Value>,
    status: &'static str,
    name: Option<String>,
) -> Value {
    named_collection_updated_event("subagents_updated", "subagents", subagents, status, name)
}

pub(crate) fn mcp_server(
    name: impl Into<String>,
    command: impl Into<String>,
    status: impl Into<String>,
    enabled: bool,
    args: Vec<String>,
    tools_count: usize,
) -> WsMcpServer {
    WsMcpServer {
        name: name.into(),
        command: command.into(),
        status: status.into(),
        enabled,
        args,
        tools_count,
    }
}

pub(crate) fn mcp_servers(
    servers: Vec<WsMcpServer>,
    loaded: u32,
    failed: u32,
    total: u32,
) -> Value {
    json!({
        "event": "mcp_servers",
        "servers": to_value_safe(servers),
        "stats": {
            "loaded": loaded,
            "failed": failed,
            "total": total,
        },
    })
}

pub(crate) fn log_entry(
    id: impl Into<String>,
    timestamp: impl Into<String>,
    level: impl Into<String>,
    target: impl Into<String>,
    message: impl Into<String>,
) -> WsLogEntry {
    WsLogEntry {
        id: id.into(),
        timestamp: timestamp.into(),
        level: level.into(),
        target: target.into(),
        message: message.into(),
    }
}

pub(crate) fn logs_data(logs: Vec<WsLogEntry>) -> Value {
    json!({
        "event": "logs_data",
        "logs": to_value_safe(logs),
    })
}

pub(crate) fn cron_jobs_updated(
    status: impl Into<String>,
    id: impl Into<String>,
    result: Value,
    inventory: crate::core::inventory::RuntimeInventory,
) -> Value {
    json!({
        "event": "cron_jobs_updated",
        "status": status.into(),
        "id": id.into(),
        "result": result,
        "inventory": to_value_safe(inventory),
    })
}

pub(crate) fn cron_logs(
    id: Option<String>,
    runs: Vec<crate::cron::CronRunRecord>,
) -> Value {
    json!({
        "event": "cron_logs",
        "id": id,
        "runs": to_value_safe(runs),
    })
}

pub(crate) fn config_update_rejected(
    reason: impl Into<String>,
    requires_gateway_token: bool,
) -> Value {
    json!({
        "event": "config_update_rejected",
        "reason": reason.into(),
        "requires_gateway_token": requires_gateway_token,
    })
}

pub(crate) fn config_data(
    defaults: Value,
    skills: Vec<crate::agent::skills::SkillView>,
    mcp_servers: Vec<Value>,
    subagents: Vec<Value>,
    providers: serde_json::Map<String, Value>,
    channels: Value,
    capabilities: Value,
) -> Value {
    json!({
        "event": "config_data",
        "defaults": defaults,
        "skills": to_value_safe(skills),
        "mcp_servers": mcp_servers,
        "subagents": subagents,
        "providers": providers,
        "channels": channels,
        "capabilities": capabilities,
        "version": env!("CARGO_PKG_VERSION"),
    })
}

pub(crate) fn config_updated(defaults: Value, capabilities: Value) -> Value {
    json!({
        "event": "config_updated",
        "defaults": defaults,
        "capabilities": capabilities,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cognitive_memory(
    entities_count: i64,
    relations_count: i64,
    facts_count: i64,
    working_memory_keys: Vec<String>,
    memory_db: impl Into<String>,
    graph_db: impl Into<String>,
    nodes: Vec<Value>,
    edges: Vec<Value>,
    facts: Vec<Value>,
) -> Value {
    json!({
        "event": "cognitive_memory",
        "stats": {
            "entitiesCount": entities_count,
            "relationsCount": relations_count,
            "factsCount": facts_count,
            "workingMemoryKeys": working_memory_keys,
        },
        "paths": {
            "memoryDb": memory_db.into(),
            "graphDb": graph_db.into(),
        },
        "nodes": nodes,
        "edges": edges,
        "facts": facts,
    })
}

pub(crate) fn notification(message: impl Into<String>) -> Value {
    json!({
        "event": "notification",
        "message": message.into(),
    })
}

#[derive(Debug, Serialize)]
#[serde(tag = "event")]
pub(crate) enum WsEvent {
    #[serde(rename = "delta")]
    Delta {
        chat_id: String,
        turn_id: Option<String>,
        content: String,
    },
    #[serde(rename = "delta")]
    DeltaWithoutTurn { chat_id: String, content: String },
    #[serde(rename = "reasoning_delta")]
    ReasoningDelta {
        chat_id: String,
        turn_id: Option<String>,
        content: String,
    },
    #[serde(rename = "tool_start")]
    ToolStart {
        chat_id: String,
        turn_id: Option<String>,
        tool_call_id: String,
        name: String,
        args: Value,
        status: &'static str,
    },
    #[serde(rename = "tool_end")]
    ToolEnd {
        chat_id: String,
        turn_id: Option<String>,
        tool_call_id: String,
        name: String,
        status: String,
        output: String,
    },
    #[serde(rename = "tool_progress")]
    ToolProgress {
        chat_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
        #[serde(skip_serializing_if = "String::is_empty")]
        tool_call_id: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        name: String,
        message: String,
    },
    #[serde(rename = "turn_started")]
    TurnStarted { chat_id: String, turn_id: String },
    #[serde(rename = "turn_end")]
    TurnEnd {
        chat_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        chat_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
        detail: String,
    },
    #[serde(rename = "stopped")]
    Stopped {
        chat_id: String,
        turn_id: Option<String>,
        status: String,
        detail: String,
    },
    #[serde(rename = "security_request")]
    SecurityRequest {
        chat_id: String,
        turn_id: Option<String>,
        req_id: String,
        tool_name: String,
        description: String,
        arguments: Value,
        status: &'static str,
    },
    #[serde(rename = "security_response_rejected")]
    SecurityResponseRejected {
        req_id: String,
        chat_id: String,
        status: &'static str,
        detail: &'static str,
    },
    #[serde(rename = "command_ack")]
    CommandAck {
        request_id: String,
        command: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

impl WsEvent {
    fn into_json(self) -> Value {
        to_value_safe(self)
    }
}

pub(crate) fn delta(
    chat_id: impl Into<String>,
    turn_id: Option<String>,
    content: impl Into<String>,
) -> Value {
    WsEvent::Delta {
        chat_id: chat_id.into(),
        turn_id,
        content: content.into(),
    }
    .into_json()
}

pub(crate) fn delta_without_turn(
    chat_id: impl Into<String>,
    content: impl Into<String>,
) -> Value {
    WsEvent::DeltaWithoutTurn {
        chat_id: chat_id.into(),
        content: content.into(),
    }
    .into_json()
}

pub(crate) fn reasoning_delta(
    chat_id: impl Into<String>,
    turn_id: Option<String>,
    content: impl Into<String>,
) -> Value {
    WsEvent::ReasoningDelta {
        chat_id: chat_id.into(),
        turn_id,
        content: content.into(),
    }
    .into_json()
}

pub(crate) fn tool_start(
    chat_id: impl Into<String>,
    turn_id: Option<String>,
    tool_call_id: impl Into<String>,
    name: impl Into<String>,
    args: Value,
) -> Value {
    WsEvent::ToolStart {
        chat_id: chat_id.into(),
        turn_id,
        tool_call_id: tool_call_id.into(),
        name: name.into(),
        args,
        status: "running",
    }
    .into_json()
}

pub(crate) fn tool_end(
    chat_id: impl Into<String>,
    turn_id: Option<String>,
    tool_call_id: impl Into<String>,
    name: impl Into<String>,
    status: impl Into<String>,
    output: impl Into<String>,
) -> Value {
    WsEvent::ToolEnd {
        chat_id: chat_id.into(),
        turn_id,
        tool_call_id: tool_call_id.into(),
        name: name.into(),
        status: status.into(),
        output: output.into(),
    }
    .into_json()
}

pub(crate) fn tool_progress(
    chat_id: impl Into<String>,
    turn_id: Option<String>,
    tool_call_id: impl Into<String>,
    name: impl Into<String>,
    message: impl Into<String>,
) -> Value {
    WsEvent::ToolProgress {
        chat_id: chat_id.into(),
        turn_id,
        tool_call_id: tool_call_id.into(),
        name: name.into(),
        message: message.into(),
    }
    .into_json()
}

pub(crate) fn turn_started(chat_id: impl Into<String>, turn_id: impl Into<String>) -> Value {
    WsEvent::TurnStarted {
        chat_id: chat_id.into(),
        turn_id: turn_id.into(),
    }
    .into_json()
}

pub(crate) fn turn_end(chat_id: impl Into<String>, turn_id: Option<String>) -> Value {
    WsEvent::TurnEnd {
        chat_id: chat_id.into(),
        turn_id,
    }
    .into_json()
}

pub(crate) fn error_for_turn(
    chat_id: impl Into<String>,
    turn_id: impl Into<String>,
    detail: impl Into<String>,
) -> Value {
    WsEvent::Error {
        chat_id: Some(chat_id.into()),
        turn_id: Some(turn_id.into()),
        detail: detail.into(),
    }
    .into_json()
}

pub(crate) fn event_error(detail: impl Into<String>) -> Value {
    WsEvent::Error {
        chat_id: None,
        turn_id: None,
        detail: detail.into(),
    }
    .into_json()
}

pub(crate) fn stopped(
    chat_id: impl Into<String>,
    turn_id: Option<String>,
    status: impl Into<String>,
    detail: impl Into<String>,
) -> Value {
    WsEvent::Stopped {
        chat_id: chat_id.into(),
        turn_id,
        status: status.into(),
        detail: detail.into(),
    }
    .into_json()
}

pub(crate) fn security_request(
    chat_id: impl Into<String>,
    turn_id: Option<String>,
    req_id: impl Into<String>,
    tool_name: impl Into<String>,
    description: impl Into<String>,
    arguments: Value,
) -> Value {
    WsEvent::SecurityRequest {
        chat_id: chat_id.into(),
        turn_id,
        req_id: req_id.into(),
        tool_name: tool_name.into(),
        description: description.into(),
        arguments,
        status: "pending",
    }
    .into_json()
}

pub(crate) fn security_response_rejected(
    req_id: impl Into<String>,
    chat_id: impl Into<String>,
) -> Value {
    WsEvent::SecurityResponseRejected {
        req_id: req_id.into(),
        chat_id: chat_id.into(),
        status: "rejected",
        detail: "Security approval is no longer pending or belongs to another client or chat.",
    }
    .into_json()
}

pub(crate) fn command_ack(
    request_id: impl Into<String>,
    command: impl Into<String>,
    status: impl Into<String>,
    detail: Option<impl Into<String>>,
) -> Value {
    WsEvent::CommandAck {
        request_id: request_id.into(),
        command: command.into(),
        status: status.into(),
        detail: detail.map(Into::into),
    }
    .into_json()
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
