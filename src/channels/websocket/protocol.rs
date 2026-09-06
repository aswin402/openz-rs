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
pub(crate) struct WsCapabilities {
    version: u8,
    providers: Vec<Value>,
    #[serde(rename = "securityModes")]
    security_modes: Vec<Value>,
    channels: Vec<Value>,
    browser: WsBrowserCapabilities,
    attachments: WsAttachmentCapabilities,
}

#[derive(Debug, Serialize)]
struct WsBrowserCapabilities {
    #[serde(rename = "firefoxWebdriverPort")]
    firefox_webdriver_port: u16,
    #[serde(rename = "firefoxAttachPort")]
    firefox_attach_port: u16,
}

#[derive(Debug, Serialize)]
struct WsAttachmentCapabilities {
    #[serde(rename = "maxCount")]
    max_count: usize,
    #[serde(rename = "maxFileBytes")]
    max_file_bytes: usize,
    #[serde(rename = "maxTotalBytes")]
    max_total_bytes: usize,
    #[serde(rename = "maxMessageBytes")]
    max_message_bytes: usize,
    #[serde(rename = "ttlSeconds")]
    ttl_seconds: u64,
    #[serde(rename = "allowedMimeTypes")]
    allowed_mime_types: &'static [&'static str],
}

impl WsCapabilities {
    pub(crate) fn into_json(self) -> Value {
        to_value_safe(self)
    }
}

#[derive(Debug, Serialize)]
struct WsRuntimeInventoryEvent {
    event: &'static str,
    inventory: Value,
}

