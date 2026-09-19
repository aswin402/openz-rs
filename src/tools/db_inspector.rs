use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::Connection;
use serde_json::{json, Value};

fn normalize_sql(sql: &str) -> String {
    let sql_upper = sql.to_uppercase();
    sql_upper
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                ' '
            } else {
                match c {
                    '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => '\0', // zero-width chars
                    '\u{FF10}'..='\u{FF19}' => ((c as u32 - 0xFF10) as u8 + b'0') as char, // fullwidth digits → ASCII
                    _ => c,
                }
            }
        })
        .filter(|c| *c != '\0')
        .collect()
}

pub struct DbInspectorTool;

#[async_trait::async_trait]
impl Tool for DbInspectorTool {
    fn name(&self) -> &str {
        "db_inspector"
    }

    fn description(&self) -> &str {
        "Inspect SQLite databases (read schemas, run SQL queries) directly."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "db_path": {
                    "type": "string",
                    "description": "Path to the SQLite database file (aliases: path, database, db, file)."
                },
                "action": {
                    "type": "string",
                    "enum": ["schema", "query"],
                    "description": "The action to perform (defaults to 'query' if sql is provided, otherwise 'schema')."
                },
                "sql": {
                    "type": "string",
                    "description": "The SELECT query to run (required for 'query' action, aliases: query, statement)."
                }
            },
            "required": ["db_path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (raw_path_input, sql_from_arg) = if let Some(s) = arguments.as_str() {
            let s_trimmed = s.trim();
            let upper = s_trimmed.to_uppercase();
            if upper.starts_with("SELECT") || upper.starts_with("EXPLAIN") {
                (None, Some(s_trimmed))
            } else {
                (Some(s_trimmed), None)
            }
        } else {
            (
                arguments
                    .get("db_path")
                    .or_else(|| arguments.get("path"))
                    .or_else(|| arguments.get("database"))
                    .or_else(|| arguments.get("db"))
                    .or_else(|| arguments.get("file"))
                    .or_else(|| arguments.get("file_path"))
                    .or_else(|| arguments.get("filePath"))
                    .or_else(|| arguments.get("target"))
                    .or_else(|| arguments.get("uri"))
                    .and_then(|v| v.as_str()),
                None,
            )
        };

        let db_path = match raw_path_input.map(str::trim).filter(|s| !s.is_empty()) {
            Some("memory" | "openz" | "default") | None => {
                crate::config::loader::runtime_db_path("memory.db")
            }
            Some(p) => {
                let clean = p.strip_prefix("file://").unwrap_or(p);
                crate::config::loader::resolve_path(clean)
            }
        };
        crate::config::loader::verify_safe_path(&db_path)?;

        let sql_opt = sql_from_arg.or_else(|| {
            arguments
                .get("sql")
                .or_else(|| arguments.get("query"))
                .or_else(|| arguments.get("statement"))
                .or_else(|| arguments.get("select"))
                .and_then(|v| v.as_str())
        });

        let action_raw = arguments
            .get("action")
            .or_else(|| arguments.get("command"))
            .or_else(|| arguments.get("cmd"))
            .or_else(|| arguments.get("act"))
            .and_then(|v| v.as_str());
        let action_norm = action_raw.map(|a| a.trim().to_lowercase());
        let action = match action_norm.as_deref() {
            Some("schema" | "tables" | "structure" | "desc" | "describe" | "list") => "schema",
            Some("query" | "select" | "run" | "sql" | "exec") => "query",
            Some(other) => other,
            None => {
                if sql_opt.is_some() {
                    "query"
                } else {
                    "schema"
                }
            }
        };

        let conn =
            Connection::open(&db_path).map_err(|e| anyhow!("Failed to open database: {}", e))?;

