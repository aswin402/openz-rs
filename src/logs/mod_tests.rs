use super::*;

#[test]
fn test_log_level_filter_parsing() {
    assert_eq!(LogLevelFilter::from_opt(Some("info")), LogLevelFilter::Info);
    assert_eq!(LogLevelFilter::from_opt(Some("warn")), LogLevelFilter::Warn);
    assert_eq!(LogLevelFilter::from_opt(Some("error")), LogLevelFilter::Error);
    assert_eq!(LogLevelFilter::from_opt(None), LogLevelFilter::All);
}

#[test]
fn test_session_filter_parsing() {
    assert_eq!(SessionFilter::from_opt(Some("all")), SessionFilter::All);
    assert_eq!(SessionFilter::from_opt(None), SessionFilter::All);
    assert_eq!(
        SessionFilter::from_opt(Some("gateway")),
        SessionFilter::Only("ws:".to_string())
    );
    assert_eq!(
        SessionFilter::from_opt(Some("session-123")),
        SessionFilter::Only("session-123".to_string())
    );
}

#[test]
fn test_log_category_aliases_and_classification() {
    assert_eq!(LogCategory::from_alias("cronlogs"), Some(LogCategory::Cron));
    assert_eq!(LogCategory::from_alias("cornlog"), Some(LogCategory::Cron));
    assert_eq!(
        LogCategory::from_alias("sessionlogs"),
        Some(LogCategory::Session)
    );
    assert_eq!(
        LogCategory::from_alias("workinglogs"),
        Some(LogCategory::Session)
    );
    assert_eq!(LogCategory::from_alias("toollogs"), Some(LogCategory::Tool));
    assert_eq!(
        LogCategory::from_alias("subagentlogs"),
        Some(LogCategory::Subagent)
    );
    assert_eq!(
        LogCategory::from_alias("curatorlogs"),
        Some(LogCategory::Curator)
    );
    assert_eq!(
        LogCategory::from_alias("channellogs"),
        Some(LogCategory::Channel)
    );
    assert_eq!(
        LogCategory::from_alias("securitylogs"),
        Some(LogCategory::Security)
    );

    assert_eq!(
        classify_log_category(
            "openz::agent::agent_loop::run",
            Some("cron:heartbeat_audit_5m"),
            "Sending completion request to LLM"
        ),
        LogCategory::Cron
    );
    assert_eq!(
        classify_log_category(
            "openz::agent::agent_loop::tool_execution",
            Some("ws:abc"),
            "Tool call completed"
        ),
        LogCategory::Tool
    );
    assert_eq!(
        classify_log_category(
            "openz::agent::agent_loop::save",
            Some("ws:abc"),
            "Self-improvement curator finished successfully."
        ),
        LogCategory::Curator
    );
    assert_eq!(
        classify_log_category(
            "openz::agent::agent_loop::run",
            Some("cli:direct"),
            "Received LLM response"
        ),
        LogCategory::Session
    );
}
