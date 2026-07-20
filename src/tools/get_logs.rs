use crate::tools::Tool;
use anyhow::Result;
use serde_json::{json, Value};

pub struct GetLogsTool;

#[async_trait::async_trait]
impl Tool for GetLogsTool {
    fn name(&self) -> &str {
        "get_logs"
    }

    fn description(&self) -> &str {
        "Retrieve recent system logs from the SQLite database. Useful to inspect runtime behavior, errors, and background channel status."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Optional number of recent log lines to retrieve (default 50)."
                },
                "session": {
                    "type": "string",
                    "description": "Optional session ID filter. Use 'current' for the current agent session, 'gateway' for the WebSocket gateway, or 'all' for all sessions."
                },
                "level": {
                    "type": "string",
                    "description": "Optional log level filter (trace, debug, info, warn, error)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
        let session_opt = arguments.get("session").and_then(|v| v.as_str());
        let level_opt = arguments.get("level").and_then(|v| v.as_str());

        let db_path = crate::logs::default_db_path();
        if !db_path.exists() {
            return Ok(json!({
                "status": "success",
                "message": "Logs database does not exist yet.",
                "logs": []
            }));
        }

        let conn = rusqlite::Connection::open(&db_path)?;

        // Resolve session filter
        let target_session = match session_opt {
            Some("all") => None,
            Some("current") | None => {
                if let Some(act) = crate::agent::activity::get_activity() {
                    Some(act.session_id)
                } else {
                    crate::logs::get_latest_session_id()
                }
            }
            Some(other) => Some(other.to_string()),
        };

        let filter = match target_session {
            Some(s) => crate::logs::SessionFilter::Only(s),
            None => crate::logs::SessionFilter::All,
        };

        let level_filter = crate::logs::LogLevelFilter::from_opt(level_opt);

        // Build SQL query
        let mut query = "SELECT id, timestamp, level, target, message, session FROM logs".to_string();
        let mut where_clauses = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        match &filter {
            crate::logs::SessionFilter::Only(s) => {
                where_clauses.push("session = ?".to_string());
                params.push(Box::new(s.clone()));
            }
            _ => {}
        }

        // Add level filters
        let min_level_val = match level_filter {
            crate::logs::LogLevelFilter::Trace => 1,
            crate::logs::LogLevelFilter::Debug => 2,
            crate::logs::LogLevelFilter::Info => 3,
            crate::logs::LogLevelFilter::Warn => 4,
            crate::logs::LogLevelFilter::Error => 5,
        };

        where_clauses.push(
            "(CASE level WHEN 'TRACE' THEN 1 WHEN 'DEBUG' THEN 2 WHEN 'INFO' THEN 3 WHEN 'WARN' THEN 4 WHEN 'ERROR' THEN 5 ELSE 2 END) >= ?"
                .to_string(),
        );
        params.push(Box::new(min_level_val));

        if !where_clauses.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&where_clauses.join(" AND "));
        }

        query.push_str(" ORDER BY id DESC LIMIT ?");
        params.push(Box::new(limit));

        let mut stmt = conn.prepare(&query)?;
        
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| &**p as &dyn rusqlite::ToSql).collect();

        struct LogRow {
            id: i64,
            timestamp: String,
            level: String,
            target: String,
            message: String,
            session: Option<String>,
        }

        let rows_iter = stmt.query_map(rusqlite::params_from_iter(params_refs), |row| {
            Ok(LogRow {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                level: row.get(2)?,
                target: row.get(3)?,
                message: row.get(4)?,
                session: row.get(5)?,
            })
        })?;

        let mut logs = Vec::new();
        for r in rows_iter {
            if let Ok(row) = r {
                logs.push(json!({
                    "id": row.id,
                    "timestamp": row.timestamp,
                    "level": row.level,
                    "target": row.target,
                    "message": row.message,
                    "session": row.session
                }));
            }
        }

        logs.reverse();

        Ok(json!({
            "status": "success",
            "logs": logs
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_logs_tool() {
        let temp_dir = std::env::temp_dir().join(format!("openz_get_logs_test_{}", uuid::Uuid::new_v4()));
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

        crate::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
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
            assert_eq!(logs_err[0].get("message").unwrap().as_str().unwrap(), "Test log message 2");
        }).await;

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
