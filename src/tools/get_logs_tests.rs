use super::*;

#[tokio::test]
async fn test_get_logs_tool() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_get_logs_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let _conn = crate::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
        let db_path = crate::logs::default_db_path();
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
        ).unwrap();

        conn.execute(
            "INSERT INTO logs (timestamp, level, target, message, session) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "2026-07-20T20:45:00Z",
                "INFO",
                "openz::test",
                "Test log message 1",
                "session_1"
            ],
        ).unwrap();

        conn.execute(
            "INSERT INTO logs (timestamp, level, target, message, session) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "2026-07-20T20:45:01Z",
                "ERROR",
                "openz::test",
                "Test log message 2",
                "session_1"
            ],
        ).unwrap();

        conn
    }).await;

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let tool = GetLogsTool;

            let args = json!({
                "limit": 10,
                "session": "session_1",
                "level": "info"
            });
            let result = tool.call(&args).await.unwrap();
            let logs = result.get("logs").unwrap().as_array().unwrap();
            assert_eq!(logs.len(), 2);

            let args_err = json!({
                "limit": 10,
                "session": "session_1",
                "level": "error"
            });
            let result_err = tool.call(&args_err).await.unwrap();
            let logs_err = result_err.get("logs").unwrap().as_array().unwrap();
            assert_eq!(logs_err.len(), 1);
            assert_eq!(
                logs_err[0].get("message").unwrap().as_str().unwrap(),
                "Test log message 2"
            );
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}
