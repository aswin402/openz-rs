use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::Connection;
use serde_json::{json, Value};

const INSPECTOR_BLOCKED_KEYWORDS: &[&str] = &[
    "INSERT",
    "UPDATE",
    "DELETE",
    "DROP",
    "ALTER",
    "CREATE",
    "ATTACH",
    "DETACH",
    "PRAGMA",
    "REINDEX",
    "REPLACE",
    "VACUUM",
    "ANALYZE",
    "INTO",
    "UNION",
    "EXCEPT",
    "INTERSECT",
    "LOAD",
    "OVERWRITE",
    "CALL",
    "EXECUTE",
];

const WRITE_BLOCKED_KEYWORDS: &[&str] = &["ATTACH", "DETACH", "LOAD"];

const BLOCKED_DOT_COMMANDS: &[&str] = &[".shell", ".import", ".output", ".read", ".system"];

fn normalize_sql(sql: &str) -> String {
    sql.to_uppercase()
        .chars()
        .map(|c| match c.is_whitespace() {
            true => ' ',
            false => match c {
                '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => '\0',
                '\u{FF10}'..='\u{FF19}' => ((c as u32 - 0xFF10) as u8 + b'0') as char,
                _ => c,
            },
        })
        .filter(|c| *c != '\0')
        .collect()
}

fn find_blocked_sql_keyword<'a>(normalized_sql: &str, blocklist: &[&'a str]) -> Option<&'a str> {
    normalized_sql
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .find_map(|token| blocklist.iter().copied().find(|&kw| kw == token))
}

fn extract_db_path_arg(arguments: &Value) -> Option<&str> {
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
        .and_then(|v| v.as_str())
}

fn resolve_safe_db_path(raw_path_input: Option<&str>) -> Result<std::path::PathBuf> {
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
    Ok(db_path)
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
        let (raw_path_input, sql_from_arg) = match arguments.as_str() {
            Some(s) => {
                let s_trimmed = s.trim();
                let upper = s_trimmed.to_uppercase();
                match upper.starts_with("SELECT") || upper.starts_with("EXPLAIN") {
                    true => (None, Some(s_trimmed)),
                    false => (Some(s_trimmed), None),
                }
            }
            None => (extract_db_path_arg(arguments), None),
        };

        let db_path = resolve_safe_db_path(raw_path_input)?;

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
            None => match sql_opt.is_some() {
                true => "query",
                false => "schema",
            },
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

                let normalized = normalize_sql(sql);
                let trimmed_sql = sql.trim_end_matches(';').trim();
                match trimmed_sql.contains(';') {
                    true => {
                        return Err(anyhow!(
                            "Only simple SELECT queries are allowed. Semicolons (stacked queries) are blocked."
                        ));
                    }
                    false => {}
                }
                match sql.contains("--") || sql.contains("/*") {
                    true => {
                        return Err(anyhow!(
                            "Only simple SELECT queries are allowed. SQL comments are blocked."
                        ));
                    }
                    false => {}
                }
                match find_blocked_sql_keyword(&normalized, INSPECTOR_BLOCKED_KEYWORDS) {
                    Some(keyword) => {
                        return Err(anyhow!(
                            "Only simple SELECT queries are allowed. Blocked keyword: {}",
                            keyword
                        ));
                    }
                    None => {}
                }
                let trimmed_raw = sql.trim();
                for dot_cmd in BLOCKED_DOT_COMMANDS {
                    match trimmed_raw.starts_with(dot_cmd) {
                        true => return Err(anyhow!("Blocked sqlite3 dot-command: {}", dot_cmd)),
                        false => {}
                    }
                }
                let trimmed = trimmed_raw.to_uppercase();
                match trimmed.starts_with("SELECT") || trimmed.starts_with("EXPLAIN") {
                    true => {}
                    false => {
                        return Err(anyhow!(
                            "Only SELECT (or EXPLAIN) queries are allowed for safety."
                        ));
                    }
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
        let (raw_path_input, sql_from_arg) = match arguments.as_str() {
            Some(s) => (None, Some(s.trim())),
            None => (extract_db_path_arg(arguments), None),
        };

        let db_path = resolve_safe_db_path(raw_path_input)?;

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

        let trimmed_sql = sql.trim_end_matches(';').trim();
        match trimmed_sql.contains(';') {
            true => {
                return Err(anyhow!(
                    "Stacked queries are not allowed. Use separate calls for multiple statements."
                ));
            }
            false => {}
        }
        match sql.contains("--") || sql.contains("/*") {
            true => {
                return Err(anyhow!("SQL comments are not allowed in write operations."));
            }
            false => {}
        }
        let lower_trimmed = sql.trim().to_lowercase();
        for dot_cmd in BLOCKED_DOT_COMMANDS {
            match lower_trimmed.starts_with(dot_cmd) {
                true => return Err(anyhow!("Blocked sqlite3 dot-command: {}", dot_cmd)),
                false => {}
            }
        }
        let normalized = normalize_sql(sql);
        match find_blocked_sql_keyword(&normalized, WRITE_BLOCKED_KEYWORDS) {
            Some(keyword) => {
                return Err(anyhow!("Blocked SQL keyword for safety: {}", keyword));
            }
            None => {}
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