#[derive(Debug, Serialize)]
struct WsSessionsListEvent {
    event: &'static str,
    sessions: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct WsSessionHistoryEvent {
    event: &'static str,
    chat_id: String,
    messages: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct WsReadyEvent {
    event: &'static str,
    chat_id: String,
    client_id: String,
}

#[derive(Debug, Serialize)]
struct WsAttachedEvent {
    event: &'static str,
    chat_id: String,
}

#[derive(Debug, Serialize)]
struct WsActivityNoticeEvent {
    event: &'static str,
    chat_id: String,
    kind: String,
    title: String,
    detail: String,
    timestamp: i64,
}

#[derive(Debug, Serialize)]
struct WsOrchestrationEvent {
    event: &'static str,
    chat_id: String,
    run_id: Option<String>,
    payload: Value,
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
struct WsModelsListEvent {
    event: &'static str,
    providers: Vec<WsModelProvider>,
    partial: bool,
    recent_models: Vec<crate::channels::ModelRef>,
    favorite_models: Vec<crate::channels::ModelRef>,
    active_provider: String,
    active_model: String,
}

#[derive(Debug, Serialize)]
struct WsModelPrefsEvent {
    event: &'static str,
    recent_models: Vec<crate::channels::ModelRef>,
    favorite_models: Vec<crate::channels::ModelRef>,
}

#[derive(Debug, Serialize)]
pub(crate) struct WsSlashCommand {
    cmd: String,
    desc: String,
}

#[derive(Debug, Serialize)]
struct WsSlashCommandsEvent {
    event: &'static str,
    commands: Vec<WsSlashCommand>,
}

#[derive(Debug, Serialize)]
struct WsMcpStatus {
    loaded: u32,
    failed: u32,
    total: u32,
}

#[derive(Debug, Serialize)]
struct WsStatusEvent {
    event: &'static str,
    version: &'static str,
    mcp: WsMcpStatus,
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
struct WsServersListEvent {
    event: &'static str,
    servers: Vec<WsManagedServer>,
    channels: Vec<WsChannelStatus>,
}

#[derive(Debug, Serialize)]
struct WsServerStoppedEvent {
    event: &'static str,
    target: String,
    result: String,
}

#[derive(Debug, Serialize)]
struct WsSessionArchivedEvent {
    event: &'static str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archive_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
struct WsSessionDeletedEvent {
    event: &'static str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
struct WsSkillsUpdatedEvent<T> {
    event: &'static str,
    skills: Vec<T>,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct WsSubagentsUpdatedEvent {
    event: &'static str,
    subagents: Vec<Value>,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
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
struct WsMcpServersEvent {
    event: &'static str,
    servers: Vec<WsMcpServer>,
    stats: WsMcpStatus,
}

#[derive(Debug, Serialize)]
pub(crate) struct WsLogEntry {
    id: String,
    timestamp: String,
    level: String,
    target: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct WsLogsDataEvent {
    event: &'static str,
    logs: Vec<WsLogEntry>,
}

#[derive(Debug, Serialize)]
struct WsCronJobsUpdatedEvent {
    event: &'static str,
    status: String,
    id: String,
    result: Value,
    inventory: crate::core::inventory::RuntimeInventory,
}

#[derive(Debug, Serialize)]
struct WsCronLogsEvent {
    event: &'static str,
    id: Option<String>,
    runs: Vec<crate::cron::CronRunRecord>,
}

#[derive(Debug, Serialize)]
struct WsConfigUpdateRejectedEvent {
    event: &'static str,
    reason: String,
    requires_gateway_token: bool,
}

#[derive(Debug, Serialize)]
struct WsConfigDataEvent {
    event: &'static str,
    defaults: Value,
    skills: Vec<crate::agent::skills::SkillView>,
    mcp_servers: Vec<Value>,
    subagents: Vec<Value>,
    providers: serde_json::Map<String, Value>,
    channels: Value,
    capabilities: Value,
    version: &'static str,
}

#[derive(Debug, Serialize)]
struct WsConfigUpdatedEvent {
    event: &'static str,
    defaults: Value,
    capabilities: Value,
}

#[derive(Debug, Serialize)]
struct WsCognitiveMemoryStats {
    #[serde(rename = "entitiesCount")]
    entities_count: i64,
    #[serde(rename = "relationsCount")]
    relations_count: i64,
    #[serde(rename = "factsCount")]
    facts_count: i64,
    #[serde(rename = "workingMemoryKeys")]
    working_memory_keys: Vec<String>,
}

#[derive(Debug, Serialize)]
struct WsCognitiveMemoryPaths {
    #[serde(rename = "memoryDb")]
    memory_db: String,
    #[serde(rename = "graphDb")]
    graph_db: String,
}

#[derive(Debug, Serialize)]
struct WsCognitiveMemoryEvent {
    event: &'static str,
    stats: WsCognitiveMemoryStats,
    paths: WsCognitiveMemoryPaths,
    nodes: Vec<Value>,
    edges: Vec<Value>,
    facts: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct WsNotificationEvent {
    event: &'static str,
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
    WsCapabilities {
        version: 1,
        providers,
        security_modes,
        channels,
        browser: WsBrowserCapabilities {
            firefox_webdriver_port,
            firefox_attach_port,
        },
        attachments: WsAttachmentCapabilities {
            max_count,
            max_file_bytes,
            max_total_bytes,
            max_message_bytes,
            ttl_seconds,
            allowed_mime_types,
        },
    }
    .into_json()
}

pub(crate) fn runtime_inventory(inventory: impl Serialize) -> Value {
    let inventory = to_value_safe(inventory);
    to_value_safe(WsRuntimeInventoryEvent {
        event: "runtime_inventory",
        inventory,
    })
}

pub(crate) fn sessions_list(sessions: Vec<Value>) -> Value {
    to_value_safe(WsSessionsListEvent {
        event: "sessions_list",
        sessions,
        total: None,
        offset: None,
        limit: None,
    })
}

pub(crate) fn sessions_list_paginated(
    sessions: Vec<Value>,
    total: usize,
    offset: usize,
    limit: usize,
) -> Value {
    to_value_safe(WsSessionsListEvent {
        event: "sessions_list",
        sessions,
        total: Some(total),
        offset: Some(offset),
        limit: Some(limit),
    })
}

pub(crate) fn session_history(chat_id: impl Into<String>, messages: Vec<Value>) -> Value {
    to_value_safe(WsSessionHistoryEvent {
        event: "session_history",
        chat_id: chat_id.into(),
        messages,
    })
}

pub(crate) fn ready(chat_id: impl Into<String>, client_id: impl Into<String>) -> Value {
    to_value_safe(WsReadyEvent {
        event: "ready",
        chat_id: chat_id.into(),
        client_id: client_id.into(),
    })
}

pub(crate) fn attached(chat_id: impl Into<String>) -> Value {
    to_value_safe(WsAttachedEvent {
        event: "attached",
        chat_id: chat_id.into(),
    })
}

pub(crate) fn activity_notice(
    chat_id: impl Into<String>,
    kind: impl Into<String>,
    title: impl Into<String>,
    detail: impl Into<String>,
    timestamp: i64,
) -> Value {
    to_value_safe(WsActivityNoticeEvent {
        event: "activity_notice",
        chat_id: chat_id.into(),
        kind: kind.into(),
        title: title.into(),
        detail: detail.into(),
        timestamp,
    })
}

pub(crate) fn orchestration_event(chat_id: impl Into<String>, payload: Value) -> Value {
    let run_id = payload
        .get("run_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    to_value_safe(WsOrchestrationEvent {
        event: "orchestration_event",
        chat_id: chat_id.into(),
        run_id,
        payload,
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
    to_value_safe(WsModelsListEvent {
        event: "models_list",
        providers,
        partial,
        recent_models: prefs.recent,
        favorite_models: prefs.favorites,
        active_provider: active_provider.into(),
        active_model: active_model.into(),
    })
}

pub(crate) fn model_prefs(prefs: crate::channels::ModelPrefs) -> Value {
    to_value_safe(WsModelPrefsEvent {
        event: "model_prefs",
        recent_models: prefs.recent,
        favorite_models: prefs.favorites,
    })
}

pub(crate) fn slash_command(cmd: impl Into<String>, desc: impl Into<String>) -> WsSlashCommand {
    WsSlashCommand {
        cmd: cmd.into(),
        desc: desc.into(),
    }
}

pub(crate) fn slash_commands(commands: Vec<WsSlashCommand>) -> Value {
    to_value_safe(WsSlashCommandsEvent {
        event: "slash_commands",
        commands,
    })
}

pub(crate) fn gateway_status(loaded: u32, failed: u32, total: u32) -> Value {
    to_value_safe(WsStatusEvent {
        event: "status",
        version: env!("CARGO_PKG_VERSION"),
        mcp: WsMcpStatus {
            loaded,
            failed,
            total,
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
    WsChannelStatus {
        name: name.into(),
        enabled,
        status: if enabled { "configured" } else { "disabled" },
        token_configured,
    }
}

pub(crate) fn servers_list(
    servers: Vec<WsManagedServer>,
    channels: Vec<WsChannelStatus>,
) -> Value {
    to_value_safe(WsServersListEvent {
        event: "servers_list",
        servers,
        channels,
    })
}

pub(crate) fn server_stopped(target: impl Into<String>, result: impl Into<String>) -> Value {
    to_value_safe(WsServerStoppedEvent {
        event: "server_stopped",
        target: target.into(),
        result: result.into(),
    })
}

pub(crate) fn session_archived(
    status: &'static str,
    chat_id: Option<String>,
    session_key: Option<String>,
    archived: Option<bool>,
    archive_path: Option<String>,
    detail: Option<String>,
) -> Value {
    to_value_safe(WsSessionArchivedEvent {
        event: "session_archived",
        status,
        chat_id,
        session_key,
        archived,
        archive_path,
        detail,
    })
}

pub(crate) fn session_deleted(
    status: &'static str,
    chat_id: Option<String>,
    session_key: Option<String>,
    deleted: Option<bool>,
    detail: Option<String>,
) -> Value {
    to_value_safe(WsSessionDeletedEvent {
        event: "session_deleted",
        status,
        chat_id,
        session_key,
        deleted,
        detail,
    })
}

pub(crate) fn skills_updated<T: Serialize>(
    skills: Vec<T>,
    status: &'static str,
    name: Option<String>,
) -> Value {
    to_value_safe(WsSkillsUpdatedEvent {
        event: "skills_updated",
        skills,
        status,
        name,
    })
}

pub(crate) fn subagents_updated(
    subagents: Vec<Value>,
    status: &'static str,
    name: Option<String>,
) -> Value {
    to_value_safe(WsSubagentsUpdatedEvent {
        event: "subagents_updated",
        subagents,
        status,
        name,
    })
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
    to_value_safe(WsMcpServersEvent {
        event: "mcp_servers",
        servers,
        stats: WsMcpStatus {
            loaded,
            failed,
            total,
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
    to_value_safe(WsLogsDataEvent {
        event: "logs_data",
        logs,
    })
}

pub(crate) fn cron_jobs_updated(
    status: impl Into<String>,
    id: impl Into<String>,
    result: Value,
    inventory: crate::core::inventory::RuntimeInventory,
) -> Value {
    to_value_safe(WsCronJobsUpdatedEvent {
        event: "cron_jobs_updated",
        status: status.into(),
        id: id.into(),
        result,
        inventory,
    })
}

pub(crate) fn cron_logs(
    id: Option<String>,
    runs: Vec<crate::cron::CronRunRecord>,
) -> Value {
    to_value_safe(WsCronLogsEvent {
        event: "cron_logs",
        id,
        runs,
    })
}

pub(crate) fn config_update_rejected(
    reason: impl Into<String>,
    requires_gateway_token: bool,
) -> Value {
    to_value_safe(WsConfigUpdateRejectedEvent {
        event: "config_update_rejected",
        reason: reason.into(),
        requires_gateway_token,
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
    to_value_safe(WsConfigDataEvent {
        event: "config_data",
        defaults,
        skills,
        mcp_servers,
        subagents,
        providers,
        channels,
        capabilities,
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub(crate) fn config_updated(defaults: Value, capabilities: Value) -> Value {
    to_value_safe(WsConfigUpdatedEvent {
        event: "config_updated",
        defaults,
        capabilities,
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
    to_value_safe(WsCognitiveMemoryEvent {
        event: "cognitive_memory",
        stats: WsCognitiveMemoryStats {
            entities_count,
            relations_count,
            facts_count,
            working_memory_keys,
        },
        paths: WsCognitiveMemoryPaths {
            memory_db: memory_db.into(),
            graph_db: graph_db.into(),
        },
        nodes,
        edges,
        facts,
    })
}

pub(crate) fn notification(message: impl Into<String>) -> Value {
    to_value_safe(WsNotificationEvent {
        event: "notification",
        message: message.into(),
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
