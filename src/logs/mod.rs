pub mod query;
pub mod storage;
pub mod subscriber;
pub mod tui;

pub use query::{
    detect_active_session, get_latest_session_id, get_running_sessions, LogLevelFilter,
    RunningSession, SessionFilter,
};
pub use storage::{default_db_path, default_log_path, init_db_writer, LogEntry};
pub use subscriber::{
    initialize_secret_redaction, redact_sensitive_text, SecretScrubWriter, SqliteLogLayer, LOG_TX,
};
pub use tui::{print_row, print_session_recent_logs, run_logs_viewer};

#[cfg(test)]
mod tests {
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
}
