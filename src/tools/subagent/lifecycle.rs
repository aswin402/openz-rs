use super::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentRunStatus {
    Queued,
    Running,
    Fallback {
        model: String,
        attempt: usize,
        total: usize,
    },
    Cancelling,
    Cancelled,
    TimedOut {
        duration_secs: Option<u64>,
    },
    Failed {
        error: String,
    },
    Completed,
}

impl SubagentRunStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Queued => "queued".to_string(),
            Self::Running => "running".to_string(),
            Self::Fallback {
                model,
                attempt,
                total,
            } => format!("fallback {attempt}/{total}: {model}"),
            Self::Cancelling => "cancelling".to_string(),
            Self::Cancelled => "cancelled".to_string(),
            Self::TimedOut { duration_secs } => match duration_secs {
                Some(secs) => format!("timed out after {secs}s"),
                None => "timed out".to_string(),
            },
            Self::Failed { error } => format!("failed: {error}"),
            Self::Completed => "completed".to_string(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Fallback { .. } => "fallback",
            Self::Cancelling => "cancelling",
            Self::Cancelled => "cancelled",
            Self::TimedOut { .. } => "timed_out",
            Self::Failed { .. } => "failed",
            Self::Completed => "completed",
        }
    }
}

pub fn classify_subagent_error(error: &str, token: &CancellationToken) -> SubagentRunStatus {
    if token.is_cancelled() {
        return SubagentRunStatus::Cancelled;
    }

    let lower = error.to_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") {
        SubagentRunStatus::TimedOut {
            duration_secs: parse_timeout_duration_secs(error),
        }
    } else if lower.contains("cancelled") || lower.contains("canceled") {
        SubagentRunStatus::Cancelled
    } else {
        SubagentRunStatus::Failed {
            error: error.to_string(),
        }
    }
}

fn parse_timeout_duration_secs(error: &str) -> Option<u64> {
    let lower = error.to_lowercase();
    let searchable = lower
        .find("after")
        .map(|idx| &lower[idx + "after".len()..])
        .unwrap_or(&lower);
    let mut pending_number = None;

    for token in searchable.split(|ch: char| {
        ch.is_whitespace() || matches!(ch, ',' | '.' | ':' | ';' | '=' | '(' | ')')
    }) {
        if token.is_empty() {
            continue;
        }

        let digit_len = token
            .char_indices()
            .take_while(|(_, ch)| ch.is_ascii_digit())
            .map(|(idx, ch)| idx + ch.len_utf8())
            .last()
            .unwrap_or(0);

        if digit_len > 0 {
            let value = token[..digit_len].parse::<u64>().ok()?;
            let unit = &token[digit_len..];
            if let Some(secs) = duration_unit_to_secs(value, unit) {
                return Some(secs);
            }
            pending_number = Some(value);
            continue;
        }

        if let Some(value) = pending_number.take() {
            if let Some(secs) = duration_unit_to_secs(value, token) {
                return Some(secs);
            }
        }
    }

    None
}

fn duration_unit_to_secs(value: u64, unit: &str) -> Option<u64> {
    let unit = unit.trim();
    if unit.is_empty() {
        return None;
    }
    if matches!(unit, "s" | "sec" | "secs") || unit.starts_with("second") {
        Some(value)
    } else if matches!(unit, "m" | "min" | "mins") || unit.starts_with("minute") {
        Some(value.saturating_mul(60))
    } else if matches!(unit, "h" | "hr" | "hrs") || unit.starts_with("hour") {
        Some(value.saturating_mul(3600))
    } else {
        None
    }
}

pub fn compact_lifecycle_line(name: &str, model: &str, status: &SubagentRunStatus) -> String {
    let clean_name = name.trim();
    let clean_model = model.trim();
    let label = status.label();

    if clean_model.is_empty() {
        format!("{clean_name} | {label}")
    } else {
        format!("{clean_name} | {clean_model} | {label}")
    }
}

pub fn status_json(status: &SubagentRunStatus) -> serde_json::Value {
    let mut value = serde_json::json!({
        "code": status.code(),
        "label": status.label(),
    });

    if let SubagentRunStatus::TimedOut {
        duration_secs: Some(secs),
    } = status
    {
        if let Some(map) = value.as_object_mut() {
            map.insert("durationSecs".to_string(), serde_json::json!(secs));
        }
    }

    value
}

pub fn cancellation_result_json(
    tool_name: &str,
    subagent_name: Option<&str>,
    session_id: &str,
    model_used: &str,
    error: &str,
) -> serde_json::Value {
    let mut value = serde_json::json!({
        "status": "cancelled",
        "lifecycle": status_json(&SubagentRunStatus::Cancelled),
        "tool": tool_name,
        "session_id": session_id,
        "model_used": model_used,
        "error": error,
    });

    if let Some(name) = subagent_name {
        if let Some(map) = value.as_object_mut() {
            map.insert(
                "subagent".to_string(),
                serde_json::Value::String(name.to_string()),
            );
        }
    }

    value
}
