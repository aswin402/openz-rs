//! Shared HTTP mechanics for provider implementations.
//!
//! Request and response schemas remain in each provider module. This module
//! owns only the common client, endpoint, authentication, and retry behavior.

use crate::providers::circuit_breaker::{retry_with_backoff_hint, CircuitBreaker};
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
    crate::core::http::http_client_with_read_timeout(
        config.connect_timeout,
        config.read_timeout,
        config.request_timeout,
    )
}

pub(crate) fn openai_chat_endpoint(api_base: &str) -> String {
    match api_base.contains("/openai/deployments") || api_base.contains("azure") {
        true => api_base.to_string(),
        false => format!("{}/chat/completions", api_base.trim_end_matches('/')),
    }
}

pub(crate) fn anthropic_messages_endpoint(api_base: &str) -> String {
    let clean_base = api_base.trim_end_matches('/').trim_end_matches("/v1");
    format!("{clean_base}/v1/messages")
}

fn parse_retry_after_header(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let ms = headers
        .get("retry-after-ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(Duration::from_millis);
    ms.or_else(|| {
        headers
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|&secs| secs > 0.0 && secs <= 300.0)
            .map(Duration::from_secs_f64)
    })
}

pub(crate) async fn post_json_with_retry(
    client: &Client,
    breaker: &CircuitBreaker,
    provider_name: &str,
    url: &str,
    api_key: &str,
    body: Value,
    auth: ProviderAuth,
) -> Result<Response> {
    let client = client.clone();
    let url = url.to_string();
    let api_key = api_key.to_string();

    retry_with_backoff_hint(
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
                    .map_err(|error| (0u16, format!("Network error: {error}"), None))?;
                match response.status().is_success() {
                    true => Ok(response),
                    false => {
                        let status = response.status().as_u16();
                        let retry_after = parse_retry_after_header(response.headers());
                        let error_text = response.text().await.unwrap_or_default();
                        Err((status, error_text, retry_after))
                    }
                }
            }
        },
    )
    .await
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
