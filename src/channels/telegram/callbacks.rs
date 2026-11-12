use crate::agent::AgentLoop;
use crate::channels::notifications::telegram_api_url;
use crate::channels::telegram::state::{
    get_remote_menu_option, remote_session_button_label, set_remote_session, APPROVAL_CALLBACKS,
};
use crate::channels::telegram::types::TelegramCallbackQuery;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

async fn handle_model_provider_callback(
    cb: &TelegramCallbackQuery,
    callback_value: &str,
    bot_token: &str,
    client: &Client,
    agent_loop: &Arc<AgentLoop>,
) {
    let inner_msg = match &cb.message {
        Some(msg) => msg,
        None => return,
    };
    let chat_id = inner_msg.chat.id;
    let selected_idx = callback_value.parse::<usize>().ok();
    let config = crate::config::loader::load_config()
        .unwrap_or_else(|_| agent_loop.config.clone());
    let providers = crate::channels::configured_provider_models(&config);
    let provider = selected_idx.and_then(|idx| providers.get(idx).copied());

    let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
    let answer_text = match provider.is_some() {
        true => "Provider selected",
        false => "Provider no longer available",
    };
    let answer_payload = serde_json::json!({
        "callback_query_id": cb.id,
        "text": answer_text,
        "show_alert": provider.is_none()
    });
    let _ = client.post(&answer_url).json(&answer_payload).send().await;

    let message_id = match inner_msg.message_id {
        Some(id) => id,
        None => return,
    };
    let edit_url = telegram_api_url(bot_token, "editMessageText");
    let edit_payload = match provider {
        Some(provider) => {
            let keyboard: Vec<Vec<serde_json::Value>> = provider
                .models
                .iter()
                .enumerate()
                .map(|(model_idx, model)| {
                    vec![serde_json::json!({
                        "text": model,
                        "callback_data": format!("model_select:{}:{}", selected_idx.unwrap_or(0), model_idx)
                    })]
                })
                .collect();
            serde_json::json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "text": format!("Select a model for {} ({})", provider.display, provider.name),
                "reply_markup": { "inline_keyboard": keyboard }
            })
        }
        None => serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": "That provider is no longer available. Use /switch-model again."
        }),
    };
    let _ = client.post(&edit_url).json(&edit_payload).send().await;
}

async fn handle_model_select_callback(
    cb: &TelegramCallbackQuery,
    callback_value: &str,
    bot_token: &str,
    client: &Client,
    agent_loop: &Arc<AgentLoop>,
) {
    let inner_msg = match &cb.message {
        Some(msg) => msg,
        None => return,
    };
    let chat_id = inner_msg.chat.id;
    let indexes: Vec<&str> = callback_value.split(':').collect();
    let provider_idx = indexes.first().and_then(|v| v.parse::<usize>().ok());
    let model_idx = indexes.get(1).and_then(|v| v.parse::<usize>().ok());
    let config = crate::config::loader::load_config()
        .unwrap_or_else(|_| agent_loop.config.clone());
    let providers = crate::channels::configured_provider_models(&config);
    let selection = provider_idx
        .and_then(|pidx| providers.get(pidx).copied().map(|p| (pidx, p)))
        .and_then(|(_, provider)| {
            model_idx
                .and_then(|midx| provider.models.get(midx).copied())
                .map(|model| (provider, model))
        });

    let result_text = match selection {
        Some((provider, model)) => {
            let session_key = format!("telegram:{}", chat_id);
            let mut overrides = serde_json::Map::new();
            overrides.insert(
                "model".to_string(),
                serde_json::Value::String(model.to_string()),
            );
            overrides.insert(
                "provider".to_string(),
                serde_json::Value::String(provider.name.to_string()),
            );
            let _ = crate::agent::agent_loop::save_session_overrides(
                &agent_loop.session_manager,
                &session_key,
                &overrides,
            )
            .await;
            match crate::channels::save_default_model_selection(&config, provider.name, model) {
                Ok(()) => format!(
                    "Model switched to {} with provider {}. Active session and new channel turns will use this selection.",
                    model, provider.name
                ),
                Err(e) => format!("Failed to switch model: {}", e),
            }
        }
        None => "That model selection is no longer available. Use /switch-model again.".to_string(),
    };

    let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
    let answer_payload = serde_json::json!({
        "callback_query_id": cb.id,
        "text": "Model switch handled"
    });
    let _ = client.post(&answer_url).json(&answer_payload).send().await;

    match inner_msg.message_id {
        Some(message_id) => {
            let edit_url = telegram_api_url(bot_token, "editMessageText");
            let edit_payload = serde_json::json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "text": result_text
            });
            let _ = client.post(&edit_url).json(&edit_payload).send().await;
        }
        None => {}
    }
}

