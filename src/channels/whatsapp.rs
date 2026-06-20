use crate::agent::AgentLoop;
use std::sync::Arc;
use reqwest::Client;
use std::sync::OnceLock;
use std::collections::HashMap;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use tokio::net::TcpListener;

static WHATSAPP_BOT_INFO: OnceLock<(String, String, Client)> = OnceLock::new();

pub fn get_whatsapp_bot_info() -> Option<(String, String, Client)> {
    WHATSAPP_BOT_INFO.get().cloned()
}

pub struct WhatsAppChannel {
    api_key: String,
    phone_number_id: String,
    agent_loop: Arc<AgentLoop>,
    client: Client,
}

#[derive(Clone)]
struct WhatsAppState {
    agent_loop: Arc<AgentLoop>,
    api_key: String,
    phone_number_id: String,
    verify_token: String,
    app_secret: String,
    client: Client,
}

impl WhatsAppChannel {
    pub fn new(api_key: String, phone_number_id: String, agent_loop: AgentLoop) -> Self {
        WhatsAppChannel {
            api_key,
            phone_number_id,
            agent_loop: Arc::new(agent_loop),
            client: Client::builder().use_rustls_tls().build().unwrap_or_default(),
        }
    }
}

#[async_trait::async_trait]
impl super::Channel for WhatsAppChannel {
    fn name(&self) -> &'static str {
        "whatsapp"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let _ = WHATSAPP_BOT_INFO.set((self.api_key.clone(), self.phone_number_id.clone(), self.client.clone()));
        let silent = std::env::var("OPENZ_SILENT").is_ok();
        if self.api_key.is_empty() || self.phone_number_id.is_empty() {
            if !silent {
                println!("⚠️ WhatsApp configurations are incomplete. WhatsApp channel deactivated.");
            }
            return Ok(());
        }

        let session_dir = self.agent_loop.session_manager.dir.clone();
        
        // Send Active message to all active targets at startup
        let targets = crate::channels::get_active_session_targets(&session_dir, "whatsapp_");
        let active_msg = crate::channels::select_random_message(crate::channels::ACTIVE_MESSAGES);
        for phone_number in &targets {
            let send_url = format!("https://graph.facebook.com/v18.0/{}/messages", self.phone_number_id);
            let payload = serde_json::json!({
                "messaging_product": "whatsapp",
                "recipient_type": "individual",
                "to": phone_number,
                "type": "text",
                "text": {
                    "body": active_msg
                }
            });
            let _ = self.client.post(&send_url)
                .bearer_auth(&self.api_key)
                .json(&payload)
                .send()
                .await;
        }

        // Expose Axum webhook receiver on webhook_port (defaulting to 8090)
        let (port, verify_token) = if let Some(ref wa_cfg) = self.agent_loop.config.channels.whatsapp {
            (wa_cfg.webhook_port, wa_cfg.verify_token.clone())
        } else {
            (8090, "openz".to_string())
        };

