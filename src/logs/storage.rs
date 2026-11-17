use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub category: String,
    pub message: String,
    pub session: Option<String>,
    pub fields_json: Option<String>,
}

pub fn default_db_path() -> PathBuf {
    crate::config::config_dir().join("logs.db")
}

/// Resolve the default combined log file path: ~/.openz/openz.log
pub fn default_log_path() -> PathBuf {
    crate::config::config_dir().join("openz.log")
}

/// Resolve the categorized logs directory: ~/.openz/logs/
pub fn category_logs_dir() -> PathBuf {
    crate::config::config_dir().join("logs")
}

pub fn category_log_path(category: &str) -> PathBuf {
    category_logs_dir().join(format!("{}.log", category))
}

fn rotate_category_log_if_needed(path: &Path) {
    const MAX_CATEGORY_LOG_BYTES: u64 = 5 * 1024 * 1024;
    let should_rotate = std::fs::metadata(path)
        .map(|m| m.len() > MAX_CATEGORY_LOG_BYTES)
        .unwrap_or(false);
    let _ = should_rotate.then(|| {
        let rotated = path.with_extension("log.1");
        let _ = std::fs::rename(path, rotated);
    });
}

fn append_category_log_entry(dir: &Path, entry: &LogEntry) {
    let path = dir.join(format!("{}.log", entry.category));
    rotate_category_log_if_needed(&path);
    let session_suffix = entry
        .session
        .as_deref()
        .map(|s| format!("  session={s}"))
        .unwrap_or_default();
    let line = format!(
        "{} {:>5} {}: {}{}\n",
        entry.timestamp, entry.level, entry.target, entry.message, session_suffix
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

pub fn ensure_logs_schema(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    let _ = conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA busy_timeout=5000;",
    );

    conn.execute(
        "CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            level TEXT NOT NULL,
            target TEXT NOT NULL,
            message TEXT NOT NULL,
            session TEXT,
            category TEXT NOT NULL DEFAULT 'system',
            fields_json TEXT
        )",
        [],
    )?;

    let mut has_category = false;
    let mut has_fields_json = false;
    let _ = conn.prepare("PRAGMA table_info(logs)").and_then(|mut stmt| {
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for col in cols.flatten() {
            match col.as_str() {
                "category" => has_category = true,
                "fields_json" => has_fields_json = true,
                _ => {}
            }
        }
        Ok(())
    });

    let _ = (!has_category).then(|| {
        let _ = conn.execute(
            "ALTER TABLE logs ADD COLUMN category TEXT NOT NULL DEFAULT 'system'",
            [],
        );
    });
    let _ = (!has_fields_json).then(|| {
        let _ = conn.execute("ALTER TABLE logs ADD COLUMN fields_json TEXT", []);
    });

    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_logs_session ON logs (session)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs (timestamp)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_logs_category ON logs (category)",
        [],
    );

    Ok(())
}

fn backfill_legacy_log_categories(conn: &rusqlite::Connection) {
    let _ = conn.execute_batch(
        "DELETE FROM logs WHERE target = 'log' AND message LIKE '%foster parenting not implemented%';
         UPDATE logs SET category = 'cron' WHERE category = 'system' AND (session LIKE 'cron:%' OR target LIKE 'openz::cron%');
         UPDATE logs SET category = 'subagent' WHERE category = 'system' AND (session LIKE 'subagent:%' OR target LIKE 'openz::subagents%' OR target LIKE 'openz::orchestrator%');
         UPDATE logs SET category = 'security' WHERE category = 'system' AND (target LIKE 'openz::agent::security%' OR target LIKE 'openz::core::secrets%');
         UPDATE logs SET category = 'curator' WHERE category = 'system' AND ((target LIKE 'openz::agent::agent_loop::save%' AND message LIKE '%curator%') OR target LIKE 'openz::agent::skills%' OR target LIKE 'openz::memory%' OR target LIKE 'openz::tools::shared_memory%');
         UPDATE logs SET category = 'tool' WHERE category = 'system' AND (target LIKE 'openz::agent::agent_loop::tool_execution%' OR target LIKE 'openz::tools::%' OR message LIKE 'Executing tool call%' OR message LIKE 'Tool call %');
         UPDATE logs SET category = 'channel' WHERE category = 'system' AND target LIKE 'openz::channels::%';
         UPDATE logs SET category = 'session' WHERE category = 'system' AND (target LIKE 'openz::agent::agent_loop::%' OR target LIKE 'openz::providers::%');",
    );
}

fn purge_expired_logs(conn: &rusqlite::Connection) {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    let _ = conn.execute("DELETE FROM logs WHERE timestamp < ?1", [&cutoff]);
}

pub async fn init_db_writer(mut rx: tokio::sync::mpsc::UnboundedReceiver<LogEntry>) {
    let db_path = default_db_path();
    let cat_dir = category_logs_dir();
    let _ = db_path.parent().map(std::fs::create_dir_all);
    let _ = std::fs::create_dir_all(&cat_dir);

    tokio::task::spawn_blocking(move || {
        let mut conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to open logs.db: {}", e);
                return;
            }
        };

        match ensure_logs_schema(&conn) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Failed to initialize logs schema: {}", e);
                return;
            }
        }

        backfill_legacy_log_categories(&conn);
        purge_expired_logs(&conn);

        let mut batches_written = 0usize;
        let mut batch = Vec::with_capacity(64);

        while let Some(first) = rx.blocking_recv() {
            batch.clear();
            batch.push(first);
            while batch.len() < 64 {
                match rx.try_recv() {
                    Ok(next) => batch.push(next),
                    Err(_) => break,
                }
            }

            for item in &batch {
                append_category_log_entry(&cat_dir, item);
            }

            let _ = conn.transaction().and_then(|tx| {
                {
                    let mut stmt = tx.prepare_cached(
                        "INSERT INTO logs (timestamp, level, target, message, session, category, fields_json)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    )?;
                    for entry in &batch {
                        let _ = stmt.execute(rusqlite::params![
                            entry.timestamp,
                            entry.level,
                            entry.target,
                            entry.message,
                            entry.session,
                            entry.category,
                            entry.fields_json,
                        ]);
                    }
                }
                tx.commit()
            });

            batches_written = batches_written.wrapping_add(1);
            let _ = batches_written
                .is_multiple_of(500)
                .then(|| purge_expired_logs(&conn));
        }
    });
}
