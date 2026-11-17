//! WebSocket command vocabulary and command-family handlers.
//!
//! The connection loop still owns socket/session state and dispatch ordering.
//! Command names live here so the acknowledgement allowlist and dispatch code
//! cannot silently drift apart as handlers are split into family modules.

pub(crate) mod config;
pub(crate) mod cron;
pub(crate) mod memory;
pub(crate) mod models;
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
            match super::resolve_ws_approval(&req_id, client_id, &response_chat_id, approved) {
                true => {}
                false => {
                    let rejection =
                        super::security_response_rejected_event(&req_id, &response_chat_id);
                    let _ = super::publish_ws_event_to_client(client_id, rejection);
                }
            }
        }
        "get_models" => {
            let config = state.current_config();
            let envelope_owned = envelope.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let event = models::models_list_event(&config, &envelope_owned).await;
                super::send_event(&tx_clone, event).await;
            });
        }
        "toggle_favorite_model" => {
            let event = models::toggle_favorite_model_event(envelope);
            super::send_event(tx, event).await;
        }
        "get_config" => {
            let config = state.current_config();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let event = config::config_data_event(&config).await;
                super::send_event(&tx_clone, event).await;
            });
        }
        "set_config" => {
            let token_missing = config::config_update_requires_gateway_token(envelope)
                && !super::gateway_token_configured();
            match token_missing {
                true => {
                    let event = super::protocol::config_update_rejected(
                        "OPENZ_GATEWAY_TOKEN is required for sensitive configuration updates",
                        true,
                    );
                    super::send_event(tx, event).await;
                    return true;
                }
                false => {}
            }

            let mut disk_config = crate::config::loader::load_config()
                .unwrap_or_else(|_| state.current_config());
            let prev_boot = disk_config
                .channels
                .websocket
                .as_ref()
                .is_some_and(|ws| ws.enabled && ws.start_on_boot);
            config::apply_webui_disk_config_update(&mut disk_config, envelope);
            let _ = crate::config::loader::save_config(&disk_config);
            let next_ws = disk_config.channels.websocket.clone().unwrap_or_default();
            let next_boot = next_ws.enabled && next_ws.start_on_boot;
            match prev_boot != next_boot {
                true => {
                    let _ = crate::cli::configure::sync_gateway_systemd_service(
                        next_boot,
                        false,
                        &next_ws.host,
                        next_ws.port,
                    );
                }
                false => {}
            }
            let effective_config = disk_config.with_ui_surface("webui");
            crate::providers::DynamicContextRegistry::ensure_active_model_synced(
                &effective_config,
            )
            .await;
            match state.live_config.write() {
                Ok(mut live) => *live = effective_config.clone(),
                Err(_) => {}
            }
            let event = config::config_updated_event(&effective_config);
            super::send_event(tx, event.clone()).await;
            super::events::publish_ws_event_except(client_id, event);
        }
        "save_skill" => {
            let envelope_owned = envelope.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                match tokio::task::spawn_blocking(move || profiles::save_skill_event(&envelope_owned))
                    .await
                {
                    Ok(event) => super::send_event(&tx_clone, event).await,
                    Err(_) => {}
                }
            });
        }
        "delete_skill" => {
            let envelope_owned = envelope.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                match tokio::task::spawn_blocking(move || {
                    profiles::delete_skill_event(&envelope_owned)
                })
                .await
                {
                    Ok(event) => super::send_event(&tx_clone, event).await,
                    Err(_) => {}
                }
            });
        }
        "rollback_skill" => {
            let envelope_owned = envelope.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                match tokio::task::spawn_blocking(move || {
                    profiles::rollback_skill_event(&envelope_owned)
                })
                .await
                {
                    Ok(event) => super::send_event(&tx_clone, event).await,
                    Err(_) => {}
                }
            });
        }
        "save_subagent" => {
            let config = state.current_config();
            let envelope_owned = envelope.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let event = profiles::save_subagent_event(&config, &envelope_owned).await;
                super::send_event(&tx_clone, event).await;
            });
        }
        "update_subagent_settings" => {
            let config = state.current_config();
            let envelope_owned = envelope.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let event =
                    profiles::update_subagent_settings_event(&config, &envelope_owned).await;
                super::send_event(&tx_clone, event).await;
            });
        }
        "delete_subagent" => {
            let envelope_owned = envelope.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let event = profiles::delete_subagent_event(&envelope_owned).await;
                super::send_event(&tx_clone, event).await;
            });
        }
        "get_status" => {
            let event = system::status_event();
            super::send_event(tx, event).await;
        }
        "pause_cron_job" | "resume_cron_job" | "delete_cron_job" => {
            let config = state.current_config();
            let command_owned = command.to_string();
            let envelope_owned = envelope.clone();
            let tools = state.agent_loop.tools.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let event = cron::cron_update_event(
                    &command_owned,
                    &envelope_owned,
                    &config,
                    Some(&tools),
                )
                .await;
                super::send_event(&tx_clone, event).await;
            });
        }
        "get_cron_logs" => {
            let envelope_owned = envelope.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                match tokio::task::spawn_blocking(move || cron::cron_logs_event(&envelope_owned))
                    .await
                {
                    Ok(event) => super::send_event(&tx_clone, event).await,
                    Err(_) => {}
                }
            });
        }
        "get_runtime_inventory" => {
            let config = state.current_config();
            let tools = state.agent_loop.tools.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                match tokio::task::spawn_blocking(move || {
                    system::runtime_inventory_event(&config, Some(&tools))
                })
                .await
                {
                    Ok(event) => super::send_event(&tx_clone, event).await,
                    Err(_) => {}
                }
            });
        }
        "get_servers" => {
            let config = state.current_config();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let event = system::servers_list_event(&config).await;
                super::send_event(&tx_clone, event).await;
            });
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
    StopTurn,
    NewChat,
    Attach,
    LoadHistory,
    ClearSession,
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
    RollbackSkill,
    SaveSubagent,
    UpdateSubagentSettings,
    DeleteSubagent,
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
            "stop_turn" => Self::StopTurn,
            "new_chat" => Self::NewChat,
            "attach" => Self::Attach,
            "load_history" => Self::LoadHistory,
            "clear_session" => Self::ClearSession,
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
            "rollback_skill" => Self::RollbackSkill,
            "save_subagent" => Self::SaveSubagent,
            "update_subagent_settings" => Self::UpdateSubagentSettings,
            "delete_subagent" => Self::DeleteSubagent,
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
