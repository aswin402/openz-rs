use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use super::query::classify_log_category;
use super::storage::LogEntry;
use crate::core::secrets::collect_all_environment_and_config_secrets;
pub(crate) use crate::core::secrets::redact_text_with_secrets;

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

#[derive(Default, Clone)]
struct SpanContextFields {
    session: Option<String>,
    job_id: Option<String>,
}

struct SpanVisitor {
    fields: SpanContextFields,
}

impl tracing::field::Visit for SpanVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let raw = format!("{:?}", value);
        let cleaned = raw
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .map(ToString::to_string)
            .unwrap_or(raw);
        match field.name() {
            "session" => self.fields.session = Some(cleaned),
            "job_id" => self.fields.job_id = Some(cleaned),
            _ => {}
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "session" => self.fields.session = Some(value.to_string()),
            "job_id" => self.fields.job_id = Some(value.to_string()),
            _ => {}
        }
    }
}

pub struct SqliteLogLayer;

impl<S> tracing_subscriber::Layer<S> for SqliteLogLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = SpanVisitor {
            fields: SpanContextFields::default(),
        };
        attrs.record(&mut visitor);
        let _ = ctx.span(id).map(|span| {
            span.extensions_mut().insert(visitor.fields);
        });
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let _ = ctx.span(id).map(|span| {
            let mut ext = span.extensions_mut();
            let current = ext.remove::<SpanContextFields>().unwrap_or_default();
            let mut visitor = SpanVisitor { fields: current };
            values.record(&mut visitor);
            ext.insert(visitor.fields);
        });
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target().to_string();

        let mut visitor = EventFieldVisitor::default();
        event.record(&mut visitor);

        let mut span_session: Option<String> = None;
        let mut span_job_id: Option<String> = None;
        let _ = ctx.event_scope(event).map(|scope| {
            for span in scope {
                let ext = span.extensions();
                let _ = ext.get::<SpanContextFields>().map(|sf| {
                    let _ = span_session
                        .is_none()
                        .then(|| sf.session.clone())
                        .flatten()
                        .map(|s| span_session = Some(s));
                    let _ = span_job_id
                        .is_none()
                        .then(|| sf.job_id.clone())
                        .flatten()
                        .map(|j| span_job_id = Some(j));
                });
            }
        });

        let job_id_fallback = visitor
            .extra
            .get("job_id")
            .cloned()
            .or(span_job_id)
            .map(|j| format!("cron:{j}"));

        let enriched_message = visitor.compose_enriched_message();
        let session = visitor
            .session
            .or(span_session)
            .or(job_id_fallback)
            .or_else(crate::agent::style::spinner::get_current_session_key);
        let redacted_message = redact_sensitive_text(&enriched_message);
        let redacted_session = session.map(|value| redact_sensitive_text(&value));
        let category =
            classify_log_category(&target, redacted_session.as_deref(), &redacted_message)
                .as_str()
                .to_string();
        let fields_json = (!visitor.extra.is_empty())
            .then(|| serde_json::to_string(&visitor.extra).ok())
            .flatten()
            .map(|raw| redact_sensitive_text(&raw));
        let timestamp = chrono::Utc::now().to_rfc3339();

        match LOG_TX.get() {
            Some(tx) => {
                let _ = tx.send(LogEntry {
                    timestamp,
                    level,
                    target,
                    category,
                    message: redacted_message,
                    session: redacted_session,
                    fields_json,
                });
            }
            None => {}
        }
    }
}

#[derive(Default)]
struct EventFieldVisitor {
    message: String,
    session: Option<String>,
    extra: BTreeMap<String, String>,
}

fn truncate_field_value(value: &str, max_chars: usize) -> String {
    let single_line = value.replace('\n', "\\n").replace('\r', "");
    match single_line.chars().count() > max_chars {
        true => {
            let prefix: String = single_line.chars().take(max_chars).collect();
            format!("{prefix}...")
        }
        false => single_line,
    }
}

impl EventFieldVisitor {
    fn insert_field(&mut self, name: &str, cleaned: String) {
        match name {
            "message" => self.message = cleaned,
            "session" => self.session = Some(cleaned),
            other => {
                self.extra
                    .insert(other.to_string(), truncate_field_value(&cleaned, 280));
            }
        }
    }

    fn compose_enriched_message(&self) -> String {
        match self.extra.is_empty() {
            true => self.message.clone(),
            false => {
                let mut pairs = Vec::with_capacity(self.extra.len());
                for (k, v) in &self.extra {
                    let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
                    let formatted_val = match v.contains(' ') || v.contains('"') || v.contains('{') {
                        true => format!("\"{escaped}\""),
                        false => v.clone(),
                    };
                    pairs.push(format!("{k}={formatted_val}"));
                }
                let suffix = pairs.join(" ");
                match self.message.is_empty() {
                    true => suffix,
                    false => format!("{}  {}", self.message, suffix),
                }
            }
        }
    }
}

impl tracing::field::Visit for EventFieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let val_str = format!("{:?}", value);
        let cleaned = val_str
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .map(ToString::to_string)
            .unwrap_or(val_str);
        self.insert_field(field.name(), cleaned);
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.insert_field(field.name(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.insert_field(field.name(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.insert_field(field.name(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.insert_field(field.name(), value.to_string());
    }
}

#[cfg(test)]
#[path = "subscriber_tests.rs"]
mod tests;