        let (stdout, status) = match action {
            "schema" => {
                let mut stmt = conn.prepare("SELECT sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY tbl_name, type DESC, name")
                    .map_err(|e| anyhow!("Failed to prepare schema query: {}", e))?;
                let rows = stmt
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(|e| anyhow!("Failed to execute schema query: {}", e))?;
                let mut schema = String::new();
                for sql in rows.flatten() {
                    schema.push_str(&sql);
                    schema.push_str(";\n");
                }
                (schema, "success")
            }
            "query" => {
                let sql = sql_opt
                    .ok_or_else(|| anyhow!("Missing 'sql' parameter for query action"))?;

                // Block dangerous SQL operations — use a strict blocklist
                let normalized = normalize_sql(sql);
                static INSPECTOR_BLOCKLIST_RE: std::sync::OnceLock<regex::Regex> =
                    std::sync::OnceLock::new();
                let re = INSPECTOR_BLOCKLIST_RE.get_or_init(|| {
                    regex::Regex::new(r"\b(INSERT|UPDATE|DELETE|DROP|ALTER|CREATE|ATTACH|DETACH|PRAGMA|REINDEX|REPLACE|VACUUM|ANALYZE|INTO|UNION|EXCEPT|INTERSECT|LOAD|OVERWRITE|CALL|EXECUTE)\b")
                        .expect("static db inspector blocklist regex must compile")
                });

                // Also block semicolons (stacked queries) and comment sequences.
                // Allow a single trailing semicolon (normal SQL syntax) but block mid-query semicolons.
                let trimmed_sql = sql.trim_end_matches(';').trim();
                if trimmed_sql.contains(';') {
                    return Err(anyhow!(
                        "Only simple SELECT queries are allowed. Semicolons (stacked queries) are blocked."
                    ));
                }
                if sql.contains("--") || sql.contains("/*") {
                    return Err(anyhow!(
                        "Only simple SELECT queries are allowed. SQL comments are blocked."
                    ));
                }
                // Also block shell-like dot commands used by sqlite3 CLI
                let blocked_dot = [".shell", ".import", ".output", ".read", ".system"];
                if let Some(mat) = re.find(&normalized) {
                    return Err(anyhow!(
                        "Only simple SELECT queries are allowed. Blocked keyword: {}",
                        mat.as_str()
                    ));
                }
                for dot_cmd in &blocked_dot {
                    if sql.trim().starts_with(dot_cmd) {
                        return Err(anyhow!("Blocked sqlite3 dot-command: {}", dot_cmd));
                    }
                }
                // Must start with SELECT or EXPLAIN (for EXPLAIN QUERY PLAN)
                let trimmed = sql.trim().to_uppercase();
                if !trimmed.starts_with("SELECT") && !trimmed.starts_with("EXPLAIN") {
                    return Err(anyhow!(
                        "Only SELECT (or EXPLAIN) queries are allowed for safety."
                    ));
                }

                let mut stmt = conn
                    .prepare(sql)
                    .map_err(|e| anyhow!("Failed to prepare query: {}", e))?;
                let col_count = stmt.column_count();
                let mut rows = stmt
                    .query([])
                    .map_err(|e| anyhow!("Failed to execute query: {}", e))?;

                let mut output = String::new();
                while let Some(row) = rows
                    .next()
                    .map_err(|e| anyhow!("Failed to retrieve row: {}", e))?
                {
                    let mut row_str = Vec::new();
                    for i in 0..col_count {
                        let val: rusqlite::types::Value = row
                            .get(i)
                            .map_err(|e| anyhow!("Failed to get column value: {}", e))?;
                        let val_str = match val {
                            rusqlite::types::Value::Null => "".to_string(),
                            rusqlite::types::Value::Integer(v) => v.to_string(),
                            rusqlite::types::Value::Real(v) => v.to_string(),
                            rusqlite::types::Value::Text(s) => s,
                            rusqlite::types::Value::Blob(b) => {
                                String::from_utf8_lossy(&b).to_string()
                            }
                        };
                        row_str.push(val_str);
                    }
                    output.push_str(&row_str.join("|"));
                    output.push('\n');
                }
                (output, "success")
            }
            _ => return Err(anyhow!("Invalid action: {}", action)),
        };

