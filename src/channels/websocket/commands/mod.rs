//! WebSocket command vocabulary and command-family handlers.
//!
//! The connection loop still owns socket/session state and dispatch ordering.
//! Command names live here so the acknowledgement allowlist and dispatch code
//! cannot silently drift apart as handlers are split into family modules.

pub(crate) mod config;
pub(crate) mod cron;
pub(crate) mod models;
pub(crate) mod memory;
pub(crate) mod observability;
pub(crate) mod profiles;
pub(crate) mod sessions;
pub(crate) mod system;

pub(super) async fn handle_post_message_command(
    command: &str,
    envelope: &serde_json::Value,
    chat_id: &str,
    client_id: &str,
    state: &super::WsState,
    tx: &tokio::sync::mpsc::Sender<axum::extract::ws::Message>,
) -> bool {
    match command {
        "security_response" => {
            let req_id = envelope
                .get("req_id")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            let approved = envelope
                .get("approved")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let response_chat_id = super::normalize_ws_chat_id(chat_id);
            if !super::resolve_ws_approval(
                &req_id,
                client_id,
                &response_chat_id,
                approved,
            ) {
                let rejection = super::security_response_rejected_event(
                    &req_id,
                    &response_chat_id,
                );
                let _ = super::publish_ws_event_to_client(client_id, rejection);
            }
        }
        "get_models" => {
            let config = state.current_config();
            let event = models::models_list_event(&config, envelope).await;
            super::send_event(tx, event).await;
        }
        "toggle_favorite_model" => {
            let event = models::toggle_favorite_model_event(envelope);
            super::send_event(tx, event).await;
        }
        "get_config" => {
            let config = state.current_config();
            let event = config::config_data_event(&config).await;
            super::send_event(tx, event).await;
        }
        "set_config" => {
            if config::config_update_requires_gateway_token(envelope)
                && !super::gateway_token_configured()
            {
                let event = super::protocol::config_update_rejected(
                    "OPENZ_GATEWAY_TOKEN is required for sensitive configuration updates",
                    true,
                );
                super::send_event(tx, event).await;
                return true;
            }

            let mut config = state.current_config();
            config::apply_config_update(&mut config, envelope);
            let _ = crate::config::loader::save_config(&config);
            if let Ok(mut live) = state.live_config.write() {
                *live = config.clone();
            }
            let event = config::config_updated_event(&config);
            super::send_event(tx, event.clone()).await;
            super::events::publish_ws_event_except(client_id, event);
        }
        "save_skill" => {
            let event = profiles::save_skill_event(envelope);
            super::send_event(tx, event).await;
        }
        "delete_skill" => {
            let event = profiles::delete_skill_event(envelope);
            super::send_event(tx, event).await;
        }
        "save_subagent" => {
            let event = profiles::save_subagent_event(&state.agent_loop.config, envelope).await;
            super::send_event(tx, event).await;
        }
        "update_subagent_settings" => {
            let event = profiles::update_subagent_settings_event(
                &state.agent_loop.config,
                envelope,
            )
            .await;
            super::send_event(tx, event).await;
        }
        "delete_subagent" => {
            let event = profiles::delete_subagent_event(envelope).await;
            super::send_event(tx, event).await;
        }
        "get_slash_commands" => {
            let event = system::slash_commands_event();
            super::send_event(tx, event).await;
        }
        "get_status" => {
            let event = system::status_event();
            super::send_event(tx, event).await;
        }
        "pause_cron_job" | "resume_cron_job" | "delete_cron_job" => {
            let config = state.current_config();
            let event = cron::cron_update_event(
                command,
                envelope,
                &config,
                Some(&state.agent_loop.tools),
            )
            .await;
            super::send_event(tx, event).await;
        }
        "get_cron_logs" => {
            let event = cron::cron_logs_event(envelope);
            super::send_event(tx, event).await;
        }
        "get_runtime_inventory" => {
            let config = state.current_config();
            let event = system::runtime_inventory_event(&config, Some(&state.agent_loop.tools));
            super::send_event(tx, event).await;
        }
        "get_servers" => {
            let config = state.current_config();
            let event = system::servers_list_event(&config).await;
            super::send_event(tx, event).await;
        }
        "stop_server" => {
            let event = system::stop_server_event(envelope);
            super::send_event(tx, event).await;
        }
        _ => return false,
    }

    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WsCommand {
    Ping,
    Message,
    NewChat,
    Attach,
    LoadHistory,
    ArchiveSession,
    DeleteSession,
    ListSessions,
    GetCognitiveMemory,
    GetMcpServers,
    GetLogs,
    GetServers,
    StopServer,
    GetModels,
    ToggleFavoriteModel,
    GetConfig,
    SetConfig,
    SaveSkill,
    DeleteSkill,
    SaveSubagent,
    UpdateSubagentSettings,
    DeleteSubagent,
    GetSlashCommands,
    GetStatus,
    GetRuntimeInventory,
    PauseCronJob,
    ResumeCronJob,
    DeleteCronJob,
    GetCronLogs,
    SecurityResponse,
}

impl WsCommand {
    pub(crate) fn parse(command: &str) -> Option<Self> {
        Some(match command {
            "ping" => Self::Ping,
            "message" => Self::Message,
            "new_chat" => Self::NewChat,
            "attach" => Self::Attach,
            "load_history" => Self::LoadHistory,
            "archive_session" => Self::ArchiveSession,
            "delete_session" => Self::DeleteSession,
            "list_sessions" => Self::ListSessions,
            "get_cognitive_memory" => Self::GetCognitiveMemory,
            "get_mcp_servers" => Self::GetMcpServers,
            "get_logs" => Self::GetLogs,
            "get_servers" => Self::GetServers,
            "stop_server" => Self::StopServer,
            "get_models" => Self::GetModels,
            "toggle_favorite_model" => Self::ToggleFavoriteModel,
            "get_config" => Self::GetConfig,
            "set_config" => Self::SetConfig,
            "save_skill" => Self::SaveSkill,
            "delete_skill" => Self::DeleteSkill,
            "save_subagent" => Self::SaveSubagent,
            "update_subagent_settings" => Self::UpdateSubagentSettings,
            "delete_subagent" => Self::DeleteSubagent,
            "get_slash_commands" => Self::GetSlashCommands,
            "get_status" => Self::GetStatus,
            "get_runtime_inventory" => Self::GetRuntimeInventory,
            "pause_cron_job" => Self::PauseCronJob,
            "resume_cron_job" => Self::ResumeCronJob,
            "delete_cron_job" => Self::DeleteCronJob,
            "get_cron_logs" => Self::GetCronLogs,
            "security_response" => Self::SecurityResponse,
            _ => return None,
        })
    }
}
