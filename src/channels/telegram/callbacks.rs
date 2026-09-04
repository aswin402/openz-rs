use crate::agent::AgentLoop;
use crate::channels::notifications::telegram_api_url;
use crate::channels::telegram::state::{
    remote_session_button_label, set_remote_session, APPROVAL_CALLBACKS,
};
use crate::channels::telegram::types::TelegramCallbackQuery;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub(crate) async fn handle_callback(
    cb: TelegramCallbackQuery,
    bot_token: &str,
    client: &Client,
    agent_loop: &Arc<AgentLoop>,
) {
    let Some(ref data) = cb.data else { return };
    let parts: Vec<&str> = data.splitn(2, ':').collect();
    if parts.len() != 2 {
        return;
    }

    let action = parts[0];
    let callback_value = parts[1];

    if action == "model_provider" {
        if let Some(ref inner_msg) = cb.message {
            let chat_id = inner_msg.chat.id;
            let selected_idx = callback_value.parse::<usize>().ok();
            let config = crate::config::loader::load_config()
                .unwrap_or_else(|_| agent_loop.config.clone());
            let providers = crate::channels::configured_provider_models(&config);
            let provider = selected_idx.and_then(|idx| providers.get(idx).copied());

            let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
            let answer_payload = serde_json::json!({
                "callback_query_id": cb.id,
                "text": if provider.is_some() { "Provider selected" } else { "Provider no longer available" },
                "show_alert": provider.is_none()
            });
            let _ = client.post(&answer_url).json(&answer_payload).send().await;

            if let Some(message_id) = inner_msg.message_id {
                let edit_url = telegram_api_url(bot_token, "editMessageText");
                let edit_payload = if let Some(provider) = provider {
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
                } else {
                    serde_json::json!({
                        "chat_id": chat_id,
                        "message_id": message_id,
                        "text": "That provider is no longer available. Use /switch-model again."
                    })
                };
                let _ = client.post(&edit_url).json(&edit_payload).send().await;
            }
        }
        return;
    }

    if action == "model_select" {
        if let Some(ref inner_msg) = cb.message {
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

            let result_text = if let Some((provider, model)) = selection {
                match crate::channels::save_default_model_selection(&config, provider.name, model) {
                    Ok(()) => format!(
                        "Model switched to {} with provider {}. New channel turns will use this default.",
                        model, provider.name
                    ),
                    Err(e) => format!("Failed to switch model: {}", e),
                }
            } else {
                "That model selection is no longer available. Use /switch-model again.".to_string()
            };

            let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
            let answer_payload = serde_json::json!({
                "callback_query_id": cb.id,
                "text": "Model switch handled"
            });
            let _ = client.post(&answer_url).json(&answer_payload).send().await;

            if let Some(message_id) = inner_msg.message_id {
                let edit_url = telegram_api_url(bot_token, "editMessageText");
                let edit_payload = serde_json::json!({
                    "chat_id": chat_id,
                    "message_id": message_id,
                    "text": result_text
                });
                let _ = client.post(&edit_url).json(&edit_payload).send().await;
            }
        }
        return;
    }

    if action == "resume_cancel" {
        if let Some(ref inner_msg) = cb.message {
            let chat_id = inner_msg.chat.id;
            let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
            let answer_payload = serde_json::json!({
                "callback_query_id": cb.id,
                "text": "Continuing current session"
            });
            let _ = client.post(&answer_url).json(&answer_payload).send().await;

            if let Some(message_id) = inner_msg.message_id {
                let edit_url = telegram_api_url(bot_token, "editMessageText");
                let edit_payload = serde_json::json!({
                    "chat_id": chat_id,
                    "message_id": message_id,
                    "text": "Continuing current session. No session was changed."
                });
                let _ = client.post(&edit_url).json(&edit_payload).send().await;
            }
        }
        return;
    }

    if action == "resume_select" {
        if let Some(ref inner_msg) = cb.message {
            let chat_id = inner_msg.chat.id;
            let idx = callback_value.parse::<usize>().ok();
            let session_key = format!("telegram:{}", chat_id);
            let sessions = crate::channels::list_channel_sessions(
                &agent_loop.session_manager.dir,
                &session_key,
                10,
            );
            let result_text = if let Some(idx) = idx {
                if let Some(item) = sessions.get(idx) {
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
                } else {
                    "That session is no longer available. Use /resume again.".to_string()
                }
            } else {
                "Invalid resume selection. Use /resume again.".to_string()
            };

            let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
            let answer_payload = serde_json::json!({
                "callback_query_id": cb.id,
                "text": "Resume selection handled"
            });
            let _ = client.post(&answer_url).json(&answer_payload).send().await;

            if let Some(message_id) = inner_msg.message_id {
                let edit_url = telegram_api_url(bot_token, "editMessageText");
                let edit_payload = serde_json::json!({
                    "chat_id": chat_id,
                    "message_id": message_id,
                    "text": result_text
                });
                let _ = client.post(&edit_url).json(&edit_payload).send().await;
            }
        }
        return;
    }

    if action == "remote" {
        if let Some(ref inner_msg) = cb.message {
            let chat_id = inner_msg.chat.id;
            let sessions = crate::agent::activity::list_active_tui_sessions();
            let selected = sessions
                .iter()
                .find(|session| session.session_key == callback_value);

            let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
            let answer_payload = if selected.is_some() {
                set_remote_session(chat_id, callback_value.to_string());
                serde_json::json!({
                    "callback_query_id": cb.id,
                    "text": "Remote TUI selected"
                })
            } else {
                serde_json::json!({
                    "callback_query_id": cb.id,
                    "text": "That TUI session is no longer active. Use /remote again.",
                    "show_alert": true
                })
            };
            let _ = client.post(&answer_url).json(&answer_payload).send().await;

            if let Some(message_id) = inner_msg.message_id {
                let edit_url = telegram_api_url(bot_token, "editMessageText");
                let text = selected
                    .map(|session| {
                        format!(
                            "🔌 Remote mode active for {}\nNext messages are forwarded to that TUI. Use /local or /exit to leave remote mode.",
                            remote_session_button_label(session)
                        )
                    })
                    .unwrap_or_else(|| {
                        "That TUI session is no longer active. Use /remote again.".to_string()
                    });
                let edit_payload = serde_json::json!({
                    "chat_id": chat_id,
                    "message_id": message_id,
                    "text": text
                });
                let _ = client.post(&edit_url).json(&edit_payload).send().await;
            }
        }
        return;
    }

    let req_id = callback_value;
    let approved = action == "approve";

    // Resolve wait condition
    let map = APPROVAL_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        if let Some(tx) = guard.remove(req_id) {
            let _ = tx.send(approved);
        }
    }

    // Answer callback query so the Telegram UI stops showing loading indicator
    let answer_url = telegram_api_url(bot_token, "answerCallbackQuery");
    let answer_payload = serde_json::json!({
        "callback_query_id": cb.id,
        "text": if approved { "Action approved ✅" } else { "Action denied ❌" }
    });
    let _ = client.post(&answer_url).json(&answer_payload).send().await;

    // Remove the inline buttons from the original message so they cannot be clicked again
    if let Some(ref inner_msg) = cb.message {
        let chat_id = inner_msg.chat.id;
        if let Some(message_id) = inner_msg.message_id {
            let edit_markup_url = telegram_api_url(bot_token, "editMessageReplyMarkup");
            let edit_payload = serde_json::json!({
                "chat_id": chat_id,
                "message_id": message_id,
                "reply_markup": {
                    "inline_keyboard": [[
                        {
                            "text": if approved { "Approved ✅" } else { "Denied ❌" },
                            "callback_data": "done"
                        }
                    ]]
                }
            });
            let _ = client.post(&edit_markup_url).json(&edit_payload).send().await;
        }
    }
}
