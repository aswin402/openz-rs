use super::*;
use crate::tools::subagent::CancellationToken;

#[test]
fn test_lifecycle_status_labels_are_stable_for_tui() {
    assert_eq!(SubagentRunStatus::Queued.label(), "queued");
    assert_eq!(SubagentRunStatus::Running.label(), "running");
    assert_eq!(
        SubagentRunStatus::Fallback {
            model: "gemini".into(),
            attempt: 1,
            total: 3
        }
        .label(),
        "fallback 1/3: gemini"
    );
    assert_eq!(SubagentRunStatus::Cancelling.label(), "cancelling");
    assert_eq!(SubagentRunStatus::Cancelled.label(), "cancelled");
    assert_eq!(
        SubagentRunStatus::TimedOut {
            duration_secs: None
        }
        .label(),
        "timed out"
    );
    assert_eq!(
        SubagentRunStatus::Failed {
            error: "boom".into()
        }
        .label(),
        "failed: boom"
    );
    assert_eq!(SubagentRunStatus::Completed.label(), "completed");
}

#[test]
fn test_compact_lifecycle_line_for_cancellation_is_stable() {
    let line = compact_lifecycle_line(
        "vision_agent",
        "google_ai_studio/gemini-2.5-flash",
        &SubagentRunStatus::Cancelling,
    );

    assert_eq!(
        line,
        "vision_agent | google_ai_studio/gemini-2.5-flash | cancelling"
    );
    assert!(!line.contains("Running..."));
}

#[test]
fn test_lifecycle_classifies_timeout_without_user_cancel() {
    let token = CancellationToken::new();

    assert_eq!(
        classify_subagent_error("Subagent execution timed out after 5 minutes", &token),
        SubagentRunStatus::TimedOut {
            duration_secs: Some(300)
        }
    );
}

#[test]
fn test_lifecycle_classifies_timeout_duration_seconds() {
    let token = CancellationToken::new();

    assert_eq!(
        classify_subagent_error("Subagent execution timed out after 900s", &token),
        SubagentRunStatus::TimedOut {
            duration_secs: Some(900)
        }
    );
}

#[test]
fn test_lifecycle_timeout_status_json_includes_duration() {
    let value = status_json(&SubagentRunStatus::TimedOut {
        duration_secs: Some(900),
    });

    assert_eq!(value["code"], "timed_out");
    assert_eq!(value["label"], "timed out after 900s");
    assert_eq!(value["durationSecs"], 900);
}

#[test]
fn test_compact_lifecycle_line_includes_timeout_duration() {
    let line = compact_lifecycle_line(
        "delegate_task",
        "deepseek/deepseek-chat",
        &SubagentRunStatus::TimedOut {
            duration_secs: Some(900),
        },
    );

    assert_eq!(
        line,
        "delegate_task | deepseek/deepseek-chat | timed out after 900s"
    );
}

#[test]
fn test_lifecycle_classifies_user_cancel_from_token() {
    let token = CancellationToken::new();
    token.cancel();

    assert_eq!(
        classify_subagent_error("provider returned 429", &token),
        SubagentRunStatus::Cancelled
    );
}
