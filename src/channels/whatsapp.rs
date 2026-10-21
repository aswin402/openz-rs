use crate::agent::AgentLoop;
use crate::channels::notifications::{chunk_message, whatsapp_message_url};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::net::TcpListener;

const WHATSAPP_MAX_MESSAGE_BYTES: usize = 65536;

static WHATSAPP_BOT_INFO: OnceLock<(String, String, Client)> = OnceLock::new();

pub fn get_whatsapp_bot_info() -> Option<(String, String, Client)> {
    WHATSAPP_BOT_INFO.get().cloned()
}

pub struct WhatsAppChannel {
    api_key: String,
    phone_number_id: String,
    agent_loop: Arc<AgentLoop>,
    client: Client,
    concurrency_limit: Arc<tokio::sync::Semaphore>,
}

#[derive(Clone)]
struct WhatsAppState {
    agent_loop: Arc<AgentLoop>,
    api_key: String,
    phone_number_id: String,
    verify_token: String,
    app_secret: String,
    client: Client,
    concurrency_limit: Arc<tokio::sync::Semaphore>,
}

impl WhatsAppChannel {
    pub fn new(api_key: String, phone_number_id: String, agent_loop: AgentLoop) -> Self {
        Self::with_shared_loop(api_key, phone_number_id, Arc::new(agent_loop))
    }

    pub fn with_shared_loop(
        api_key: String,
        phone_number_id: String,
        agent_loop: Arc<AgentLoop>,
    ) -> Self {
        WhatsAppChannel {
            api_key,
            phone_number_id,
            agent_loop,
            client: crate::core::http::custom_http_client(
                std::time::Duration::from_secs(5),
                std::time::Duration::from_secs(15),
            ),
            concurrency_limit: crate::channels::channel_concurrency_semaphore(),
        }
    }
}

async fn send_whatsapp_chunks(
    client: &Client,
    api_key: &str,
    phone_number_id: &str,
    recipient: &str,
    text: &str,
) {
    let send_url = whatsapp_message_url(phone_number_id);
    for chunk in chunk_message(text, WHATSAPP_MAX_MESSAGE_BYTES) {
        let reply_payload = serde_json::json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": recipient,
            "type": "text",
            "text": { "body": chunk }
        });
        let _ = client
            .post(&send_url)
            .bearer_auth(api_key)
            .json(&reply_payload)
            .send()
            .await;
    }
}

#[async_trait::async_trait]
impl super::Channel for WhatsAppChannel {
    fn name(&self) -> &'static str {
        "whatsapp"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let _ = WHATSAPP_BOT_INFO.set((
            self.api_key.clone(),
            self.phone_number_id.clone(),
            self.client.clone(),
        ));
        let silent = std::env::var("OPENZ_SILENT").is_ok();
        let incomplete = self.api_key.is_empty() || self.phone_number_id.is_empty();
        match (incomplete, silent) {
            (true, false) => {
                println!(
                    "⚠️ WhatsApp configurations are incomplete. WhatsApp channel deactivated."
                );
                return Ok(());
            }
            (true, true) => return Ok(()),
            (false, _) => {}
        }

        let session_dir = self.agent_loop.session_manager.dir.clone();

        let targets = crate::channels::get_active_session_targets(&session_dir, "whatsapp_");
        let active_msg = crate::channels::select_random_message(crate::channels::ACTIVE_MESSAGES);
        for phone_number in &targets {
            send_whatsapp_chunks(
                &self.client,
                &self.api_key,
                &self.phone_number_id,
                phone_number,
                &active_msg,
            )
            .await;
        }

        let (port, verify_token) = match &self.agent_loop.config.channels.whatsapp {
            Some(wa_cfg) => (wa_cfg.webhook_port, wa_cfg.verify_token.clone()),
            None => (8090, String::new()),
        };

        let port = std::env::var("WHATSAPP_WEBHOOK_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(port);
        let verify_token = std::env::var("WHATSAPP_WEBHOOK_VERIFY_TOKEN").unwrap_or(verify_token);
        match (verify_token.trim().is_empty(), silent) {
            (true, false) => {
                println!(
                    "⚠️ WhatsApp webhook verify token is not configured. WhatsApp channel deactivated."
                );
                return Ok(());
            }
            (true, true) => return Ok(()),
            (false, _) => {}
        }
        let app_secret = std::env::var("WHATSAPP_APP_SECRET").unwrap_or_default();

        let state = WhatsAppState {
            agent_loop: self.agent_loop.clone(),
            api_key: self.api_key.clone(),
            phone_number_id: self.phone_number_id.clone(),
            verify_token,
            app_secret,
            client: self.client.clone(),
            concurrency_limit: self.concurrency_limit.clone(),
        };

        let app = Router::new()
            .route(
                "/webhook/whatsapp",
                get(verify_webhook).post(receive_webhook),
            )
            .with_state(state);

        let addr = format!("127.0.0.1:{}", port);
        match silent {
            false => println!(
                "🤖 WhatsApp Channel webhook server started on http://{}/webhook/whatsapp",
                addr
            ),
            true => {}
        }

        let mut shutdown_rx = match crate::shutdown::receiver() {
            Some(rx) => rx,
            None => {
                let (_, rx) = tokio::sync::watch::channel(false);
                rx
            }
        };

        let listener = TcpListener::bind(&addr).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let already_shutting_down = *shutdown_rx.borrow();
                match already_shutting_down {
                    true => {}
                    false => {
                        let _ = shutdown_rx.changed().await;
                    }
                }
            })
            .await?;

        Ok(())
    }
}