async fn handle_resume_cancel_callback(
    cb: &TelegramCallbackQuery,
    bot_token: &str,
    client: &Client,
) {
    let inner_msg = match &cb.message {
        Some(msg) => msg,
        None => return,
    };
    let chat_id = inner_msg.chat.id;
    let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
    let answer_payload = serde_json::json!({
        "callback_query_id": cb.id,
        "text": "Continuing current session"
    });
    let _ = client.post(&answer_url).json(&answer_payload).send().await;

    match inner_msg.message_id {
        Some(message_id) => {
            let edit_url = telegram_api_url(bot_token, "editMessageText");
            let edit_payload = serde_json::json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "text": "Continuing current session. No session was changed."
            });
            let _ = client.post(&edit_url).json(&edit_payload).send().await;
        }
        None => {}
    }
}

async fn handle_resume_select_callback(
    cb: &TelegramCallbackQuery,
    callback_value: &str,
    bot_token: &str,
    client: &Client,
    agent_loop: &Arc<AgentLoop>,
) {
    let inner_msg = match &cb.message {
        Some(msg) => msg,
        None => return,
    };
    let chat_id = inner_msg.chat.id;
    let idx = callback_value.parse::<usize>().ok();
    let session_key = format!("telegram:{}", chat_id);
    let sessions = crate::channels::list_channel_sessions(
        &agent_loop.session_manager.dir,
        &session_key,
        10,
    );
    let selected_item = idx.and_then(|i| sessions.get(i));
    let result_text = match (idx, selected_item) {
        (Some(_), Some(item)) => {
            match crate::channels::resume_channel_session(
                &agent_loop.session_manager,
                &session_key,
                &item.key,
            )
            .await
            {
                Ok(msg) => msg,
                Err(e) => format!("Failed to resume session: {e}"),
            }
        }
        (Some(_), None) => "That session is no longer available. Use /resume again.".to_string(),
        (None, _) => "Invalid resume selection. Use /resume again.".to_string(),
    };

    let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
    let answer_payload = serde_json::json!({
        "callback_query_id": cb.id,
        "text": "Resume selection handled"
    });
    let _ = client.post(&answer_url).json(&answer_payload).send().await;

    match inner_msg.message_id {
        Some(message_id) => {
            let edit_url = telegram_api_url(bot_token, "editMessageText");
            let edit_payload = serde_json::json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "text": result_text
            });
            let _ = client.post(&edit_url).json(&edit_payload).send().await;
        }
        None => {}
    }
}

async fn handle_remote_idx_callback(
    cb: &TelegramCallbackQuery,
    callback_value: &str,
    bot_token: &str,
    client: &Client,
) {
    let inner_msg = match &cb.message {
        Some(msg) => msg,
        None => return,
    };
    let chat_id = inner_msg.chat.id;
    let selected = callback_value
        .parse::<usize>()
        .ok()
        .and_then(|idx| get_remote_menu_option(chat_id, idx))
        .filter(|target| crate::agent::activity::remote_target_is_available(&target.session_key));

    let resolved = selected.map(|mut target| {
        match target.session_key == "ws:new" {
            true => {
                let fresh_id = uuid::Uuid::new_v4().to_string();
                target.session_key = format!("ws:{fresh_id}");
                target.label = format!("🌐 WebUI (New Session) | {}", &fresh_id[..8]);
            }
            false => {}
        }
        target
    });

    let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
    let answer_payload = match &resolved {
        Some(target) => {
            set_remote_session(chat_id, target.session_key.clone());
            serde_json::json!({
                "callback_query_id": cb.id,
                "text": format!("Remote {} session selected", target.surface.to_uppercase())
            })
        }
        None => serde_json::json!({
            "callback_query_id": cb.id,
            "text": "That session is no longer active. Use /remote again.",
            "show_alert": true
        }),
    };
    let _ = client.post(&answer_url).json(&answer_payload).send().await;

    match inner_msg.message_id {
        Some(message_id) => {
            let edit_url = telegram_api_url(bot_token, "editMessageText");
            let text = resolved
                .map(|target| {
                    let surface_name = match target.surface == "webui" {
                        true => "WebUI / Gateway",
                        false => "TUI",
                    };
                    format!(
                        "🔌 Remote mode active for {}\nNext messages will execute inside that {} session and reply here. Use /local or /exit to return to Telegram local chat.",
                        target.label, surface_name
                    )
                })
                .unwrap_or_else(|| {
                    "That session is no longer active. Use /remote again.".to_string()
                });
            let edit_payload = serde_json::json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "text": text
            });
            let _ = client.post(&edit_url).json(&edit_payload).send().await;
        }
        None => {}
    }
}

