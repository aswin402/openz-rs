//! WebSocket attachment quotas, validation, cleanup, and persistence.
//!
//! Attachments are decoded and stored under the OpenZ config directory. The
//! gateway only forwards sanitized file references to the agent loop, and all
//! existing quotas are kept here as the single source of truth.

use serde_json::Value;
use std::path::Path;

pub(crate) const MAX_WS_MESSAGE_SIZE: usize = 40 * 1024 * 1024;
pub(crate) const MAX_ATTACHMENT_COUNT: usize = 8;
pub(crate) const MAX_ATTACHMENT_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_ATTACHMENT_TOTAL_BYTES: usize = 24 * 1024 * 1024;
pub(crate) const ATTACHMENT_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);
pub(crate) const ATTACHMENT_ALLOWED_MIME_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/bmp",
    "image/tiff",
    "image/svg+xml",
    "application/pdf",
    "text/plain",
    "text/markdown",
    "text/csv",
    "application/json",
    "application/xml",
    "text/xml",
    "application/msword",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
];

pub(crate) fn attachment_mime_allowed(mime: &str) -> bool {
    let mime = mime.trim().to_ascii_lowercase();
    if mime.is_empty() || mime.len() > 128 || mime.chars().any(char::is_control) {
        return false;
    }
    ATTACHMENT_ALLOWED_MIME_TYPES.contains(&mime.as_str())
}

pub(crate) fn attachment_total_within_quota(current: usize, next: usize) -> bool {
    current <= MAX_ATTACHMENT_TOTAL_BYTES
        && next <= MAX_ATTACHMENT_TOTAL_BYTES.saturating_sub(current)
}

pub(crate) fn sanitize_attachment_name(raw_name: &str) -> Option<String> {
    let clean_name: String = raw_name
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' '))
        .take(64)
        .collect::<String>()
        .trim()
        .to_string();
    if clean_name.is_empty() || clean_name == "." || clean_name == ".." {
        None
    } else {
        Some(clean_name)
    }
}

async fn cleanup_stale_attachments(attach_dir: &Path) {
    let Ok(mut entries) = tokio::fs::read_dir(attach_dir).await else {
        return;
    };
    let now = std::time::SystemTime::now();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(metadata) = entry.metadata().await else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if now.duration_since(modified).unwrap_or_default() > ATTACHMENT_TTL {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

/// Persist base64 attachment payloads sent by the WebUI and return sanitized
/// markdown references for the outgoing agent message.
pub(crate) async fn persist_attachments(attachments: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    let Some(arr) = attachments.as_array() else {
        return refs;
    };
    if arr.is_empty() {
        return refs;
    }
    let attach_dir = crate::config::loader::config_dir().join("attachments");
    if tokio::fs::create_dir_all(&attach_dir).await.is_err() {
        return refs;
    }
    cleanup_stale_attachments(&attach_dir).await;
    use base64::{engine::general_purpose, Engine as _};
    let mut total_bytes = 0usize;
    for att in arr.iter().take(MAX_ATTACHMENT_COUNT) {
        let Some(data_b64) = att.get("data").and_then(|v| v.as_str()) else {
            continue;
        };
        if data_b64.len() > (MAX_ATTACHMENT_BYTES * 4 / 3 + 4) {
            continue;
        }
        let Ok(bytes) = general_purpose::STANDARD.decode(data_b64) else {
            continue;
        };
        if bytes.is_empty()
            || bytes.len() > MAX_ATTACHMENT_BYTES
            || !attachment_total_within_quota(total_bytes, bytes.len())
        {
            continue;
        }
        let mime = att
            .get("mime")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !attachment_mime_allowed(mime) {
            continue;
        }
        let raw_name = att
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let Some(clean_name) = sanitize_attachment_name(raw_name) else {
            continue;
        };
        let mime = mime.trim().to_ascii_lowercase();
        let file_name = format!("{}_{}", &uuid::Uuid::new_v4().to_string()[..8], clean_name);
        let path = attach_dir.join(&file_name);
        if tokio::fs::write(&path, &bytes).await.is_err() {
            continue;
        }
        total_bytes += bytes.len();
        let link = format!("file://{}", path.to_string_lossy());
        if mime.starts_with("image/") {
            refs.push(format!("![]({link})"));
        } else {
            refs.push(format!("📎 [{}]({link})", clean_name));
        }
    }
    refs
}