async fn verify_webhook(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<WhatsAppState>,
) -> impl IntoResponse {
    let mode = params.get("hub.mode").map(String::as_str);
    let token = params.get("hub.verify_token").map(String::as_str);
    let challenge = params.get("hub.challenge");

    match (mode, token == Some(state.verify_token.as_str()), challenge) {
        (Some("subscribe"), true, Some(challenge)) => {
            (StatusCode::OK, challenge.clone()).into_response()
        }
        _ => (StatusCode::FORBIDDEN, "Verification failed").into_response(),
    }
}

fn verify_whatsapp_signature(app_secret: &str, headers: &HeaderMap, body: &[u8]) -> bool {
    match app_secret.is_empty() {
        true => true,
        false => {
            let sig_str = match headers
                .get("X-Hub-Signature-256")
                .and_then(|header| header.to_str().ok())
            {
                Some(sig) => sig,
                None => {
                    tracing::warn!("WhatsApp webhook missing or invalid X-Hub-Signature-256 header");
                    return false;
                }
            };
            let expected_sig = sig_str.strip_prefix("sha256=").unwrap_or(sig_str);
            let computed = {
                use hmac::{Hmac, Mac};
                use sha2::Sha256;
                type HmacSha256 = Hmac<Sha256>;
                let mut mac = HmacSha256::new_from_slice(app_secret.as_bytes())
                    .expect("HMAC can take key of any size");
                mac.update(body);
                hex::encode(mac.finalize().into_bytes())
            };
            let matched = super::secure_compare(&computed, expected_sig);
            match matched {
                true => true,
                false => {
                    tracing::warn!("WhatsApp webhook signature mismatch — rejecting request");
                    false
                }
            }
        }
    }
}

async fn process_whatsapp_incoming_message(state: &WhatsAppState, from_str: String, text: String) {
    let agent = state.agent_loop.clone();
    let api_key = state.api_key.clone();
    let phone_number_id = state.phone_number_id.clone();
    let client = state.client.clone();

    let whatsapp_session_key = format!("whatsapp:{}", from_str);
    match crate::channels::intercept_channel_text_command(
        &agent.session_manager,
        &whatsapp_session_key,
        &text,
        "▲ Stop requested. Active OpenZ turn interrupted.",
    )
    .await
    {
        Some(response_text) => {
            send_whatsapp_chunks(
                &client,
                &api_key,
                &phone_number_id,
                &from_str,
                &response_text,
            )
            .await;
            return;
        }
        None => {}
    }

    let concurrency_limit = state.concurrency_limit.clone();
    tokio::spawn(async move {
        let _permit = match concurrency_limit.acquire().await {
            Ok(p) => p,
            Err(_) => return,
        };
        let session_key = format!("whatsapp:{}", from_str);
        let run_res = agent.run(&text, &session_key).await;

        let body_text = match run_res {
            Ok(res) => res.content,
            Err(e) => format!("Error processing request: {}", e),
        };

        send_whatsapp_chunks(
            &client,
            &api_key,
            &phone_number_id,
            &from_str,
            &body_text,
        )
        .await;
    });
}

async fn receive_webhook(
    State(state): State<WhatsAppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    match verify_whatsapp_signature(&state.app_secret, &headers, &body) {
        true => {}
        false => return StatusCode::FORBIDDEN.into_response(),
    }

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("WhatsApp webhook body parse error: {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let messages = payload
        .get("entry")
        .and_then(|e| e.as_array())
        .and_then(|a| a.first())
        .and_then(|entry| entry.get("changes"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|change| change.get("value"))
        .and_then(|val| val.get("messages"))
        .and_then(|m| m.as_array());

    match messages {
        Some(messages) => {
            for msg in messages {
                let from = msg.get("from").and_then(|f| f.as_str());
                let body_text = msg
                    .get("text")
                    .and_then(|t| t.get("body"))
                    .and_then(|b| b.as_str());
                match (from, body_text) {
                    (Some(from), Some(body_text)) => {
                        process_whatsapp_incoming_message(
                            &state,
                            from.to_string(),
                            body_text.to_string(),
                        )
                        .await;
                    }
                    _ => {}
                }
            }
        }
        None => {}
    }
    StatusCode::OK.into_response()
}

#[cfg(test)]
#[path = "whatsapp_tests.rs"]
mod tests;