        Ok(json!({
            "status": status,
            "stdout": stdout,
            "stderr": "",
            "code": 0
        }))
    }
}

pub struct DbWriteTool;

#[async_trait::async_trait]
impl Tool for DbWriteTool {
    fn name(&self) -> &str {
        "db_write"
    }

    fn description(&self) -> &str {
        "Execute database mutations (INSERT, UPDATE, DELETE, CREATE TABLE, DROP TABLE, etc.) on a SQLite database."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "db_path": {
                    "type": "string",
                    "description": "Path to the SQLite database file (aliases: path, database, db, file)."
                },
                "sql": {
                    "type": "string",
                    "description": "The mutation query statement to execute (aliases: query, statement, mutation)."
                }
            },
            "required": ["db_path", "sql"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (raw_path_input, sql_from_arg) = if let Some(s) = arguments.as_str() {
            (None, Some(s.trim()))
        } else {
            (
                arguments
                    .get("db_path")
                    .or_else(|| arguments.get("path"))
                    .or_else(|| arguments.get("database"))
                    .or_else(|| arguments.get("db"))
                    .or_else(|| arguments.get("file"))
                    .or_else(|| arguments.get("file_path"))
                    .or_else(|| arguments.get("filePath"))
                    .or_else(|| arguments.get("target"))
                    .or_else(|| arguments.get("uri"))
                    .and_then(|v| v.as_str()),
                None,
            )
        };

        let db_path = match raw_path_input.map(str::trim).filter(|s| !s.is_empty()) {
            Some("memory" | "openz" | "default") | None => {
                crate::config::loader::runtime_db_path("memory.db")
            }
            Some(p) => {
                let clean = p.strip_prefix("file://").unwrap_or(p);
                crate::config::loader::resolve_path(clean)
            }
        };
        crate::config::loader::verify_safe_path(&db_path)?;

        let sql = sql_from_arg
            .or_else(|| {
                arguments
                    .get("sql")
                    .or_else(|| arguments.get("query"))
                    .or_else(|| arguments.get("statement"))
                    .or_else(|| arguments.get("mutation"))
                    .or_else(|| arguments.get("command"))
                    .or_else(|| arguments.get("exec"))
                    .and_then(|v| v.as_str())
            })
            .ok_or_else(|| anyhow!("Missing 'sql' parameter"))?;

        // Safety checks for db_write
        let trimmed_sql = sql.trim_end_matches(';').trim();
        if trimmed_sql.contains(';') {
            return Err(anyhow!(
                "Stacked queries are not allowed. Use separate calls for multiple statements."
            ));
        }
        if sql.contains("--") || sql.contains("/*") {
            return Err(anyhow!("SQL comments are not allowed in write operations."));
        }
        let blocked_dot = [".shell", ".import", ".output", ".read", ".system"];
        for dot_cmd in &blocked_dot {
            if sql.trim().to_lowercase().starts_with(dot_cmd) {
                return Err(anyhow!("Blocked sqlite3 dot-command: {}", dot_cmd));
            }
        }
        let normalized = normalize_sql(sql);
        static WRITE_BLOCKLIST_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = WRITE_BLOCKLIST_RE
            .get_or_init(|| regex::Regex::new(r"\b(ATTACH|DETACH|LOAD)\b").expect("static write blocklist regex must compile"));
        if let Some(mat) = re.find(&normalized) {
            return Err(anyhow!("Blocked SQL keyword for safety: {}", mat.as_str()));
        }

        let conn =
            Connection::open(&db_path).map_err(|e| anyhow!("Failed to open database: {}", e))?;

        let changes = conn
            .execute(sql, [])
            .map_err(|e| anyhow!("Failed to execute mutation query: {}", e))?;

        Ok(json!({
            "status": "success",
            "stdout": format!("Query executed successfully. Changes: {}", changes),
            "stderr": "",
            "code": 0
        }))
    }
}

#[cfg(test)]
#[path = "db_inspector_tests.rs"]
mod tests;

