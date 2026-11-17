use super::{load_ws_logs_from_db, normalize_ws_session_key};

#[test]
fn test_normalize_ws_session_key() {
    assert_eq!(normalize_ws_session_key(""), None);
    assert_eq!(normalize_ws_session_key("   "), None);
    assert_eq!(
        normalize_ws_session_key("chat-123"),
        Some("ws:chat-123".to_string())
    );
    assert_eq!(
        normalize_ws_session_key("ws:chat-123"),
        Some("ws:chat-123".to_string())
    );
    assert_eq!(
        normalize_ws_session_key("cron:heartbeat_audit_5m"),
        Some("cron:heartbeat_audit_5m".to_string())
    );
}

#[test]
fn test_load_ws_logs_preserves_active_session_under_heavy_cron_burst() {
    let temp_db =
        std::env::temp_dir().join(format!("openz_ws_obs_test_{}.db", uuid::Uuid::new_v4()));
    let conn = rusqlite::Connection::open(&temp_db).expect("open temp logs db");
    crate::logs::ensure_logs_schema(&conn).expect("init logs schema");

    conn.execute(
        "INSERT INTO logs (timestamp, level, target, message, session, category, fields_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            "2026-11-18T01:00:00Z",
            "INFO",
            "openz::agent::agent_loop::tool_execution",
            "Tool call completed  status=success tool=curate_skill",
            "ws:chat-alpha",
            "tool",
            r#"{"status":"success","tool":"curate_skill"}"#,
        ],
    )
    .expect("insert session tool log");

    for idx in 0..180 {
        conn.execute(
            "INSERT INTO logs (timestamp, level, target, message, session, category, fields_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                format!("2026-11-18T01:05:{:02}Z", idx % 60),
                "INFO",
                "openz::cron::scheduler",
                "cron job completed successfully",
                "cron:heartbeat_audit_5m",
                "cron",
                r#"{"job_id":"heartbeat_audit_5m"}"#,
            ],
        )
        .expect("insert cron log");
    }

    conn.execute(
        "INSERT INTO logs (timestamp, level, target, message, session, category, fields_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            "2026-11-18T01:06:00Z",
            "INFO",
            "openz::channels::websocket",
            "Gateway ready",
            Option::<String>::None,
            "channel",
            Option::<String>::None,
        ],
    )
    .expect("insert gateway log");
    drop(conn);

    let entries = load_ws_logs_from_db(&temp_db, Some("ws:chat-alpha"));
    let serialized = serde_json::to_value(&entries).expect("serialize ws log entries");
    let arr = serialized.as_array().expect("entries array");

    let alpha_entries: Vec<&serde_json::Value> = arr
        .iter()
        .filter(|item| item.get("session").and_then(|v| v.as_str()) == Some("ws:chat-alpha"))
        .collect();
    assert_eq!(alpha_entries.len(), 1);
    assert_eq!(
        alpha_entries[0]
            .get("fields")
            .and_then(|f| f.get("tool"))
            .and_then(|v| v.as_str()),
        Some("curate_skill")
    );

    let cron_count = arr
        .iter()
        .filter(|item| item.get("category").and_then(|v| v.as_str()) == Some("cron"))
        .count();
    assert_eq!(cron_count, 80);

    let _ = std::fs::remove_file(&temp_db);
}
