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

#[tokio::test]
async fn test_get_logs_category_and_gateway_prefix_filtering() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_get_logs_cat_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let db_path = crate::logs::default_db_path();
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            crate::logs::ensure_logs_schema(&conn).unwrap();

            conn.execute(
                "INSERT INTO logs (timestamp, level, target, message, session, category, fields_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    "2026-11-17T12:00:00Z",
                    "INFO",
                    "openz::cron::scheduler",
                    "cron job completed successfully  job_id=heartbeat_audit_5m",
                    "cron:heartbeat_audit_5m",
                    "cron",
                    r#"{"job_id":"heartbeat_audit_5m"}"#
                ],
            )
            .unwrap();

            conn.execute(
                "INSERT INTO logs (timestamp, level, target, message, session, category, fields_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    "2026-11-17T12:01:00Z",
                    "INFO",
                    "openz::agent::agent_loop::run",
                    "Received LLM response  duration_ms=420",
                    "ws:session_xyz",
                    "session",
                    r#"{"duration_ms":"420"}"#
                ],
            )
            .unwrap();

            let tool = GetLogsTool;
            let cron_res = tool
                .call(&json!({ "category": "cronlogs", "limit": 10 }))
                .await
                .unwrap();
            let cron_logs = cron_res["logs"].as_array().unwrap();
            assert_eq!(cron_logs.len(), 1);
            assert_eq!(cron_logs[0]["category"].as_str(), Some("cron"));
            assert_eq!(
                cron_logs[0]["fields"]["job_id"].as_str(),
                Some("heartbeat_audit_5m")
            );

            let gw_res = tool
                .call(&json!({ "session": "gateway", "limit": 10 }))
                .await
                .unwrap();
            let gw_logs = gw_res["logs"].as_array().unwrap();
            assert_eq!(gw_logs.len(), 1);
            assert_eq!(gw_logs[0]["session"].as_str(), Some("ws:session_xyz"));
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}
