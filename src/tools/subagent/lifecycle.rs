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
    TimedOut,
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
            Self::TimedOut => "timed out".to_string(),
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
            Self::TimedOut => "timed_out",
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
        SubagentRunStatus::TimedOut
    } else if lower.contains("cancelled") || lower.contains("canceled") {
        SubagentRunStatus::Cancelled
    } else {
        SubagentRunStatus::Failed {
            error: error.to_string(),
        }
    }
}

pub fn status_json(status: &SubagentRunStatus) -> serde_json::Value {
    serde_json::json!({
        "code": status.code(),
        "label": status.label(),
    })
}
