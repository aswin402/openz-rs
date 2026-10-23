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
    match token.is_cancelled() {
        true => return SubagentRunStatus::Cancelled,
        false => {}
    }

    let lower = error.to_lowercase();
    let is_timeout = lower.contains("timed out") || lower.contains("timeout");
    let is_cancelled = lower.contains("cancelled") || lower.contains("canceled");
    match (is_timeout, is_cancelled) {
        (true, _) => SubagentRunStatus::TimedOut {
            duration_secs: parse_timeout_duration_secs(error),
        },
        (false, true) => SubagentRunStatus::Cancelled,
        (false, false) => SubagentRunStatus::Failed {
            error: error.to_string(),
        },
    }
}

fn parse_timeout_duration_secs(error: &str) -> Option<u64> {
    let lower = error.to_lowercase();
    let searchable = lower
        .find("after")
        .map(|idx| &lower[idx + "after".len()..])
        .unwrap_or(&lower);
    let mut pending_number = None;

    for token in searchable
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | '.' | ':' | ';' | '=' | '(' | ')'))
        .filter(|token| !token.is_empty())
    {
        let digit_len = token
            .char_indices()
            .take_while(|(_, ch)| ch.is_ascii_digit())
            .map(|(idx, ch)| idx + ch.len_utf8())
            .last()
            .unwrap_or(0);

        match digit_len > 0 {
            true => {
                let value = token[..digit_len].parse::<u64>().ok()?;
                let unit = &token[digit_len..];
                match duration_unit_to_secs(value, unit) {
                    Some(secs) => return Some(secs),
                    None => pending_number = Some(value),
                }
            }
            false => match pending_number.take().and_then(|v| duration_unit_to_secs(v, token)) {
                Some(secs) => return Some(secs),
                None => {}
            },
        }
    }

    None
}

fn duration_unit_to_secs(value: u64, unit: &str) -> Option<u64> {
    let unit = unit.trim();
    const UNIT_RULES: &[(&[&str], &str, u64)] = &[
        (&["s", "sec", "secs"], "second", 1),
        (&["m", "min", "mins"], "minute", 60),
        (&["h", "hr", "hrs"], "hour", 3600),
    ];
    UNIT_RULES.iter().find_map(|(exact, prefix, mult)| {
        (!unit.is_empty() && (exact.contains(&unit) || unit.starts_with(prefix)))
            .then(|| value.saturating_mul(*mult))
    })
}

pub fn compact_lifecycle_line(name: &str, model: &str, status: &SubagentRunStatus) -> String {
    let clean_name = name.trim();
    let clean_model = model.trim();
    let label = status.label();

    match clean_model.is_empty() {
        true => format!("{clean_name} | {label}"),
        false => format!("{clean_name} | {clean_model} | {label}"),
    }
}

pub fn status_json(status: &SubagentRunStatus) -> serde_json::Value {
    let mut value = serde_json::json!({
        "code": status.code(),
        "label": status.label(),
    });

    match (status, value.as_object_mut()) {
        (
            SubagentRunStatus::TimedOut {
                duration_secs: Some(secs),
            },
            Some(map),
        ) => {
            map.insert("durationSecs".to_string(), serde_json::json!(secs));
        }
        _ => {}
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

    match (subagent_name, value.as_object_mut()) {
        (Some(name), Some(map)) => {
            map.insert(
                "subagent".to_string(),
                serde_json::Value::String(name.to_string()),
            );
        }
        _ => {}
    }

    value
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;