        let port = std::env::var("WHATSAPP_WEBHOOK_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(port);
        let verify_token = std::env::var("WHATSAPP_WEBHOOK_VERIFY_TOKEN")
            .unwrap_or(verify_token);
        let app_secret = std::env::var("WHATSAPP_APP_SECRET")
            .unwrap_or_default();

        let state = WhatsAppState {
            agent_loop: self.agent_loop.clone(),
            api_key: self.api_key.clone(),
            phone_number_id: self.phone_number_id.clone(),
            verify_token,
            app_secret,
            client: self.client.clone(),
        };

        let app = Router::new()
            .route("/webhook/whatsapp", get(verify_webhook).post(receive_webhook))
            .with_state(state);

        let addr = format!("0.0.0.0:{}", port);
        if !silent {
            println!("🤖 WhatsApp Channel webhook server started on http://{}/webhook/whatsapp", addr);
        }

        let listener = TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

async fn verify_webhook(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<WhatsAppState>,
) -> impl IntoResponse {
    let mode = params.get("hub.mode");
    let token = params.get("hub.verify_token");
    let challenge = params.get("hub.challenge");

    if let (Some(mode), Some(token), Some(challenge)) = (mode, token, challenge) {
        if mode == "subscribe" && token == &state.verify_token {
            return (StatusCode::OK, challenge.clone()).into_response();
        }
    }
    (StatusCode::FORBIDDEN, "Verification failed").into_response()
}

async fn receive_webhook(
    State(state): State<WhatsAppState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Verify HMAC signature if app_secret is configured
    if !state.app_secret.is_empty() {
        let body_bytes = serde_json::to_vec(&payload).unwrap_or_default();
        if let Some(signature_header) = headers.get("X-Hub-Signature-256") {
            if let Ok(sig_str) = signature_header.to_str() {
                // Format: "sha256=<hex>"
                let expected_sig = sig_str.strip_prefix("sha256=").unwrap_or(sig_str);
                let computed = {
                    use hmac::{Hmac, Mac};
                    use sha2::Sha256;
                    type HmacSha256 = Hmac<Sha256>;
                    let mut mac = HmacSha256::new_from_slice(state.app_secret.as_bytes())
                        .expect("HMAC can take key of any size");
                    mac.update(&body_bytes);
                    let result = mac.finalize();
                    hex::encode(result.into_bytes())
                };
                if computed != expected_sig {
                    tracing::warn!("WhatsApp webhook signature mismatch — rejecting request");
                    return StatusCode::FORBIDDEN.into_response();
                }
            } else {
                return StatusCode::FORBIDDEN.into_response();
            }
        } else {
            tracing::warn!("WhatsApp webhook missing X-Hub-Signature-256 header");
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    if let Some(entry) = payload.get("entry").and_then(|e| e.as_array()).and_then(|a| a.first()) {
        if let Some(change) = entry.get("changes").and_then(|c| c.as_array()).and_then(|a| a.first()) {
            if let Some(val) = change.get("value") {
                if let Some(messages) = val.get("messages").and_then(|m| m.as_array()) {
                    for msg in messages {
                        if let Some(from) = msg.get("from").and_then(|f| f.as_str()) {
                            if let Some(body) = msg.get("text").and_then(|t| t.get("body")).and_then(|b| b.as_str()) {
                                let agent = state.agent_loop.clone();
                                let api_key = state.api_key.clone();
                                let phone_number_id = state.phone_number_id.clone();
                                let client = state.client.clone();
                                let text = body.to_string();
                                let from_str = from.to_string();

                                tokio::spawn(async move {
                                    let session_key = format!("whatsapp:{}", from_str);
                                    let run_res = agent.run(&text, &session_key).await;
                                    
                                    let body_text = match run_res {
                                        Ok(res) => res.content,
                                        Err(e) => format!("Error processing request: {}", e),
                                    };

                                    let send_url = format!("https://graph.facebook.com/v18.0/{}/messages", phone_number_id);
                                    let reply_payload = serde_json::json!({
                                        "messaging_product": "whatsapp",
                                        "recipient_type": "individual",
                                        "to": from_str,
                                        "type": "text",
                                        "text": {
                                            "body": body_text
                                        }
                                    });
                                    let _ = client.post(&send_url)
                                        .bearer_auth(&api_key)
                                        .json(&reply_payload)
                                        .send()
                                        .await;
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    StatusCode::OK.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn test_verify_webhook_success() {
        let mut params = HashMap::new();
        params.insert("hub.mode".to_string(), "subscribe".to_string());
        params.insert("hub.verify_token".to_string(), "my_test_token".to_string());
        params.insert("hub.challenge".to_string(), "12345".to_string());

        let mut config = crate::config::schema::Config::default();
        config.agents.defaults.provider = "ollama".to_string();
        config.agents.defaults.model = "ollama/llama3".to_string();
        let agent_loop = crate::cli::build_agent_loop(config).await.unwrap();

        let state = WhatsAppState {
            agent_loop: Arc::new(agent_loop),
            api_key: "key".to_string(),
            phone_number_id: "phone".to_string(),
            verify_token: "my_test_token".to_string(),
            app_secret: String::new(),
            client: reqwest::Client::new(),
        };

        let response = verify_webhook(Query(params), State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body_bytes = axum::body::to_bytes(response.into_body(), 1000).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert_eq!(body_str, "12345");
    }

    #[tokio::test]
    async fn test_verify_webhook_failure() {
        let mut params = HashMap::new();
        params.insert("hub.mode".to_string(), "subscribe".to_string());
        params.insert("hub.verify_token".to_string(), "wrong_token".to_string());
        params.insert("hub.challenge".to_string(), "12345".to_string());

        let mut config = crate::config::schema::Config::default();
        config.agents.defaults.provider = "ollama".to_string();
        config.agents.defaults.model = "ollama/llama3".to_string();
        let agent_loop = crate::cli::build_agent_loop(config).await.unwrap();

        let state = WhatsAppState {
            agent_loop: Arc::new(agent_loop),
            api_key: "key".to_string(),
            phone_number_id: "phone".to_string(),
            verify_token: "my_test_token".to_string(),
            app_secret: String::new(),
            client: reqwest::Client::new(),
        };

        let response = verify_webhook(Query(params), State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
