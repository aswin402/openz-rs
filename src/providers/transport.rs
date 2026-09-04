//! Shared HTTP mechanics for provider implementations.
//!
//! Request and response schemas remain in each provider module. This module
//! owns only the common client, endpoint, authentication, and retry behavior.

use crate::providers::circuit_breaker::{retry_with_backoff, CircuitBreaker};
use anyhow::Result;
use reqwest::{Client, Response};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderHttpConfig {
    pub(crate) connect_timeout: Duration,
    pub(crate) read_timeout: Duration,
    pub(crate) request_timeout: Duration,
}

impl Default for ProviderHttpConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(15),
            read_timeout: Duration::from_secs(120),
            request_timeout: Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderAuth {
    Bearer,
    Azure,
    Anthropic,
}

pub(crate) fn build_provider_client() -> Client {
    build_provider_client_with_config(ProviderHttpConfig::default())
}

pub(crate) fn build_provider_client_with_config(config: ProviderHttpConfig) -> Client {
    Client::builder()
        .use_rustls_tls()
        .connect_timeout(config.connect_timeout)
        .read_timeout(config.read_timeout)
        .timeout(config.request_timeout)
        .build()
        .unwrap_or_default()
}

pub(crate) fn openai_chat_endpoint(api_base: &str) -> String {
    if api_base.contains("/openai/deployments") || api_base.contains("azure") {
        api_base.to_string()
    } else {
        format!("{}/chat/completions", api_base.trim_end_matches('/'))
    }
}

pub(crate) fn anthropic_messages_endpoint(api_base: &str) -> String {
    let clean_base = api_base.trim_end_matches('/').trim_end_matches("/v1");
    format!("{clean_base}/v1/messages")
}

pub(crate) async fn post_json_with_retry(
    client: &Client,
    breaker: &CircuitBreaker,
    provider_name: &'static str,
    url: &str,
    api_key: &str,
    body: Value,
    auth: ProviderAuth,
) -> Result<Response> {
    let client = client.clone();
    let url = url.to_string();
    let api_key = api_key.to_string();

    retry_with_backoff(
        breaker,
        3,
        Duration::from_secs(1),
        Duration::from_secs(30),
        provider_name,
        move || {
            let client = client.clone();
            let url = url.clone();
            let api_key = api_key.clone();
            let body = body.clone();
            async move {
                let mut request = client.post(&url);
                request = match auth {
                    ProviderAuth::Bearer => request.bearer_auth(&api_key),
                    ProviderAuth::Azure => request.header("api-key", &api_key),
                    ProviderAuth::Anthropic => request
                        .header("x-api-key", &api_key)
                        .header("anthropic-version", "2023-06-01")
                        .header("anthropic-beta", "prompt-caching-2024-07-31"),
                };

                let response = request
                    .json(&body)
                    .send()
                    .await
                    .map_err(|error| (0u16, format!("Network error: {error}")))?;
                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let error_text = response.text().await.unwrap_or_default();
                    Err((status, error_text))
                } else {
                    Ok(response)
                }
            }
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_http_config_preserves_timeout_defaults() {
        assert_eq!(
            ProviderHttpConfig::default(),
            ProviderHttpConfig {
                connect_timeout: Duration::from_secs(15),
                read_timeout: Duration::from_secs(120),
                request_timeout: Duration::from_secs(300),
            }
        );
    }

    #[test]
    fn provider_http_transport_preserves_endpoints() {
        assert_eq!(
            openai_chat_endpoint("https://api.openai.com/v1/"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            openai_chat_endpoint("https://example.openai.azure.com/openai/deployments/model"),
            "https://example.openai.azure.com/openai/deployments/model"
        );
        assert_eq!(
            anthropic_messages_endpoint("https://api.anthropic.com/"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            anthropic_messages_endpoint("https://proxy.example/v1/"),
            "https://proxy.example/v1/messages"
        );
    }
}
