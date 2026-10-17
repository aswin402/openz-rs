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
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;
        let session_opt = arguments.get("session").and_then(|v| v.as_str());
        let level_opt = arguments.get("level").and_then(|v| v.as_str());

        let db_path = crate::logs::default_db_path();
        match db_path.exists() {
            false => {
                return Ok(json!({
                    "status": "success",
                    "message": "Logs database does not exist yet.",
                    "logs": []
                }));
            }
            true => {}
        }

        let conn = rusqlite::Connection::open(&db_path)?;

        let target_session = match session_opt {
            Some("all") => None,
            Some("current") | None => crate::agent::activity::get_activity()
                .map(|act| act.session_id)
                .or_else(crate::logs::get_latest_session_id),
            Some(other) => Some(other.to_string()),
        };

        let filter = match target_session {
            Some(s) => crate::logs::SessionFilter::Only(s),
            None => crate::logs::SessionFilter::All,
        };

        let level_filter = crate::logs::LogLevelFilter::from_opt(level_opt);

        let mut query =
            "SELECT id, timestamp, level, target, message, session FROM logs".to_string();
        let mut where_clauses = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        match &filter {
            crate::logs::SessionFilter::Only(s) => {
                where_clauses.push("session = ?".to_string());
                params.push(Box::new(s.clone()));
            }
            _ => {}
        }

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

        match where_clauses.is_empty() {
            true => {}
            false => {
                query.push_str(" WHERE ");
                query.push_str(&where_clauses.join(" AND "));
            }
        }

        query.push_str(" ORDER BY id DESC LIMIT ?");
        params.push(Box::new(limit));

        let mut stmt = conn.prepare(&query)?;

        let params_refs: Vec<&dyn rusqlite::ToSql> = params
            .iter()
            .map(|p| &**p as &dyn rusqlite::ToSql)
            .collect();

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
        for row in rows_iter.flatten() {
            logs.push(json!({
                "id": row.id,
                "timestamp": row.timestamp,
                "level": row.level,
                "target": row.target,
                "message": row.message,
                "session": row.session
            }));
        }

        logs.reverse();

        Ok(json!({
            "status": "success",
            "logs": logs
        }))
    }
}

#[cfg(test)]
#[path = "get_logs_tests.rs"]
mod tests;
