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
        "Retrieve categorized structured logs from OpenZ's SQLite database (categories: session, tool, cron, subagent, curator, channel, security, system)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Optional number of recent log lines to retrieve (default 50, max 500)."
                },
                "category": {
                    "type": "string",
                    "enum": ["all", "session", "tool", "cron", "subagent", "curator", "channel", "security", "system"],
                    "description": "Optional log category filter (e.g. 'cron' / 'cronlogs', 'session' / 'sessionlogs', 'tool', 'subagent', 'curator', 'channel', 'security', 'system', or 'all')."
                },
                "session": {
                    "type": "string",
                    "description": "Optional session ID or prefix filter ('current' for active interactive session, 'gateway' for WebSocket sessions, 'cron' for scheduled jobs, or 'all')."
                },
                "level": {
                    "type": "string",
                    "description": "Optional minimum log level filter (trace, debug, info, warn, error)."
                },
                "search": {
                    "type": "string",
                    "description": "Optional substring filter matching message or module target."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(50)
            .clamp(1, 500) as usize;
        let session_opt = arguments.get("session").and_then(|v| v.as_str());
        let category_opt = arguments.get("category").and_then(|v| v.as_str());
        let level_opt = arguments.get("level").and_then(|v| v.as_str());
        let search_opt = arguments
            .get("search")
            .or_else(|| arguments.get("query"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());

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
        let _ = crate::logs::ensure_logs_schema(&conn);

        let category_filter = crate::logs::LogCategoryFilter::from_opt(category_opt);
        let has_category_filter = !matches!(category_filter, crate::logs::LogCategoryFilter::All);

        let target_session = match (session_opt, has_category_filter) {
            (Some("all"), _) | (None, true) => None,
            (Some("gateway" | "webui" | "ws"), _) => Some("ws:".to_string()),
            (Some("cron" | "cronlogs"), _) => Some("cron:".to_string()),
            (Some("current"), _) | (None, false) => crate::agent::activity::get_activity()
                .map(|act| act.session_id)
                .or_else(crate::logs::get_latest_session_id),
            (Some(other), _) => Some(other.to_string()),
        };

        let level_filter = crate::logs::LogLevelFilter::from_opt(level_opt);

        let mut query = "SELECT id, timestamp, level, target, message, session, COALESCE(category, ''), fields_json FROM logs".to_string();
        let mut where_clauses = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        match target_session {
            Some(prefix) => {
                where_clauses.push("(session = ? OR session LIKE ?)".to_string());
                params.push(Box::new(prefix.clone()));
                params.push(Box::new(format!("{prefix}%")));
            }
            None => {}
        }

        match category_filter {
            crate::logs::LogCategoryFilter::All => {}
            crate::logs::LogCategoryFilter::Only(cat) => {
                where_clauses.push("category = ?".to_string());
                params.push(Box::new(cat.as_str().to_string()));
            }
        }

        let _ = search_opt.map(|needle| {
            let pattern = format!("%{needle}%");
            where_clauses.push("(message LIKE ? OR target LIKE ?)".to_string());
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern));
        });

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
            category: String,
            fields_json: Option<String>,
        }

        let rows_iter = stmt.query_map(rusqlite::params_from_iter(params_refs), |row| {
            Ok(LogRow {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                level: row.get(2)?,
                target: row.get(3)?,
                message: row.get(4)?,
                session: row.get(5)?,
                category: row.get(6)?,
                fields_json: row.get(7)?,
            })
        })?;

        let mut logs = Vec::new();
        for row in rows_iter.flatten() {
            let resolved_category = match row.category.is_empty() {
                true => crate::logs::classify_log_category(
                    &row.target,
                    row.session.as_deref(),
                    &row.message,
                )
                .as_str()
                .to_string(),
                false => row.category,
            };
            let fields_val = row
                .fields_json
                .as_deref()
                .and_then(|s| serde_json::from_str::<Value>(s).ok());
            logs.push(json!({
                "id": row.id,
                "timestamp": row.timestamp,
                "level": row.level,
                "category": resolved_category,
                "target": row.target,
                "message": row.message,
                "session": row.session,
                "fields": fields_val,
            }));
        }

        logs.reverse();

        Ok(json!({
            "status": "success",
            "count": logs.len(),
            "logs": logs
        }))
    }
}

#[cfg(test)]
#[path = "get_logs_tests.rs"]
mod tests;
