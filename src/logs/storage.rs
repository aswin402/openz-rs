use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
    pub session: Option<String>,
}

pub fn default_db_path() -> PathBuf {
    crate::config::config_dir().join("logs.db")
}

/// Resolve the default log file path: ~/.openz/openz.log (or OPENZ_CONFIG_DIR/openz.log)
pub fn default_log_path() -> PathBuf {
    crate::config::config_dir().join("openz.log")
}

pub async fn init_db_writer(mut rx: tokio::sync::mpsc::UnboundedReceiver<LogEntry>) {
    let db_path = default_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    tokio::task::spawn_blocking(move || {
        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to open logs.db: {}", e);
                return;
            }
        };

        if let Err(e) = conn.execute(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                message TEXT NOT NULL,
                session TEXT
            )",
            [],
        ) {
            eprintln!("Failed to create logs table: {}", e);
            return;
        }

        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_session ON logs (session)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs (timestamp)",
            [],
        );

        // Purge logs older than 7 days on startup
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let _ = conn.execute("DELETE FROM logs WHERE timestamp < ?1", [&cutoff]);

        while let Some(entry) = rx.blocking_recv() {
            let _ = conn.execute(
                "INSERT INTO logs (timestamp, level, target, message, session) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    entry.timestamp,
                    entry.level,
                    entry.target,
                    entry.message,
                    entry.session,
                ],
            );
        }
    });
}
