use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use rusqlite::Connection;

fn normalize_sql(sql: &str) -> String {
    let sql_upper = sql.to_uppercase();
    sql_upper.chars()
        .map(|c| if c.is_whitespace() {
            ' '
        } else {
            match c {
                '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => '\0', // zero-width chars
                '\u{FF10}'..='\u{FF19}' => ((c as u32 - 0xFF10) as u8 + b'0') as char, // fullwidth digits → ASCII
                _ => c,
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
                    "description": "Path to the SQLite database file."
                },
                "action": {
                    "type": "string",
                    "enum": ["schema", "query"],
                    "description": "The action to perform."
                },
                "sql": {
                    "type": "string",
                    "description": "The SELECT query to run (required for 'query')."
                }
            },
            "required": ["db_path", "action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let db_path_raw = arguments.get("db_path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'db_path' parameter"))?;
        let db_path = crate::config::loader::resolve_path(db_path_raw);
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let conn = Connection::open(&db_path)
            .map_err(|e| anyhow!("Failed to open database: {}", e))?;

        let (stdout, status) = match action {
            "schema" => {
                let mut stmt = conn.prepare("SELECT sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY tbl_name, type DESC, name")
                    .map_err(|e| anyhow!("Failed to prepare schema query: {}", e))?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(0))
                    .map_err(|e| anyhow!("Failed to execute schema query: {}", e))?;
                let mut schema = String::new();
                for row in rows {
                    if let Ok(sql) = row {
                        schema.push_str(&sql);
                        schema.push_str(";\n");
                    }
                }
                (schema, "success")
            }
            "query" => {
                let sql = arguments.get("sql").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'sql' parameter for query action"))?;

                // Block dangerous SQL operations — use a strict blocklist
                let normalized = normalize_sql(sql);
                static INSPECTOR_BLOCKLIST_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
                let re = INSPECTOR_BLOCKLIST_RE.get_or_init(|| {
                    regex::Regex::new(r"\b(INSERT|UPDATE|DELETE|DROP|ALTER|CREATE|ATTACH|DETACH|PRAGMA|REINDEX|REPLACE|VACUUM|ANALYZE|INTO|UNION|EXCEPT|INTERSECT|LOAD|OVERWRITE|CALL|EXECUTE)\b").unwrap()
                });

                // Also block semicolons (stacked queries) and comment sequences.
                // Allow a single trailing semicolon (normal SQL syntax) but block mid-query semicolons.
                let trimmed_sql = sql.trim_end_matches(';').trim();
                if trimmed_sql.contains(';') {
                    return Err(anyhow!("Only simple SELECT queries are allowed. Semicolons (stacked queries) are blocked."));
                }
                if sql.contains("--") || sql.contains("/*") {
                    return Err(anyhow!("Only simple SELECT queries are allowed. SQL comments are blocked."));
                }
                // Also block shell-like dot commands used by sqlite3 CLI
                let blocked_dot = [".shell", ".import", ".output", ".read", ".system"];
                if let Some(mat) = re.find(&normalized) {
                    return Err(anyhow!("Only simple SELECT queries are allowed. Blocked keyword: {}", mat.as_str()));
                }
                for dot_cmd in &blocked_dot {
                    if sql.trim().starts_with(dot_cmd) {
                        return Err(anyhow!("Blocked sqlite3 dot-command: {}", dot_cmd));
                    }
                }
                // Must start with SELECT or EXPLAIN (for EXPLAIN QUERY PLAN)
                let trimmed = sql.trim().to_uppercase();
                if !trimmed.starts_with("SELECT") && !trimmed.starts_with("EXPLAIN") {
                    return Err(anyhow!("Only SELECT (or EXPLAIN) queries are allowed for safety."));
                }

                let mut stmt = conn.prepare(sql)
                    .map_err(|e| anyhow!("Failed to prepare query: {}", e))?;
                let col_count = stmt.column_count();
                let mut rows = stmt.query([])
                    .map_err(|e| anyhow!("Failed to execute query: {}", e))?;
                
                let mut output = String::new();
                while let Some(row) = rows.next().map_err(|e| anyhow!("Failed to retrieve row: {}", e))? {
                    let mut row_str = Vec::new();
                    for i in 0..col_count {
                        let val: rusqlite::types::Value = row.get(i)
                            .map_err(|e| anyhow!("Failed to get column value: {}", e))?;
                        let val_str = match val {
                            rusqlite::types::Value::Null => "".to_string(),
                            rusqlite::types::Value::Integer(v) => v.to_string(),
                            rusqlite::types::Value::Real(v) => v.to_string(),
                            rusqlite::types::Value::Text(s) => s,
                            rusqlite::types::Value::Blob(b) => String::from_utf8_lossy(&b).to_string(),
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
                    "description": "Path to the SQLite database file."
                },
                "sql": {
                    "type": "string",
                    "description": "The mutation query statement to execute."
                }
            },
            "required": ["db_path", "sql"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let db_path_raw = arguments.get("db_path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'db_path' parameter"))?;
        let db_path = crate::config::loader::resolve_path(db_path_raw);
        let sql = arguments.get("sql").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'sql' parameter"))?;

        // Safety checks for db_write
        let trimmed_sql = sql.trim_end_matches(';').trim();
        if trimmed_sql.contains(';') {
            return Err(anyhow!("Stacked queries are not allowed. Use separate calls for multiple statements."));
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
        let re = WRITE_BLOCKLIST_RE.get_or_init(|| {
            regex::Regex::new(r"\b(ATTACH|DETACH|LOAD)\b").unwrap()
        });
        if let Some(mat) = re.find(&normalized) {
            return Err(anyhow!("Blocked SQL keyword for safety: {}", mat.as_str()));
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| anyhow!("Failed to open database: {}", e))?;
        
        let changes = conn.execute(sql, [])
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_write() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("openz_db_write_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        let db_file = temp_dir.join("test.db");
        let db_path_str = db_file.to_str().unwrap();

        let tool = DbWriteTool;
        let res = tool.call(&json!({
            "db_path": db_path_str,
            "sql": "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);"
        })).await?;
        assert_eq!(res["status"], "success");

        let res = tool.call(&json!({
            "db_path": db_path_str,
            "sql": "INSERT INTO users (name) VALUES ('Bob');"
        })).await?;
        assert_eq!(res["status"], "success");

        let inspector = DbInspectorTool;
        let res = inspector.call(&json!({
            "db_path": db_path_str,
            "action": "query",
            "sql": "SELECT * FROM users;"
        })).await?;
        assert_eq!(res["status"], "success");
        assert!(res["stdout"].as_str().unwrap().contains("Bob"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_db_inspector_actions() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("openz_db_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        let db_file = temp_dir.join("test.db");
        let db_path_str = db_file.to_str().unwrap();

        // Create table and insert test data via sqlite3 CLI
        let init_status = std::process::Command::new("sqlite3")
            .arg(db_path_str)
            .arg("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO users (name) VALUES ('Alice');")
            .status()?;
        
        if !init_status.success() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Ok(());
        }

        let tool = DbInspectorTool;

        // Test action: schema
        let res = tool.call(&json!({
            "db_path": db_path_str,
            "action": "schema"
        })).await?;
        assert_eq!(res["status"], "success");
        assert!(res["stdout"].as_str().unwrap().contains("CREATE TABLE users"));

        // Test action: query
        let res = tool.call(&json!({
            "db_path": db_path_str,
            "action": "query",
            "sql": "SELECT * FROM users;"
        })).await?;
        assert_eq!(res["status"], "success");
        assert!(res["stdout"].as_str().unwrap().contains("Alice"));

        // Test action: query (invalid mutating query)
        let res = tool.call(&json!({
            "db_path": db_path_str,
            "action": "query",
            "sql": "DROP TABLE users;"
        })).await;
        assert!(res.is_err());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