async fn handle_remote_callback(
    cb: &TelegramCallbackQuery,
    callback_value: &str,
    bot_token: &str,
    client: &Client,
) {
    let inner_msg = match &cb.message {
        Some(msg) => msg,
        None => return,
    };
    let chat_id = inner_msg.chat.id;
    let resolved_key = match callback_value == "ws:new" {
        true => format!("ws:{}", uuid::Uuid::new_v4()),
        false => callback_value.to_string(),
    };
    let available = crate::agent::activity::remote_target_is_available(&resolved_key);
    let tui_label = crate::agent::activity::list_active_tui_sessions()
        .into_iter()
        .find(|session| session.session_key == resolved_key)
        .map(|session| remote_session_button_label(&session));

    let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
    let answer_payload = match available {
        true => {
            set_remote_session(chat_id, resolved_key.clone());
            serde_json::json!({
                "callback_query_id": cb.id,
                "text": "Remote session selected"
            })
        }
        false => serde_json::json!({
            "callback_query_id": cb.id,
            "text": "That session is no longer active. Use /remote again.",
            "show_alert": true
        }),
    };
    let _ = client.post(&answer_url).json(&answer_payload).send().await;

    match inner_msg.message_id {
        Some(message_id) => {
            let edit_url = telegram_api_url(bot_token, "editMessageText");
            let text = match available {
                true => {
                    let display = tui_label.unwrap_or_else(|| resolved_key.clone());
                    format!(
                        "🔌 Remote mode active for {}\nNext messages are forwarded to that session. Use /local or /exit to leave remote mode.",
                        display
                    )
                }
                false => "That session is no longer active. Use /remote again.".to_string(),
            };
            let edit_payload = serde_json::json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "text": text
            });
            let _ = client.post(&edit_url).json(&edit_payload).send().await;
        }
        None => {}
    }
}

pub(crate) async fn handle_callback(
    cb: TelegramCallbackQuery,
    bot_token: &str,
    client: &Client,
    agent_loop: &Arc<AgentLoop>,
) {
    let data = match &cb.data {
        Some(data) => data.clone(),
        None => return,
    };
    let (action, callback_value) = match data.split_once(':') {
        Some(pair) => pair,
        None => return,
    };

    match action {
        "model_provider" => {
            handle_model_provider_callback(&cb, callback_value, bot_token, client, agent_loop)
                .await;
            return;
        }
        "model_select" => {
            handle_model_select_callback(&cb, callback_value, bot_token, client, agent_loop).await;
            return;
        }
        "resume_cancel" => {
            handle_resume_cancel_callback(&cb, bot_token, client).await;
            return;
        }
        "resume_select" => {
            handle_resume_select_callback(&cb, callback_value, bot_token, client, agent_loop).await;
            return;
        }
        "remote_idx" => {
            handle_remote_idx_callback(&cb, callback_value, bot_token, client).await;
            return;
        }
        "remote" => {
            handle_remote_callback(&cb, callback_value, bot_token, client).await;
            return;
        }
        _ => {}
    }

    let req_id = callback_value;
    let approved = action == "approve";

    let map = APPROVAL_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
    match map.lock() {
        Ok(mut guard) => match guard.remove(req_id) {
            Some(tx) => {
                let _ = tx.send(approved);
            }
            None => {}
        },
        Err(_) => {}
    }

    let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
    let answer_text = match approved {
        true => "Action approved ✅",
        false => "Action denied ❌",
    };
    let answer_payload = serde_json::json!({
        "callback_query_id": cb.id,
        "text": answer_text
    });
    let _ = client.post(&answer_url).json(&answer_payload).send().await;

    let (chat_id, message_id) = match &cb.message {
        Some(inner_msg) => match inner_msg.message_id {
            Some(message_id) => (inner_msg.chat.id, message_id),
            None => return,
        },
        None => return,
    };

    let edit_markup_url = telegram_api_url(bot_token, "editMessageReplyMarkup");
    let button_text = match approved {
        true => "Approved ✅",
        false => "Denied ❌",
    };
    let edit_payload = serde_json::json!({
        "chat_id": chat_id,
        "message_id": message_id,
        "reply_markup": {
            "inline_keyboard": [[
                {
                    "text": button_text,
                    "callback_data": "done"
                }
            ]]
        }
    });
    let _ = client.post(&edit_markup_url).json(&edit_payload).send().await;
}
