use std::sync::{Arc, OnceLock};

use super::storage::LogEntry;
use crate::core::secrets::{
    collect_all_environment_and_config_secrets,
    redact_text_with_secrets as core_redact_text_with_secrets,
};

pub static LOG_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<LogEntry>> = OnceLock::new();

static LOG_SECRETS: OnceLock<Arc<Vec<String>>> = OnceLock::new();

/// Load configured and environment-backed credentials into the log scrubber.
/// Values are retained only in memory and are never emitted to logs.
pub fn initialize_secret_redaction(config: &serde_json::Value) -> Arc<Vec<String>> {
    let secrets = collect_all_environment_and_config_secrets(config);
    let shared = Arc::new(secrets);
    let _ = LOG_SECRETS.set(shared.clone());
    shared
}

pub(crate) fn redact_text_with_secrets(text: &str, secrets: &[String]) -> String {
    core_redact_text_with_secrets(text, secrets)
}

pub fn redact_sensitive_text(text: &str) -> String {
    LOG_SECRETS
        .get()
        .map(|secrets| redact_text_with_secrets(text, secrets))
        .unwrap_or_else(|| text.to_string())
}

pub struct SecretScrubWriter<W> {
    inner: W,
    secrets: Arc<Vec<String>>,
}

impl<W> SecretScrubWriter<W> {
    pub fn new(inner: W, secrets: Arc<Vec<String>>) -> Self {
        Self { inner, secrets }
    }
}

impl<W: std::io::Write> std::io::Write for SecretScrubWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let redacted = redact_text_with_secrets(&text, &self.secrets);
        self.inner.write_all(redacted.as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub struct SqliteLogLayer;

impl<S> tracing_subscriber::Layer<S> for SqliteLogLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target().to_string();

        let mut visitor = EventFieldVisitor {
            message: String::new(),
            session: None,
        };
        event.record(&mut visitor);

        let session = visitor
            .session
            .or_else(crate::agent::style::spinner::get_current_session_key);
        let timestamp = chrono::Utc::now().to_rfc3339();

        if let Some(tx) = LOG_TX.get() {
            let _ = tx.send(LogEntry {
                timestamp,
                level,
                target,
                message: redact_sensitive_text(&visitor.message),
                session: session.map(|value| redact_sensitive_text(&value)),
            });
        }
    }
}

struct EventFieldVisitor {
    message: String,
    session: Option<String>,
}

impl tracing::field::Visit for EventFieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let val_str = format!("{:?}", value);
        let cleaned = if val_str.starts_with('"') && val_str.ends_with('"') && val_str.len() >= 2 {
            val_str[1..val_str.len() - 1].to_string()
        } else {
            val_str
        };
        if field.name() == "message" {
            self.message = cleaned;
        } else if field.name() == "session" {
            self.session = Some(cleaned);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else if field.name() == "session" {
            self.session = Some(value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_scrubber_masks_credentials_inside_log_text() {
        let secrets = vec![
            "bot123:token-value".to_string(),
            "provider-secret".to_string(),
        ];
        let redacted = redact_text_with_secrets(
            "curl https://api.telegram.org/bot123:token-value/sendMessage key=provider-secret",
            &secrets,
        );
        assert!(!redacted.contains("bot123:token-value"));
        assert!(!redacted.contains("provider-secret"));
        assert_eq!(redacted.matches("[REDACTED_SECRET]").count(), 2);
    }
}
