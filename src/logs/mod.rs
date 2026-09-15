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
#[path = "mod_tests.rs"]
mod tests;

