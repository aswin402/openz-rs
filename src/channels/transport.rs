//! Shared outbound HTTP transport for channel notifications.
//!
//! Request construction remains in `notifications.rs`; this module owns the
//! delivery mechanics and redacted error reporting so every caller uses the
//! same HTTP behavior.

use super::notifications::{NotificationAuth, NotificationRequest};
use anyhow::{anyhow, Result};

pub(crate) struct NotificationResponse {
    pub(crate) status: reqwest::StatusCode,
    pub(crate) body: String,
}

pub(crate) async fn send_notification_request(
    client: &reqwest::Client,
    request: &NotificationRequest,
) -> Result<NotificationResponse> {
    let mut builder = client.post(&request.url).json(&request.payload);
    match &request.auth {
        NotificationAuth::None => {}
        NotificationAuth::Bearer(token) => {
            builder = builder.bearer_auth(token);
        }
        NotificationAuth::Header { name, value } => {
            builder = builder.header(*name, value);
        }
    }

    let response = builder.send().await.map_err(|error| anyhow!(error))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    Ok(NotificationResponse { status, body })
}

pub(crate) async fn send_external_notification(
    client: &reqwest::Client,
    request: &NotificationRequest,
) {
    match send_notification_request(client, request).await {
        Ok(response) if response.status.is_success() => {}
        Ok(response) => {
            let error = redact_notification_text(
                request,
                &format!("HTTP {}: {}", response.status, response.body),
            );
            tracing::warn!(
                channel = request.target.channel_name(),
                target = request.target.display_id(),
                error = %error,
                "Error sending external notification"
            );
        }
        Err(err) => {
            let error = redact_notification_text(request, &err.to_string());
            tracing::warn!(
                channel = request.target.channel_name(),
                target = request.target.display_id(),
                error = %error,
                "Error sending external notification"
            );
        }
    }
}

pub(crate) fn redact_notification_text(request: &NotificationRequest, text: &str) -> String {
    let mut redacted = text.replace(&request.url, "<notification endpoint>");
    match &request.auth {
        NotificationAuth::None => {}
        NotificationAuth::Bearer(token) => {
            if !token.is_empty() {
                redacted = redacted.replace(token, "<redacted credential>");
            }
        }
        NotificationAuth::Header { value, .. } => {
            if !value.is_empty() {
                redacted = redacted.replace(value, "<redacted credential>");
            }
        }
    }
    redacted
}
