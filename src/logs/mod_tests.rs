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
        SessionFilter::from_opt(Some("session-123")),
        SessionFilter::Only("session-123".to_string())
    );
}
