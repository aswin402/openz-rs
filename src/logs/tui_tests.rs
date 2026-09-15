use super::*;

#[tokio::test]
async fn test_sqlite_logging_workflow() {
    let db_path = std::env::temp_dir().join(format!("logs_test_{}.db", uuid::Uuid::new_v4()));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            level TEXT NOT NULL,
            target TEXT NOT NULL,
            message TEXT NOT NULL,
            session TEXT
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO logs (timestamp, level, target, message, session) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            "2026-07-20T12:00:00Z",
            "INFO",
            "openz::test",
            "Test message 1",
            Some("session-123"),
        ],
    ).unwrap();

    conn.execute(
        "INSERT INTO logs (timestamp, level, target, message, session) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            "2026-07-20T12:01:00Z",
            "ERROR",
            "openz::test",
            "Test message 2",
            Some("session-123"),
        ],
    ).unwrap();

    let filter = SessionFilter::Only("session-123".to_string());
    let level_filter = LogLevelFilter::Trace;
    let last_id = print_tail_sqlite(&db_path, 10, &filter, &level_filter, None).unwrap();
    assert_eq!(last_id, 2);

    let error_level_filter = LogLevelFilter::Error;
    let last_id_error =
        print_tail_sqlite(&db_path, 10, &filter, &error_level_filter, None).unwrap();
    assert_eq!(last_id_error, 2);

    let _ = std::fs::remove_file(&db_path);
}
