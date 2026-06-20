use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;

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

        let mut cmd = Command::new("sqlite3");
        crate::config::loader::set_command_cwd(&mut cmd);
        cmd.arg(db_path);

        match action {
            "schema" => {
                cmd.arg(".schema");
            }
            "query" => {
                let sql = arguments.get("sql").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'sql' parameter for query action"))?;

                // Block dangerous SQL operations — use a strict blocklist
                let sql_upper = sql.to_uppercase();
                let normalized: String = sql_upper.chars().filter(|c| !c.is_whitespace()).collect();
                let blocked_keywords = [
                    "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE",
                    "ATTACH", "DETACH", "PRAGMA", "REINDEX", "REPLACE",
                    "VACUUM", "ANALYZE", "INTO",
                ];
                // Also block shell-like dot commands used by sqlite3 CLI
                let blocked_dot = [".shell", ".import", ".output", ".read", ".system"];
                for kw in &blocked_keywords {
                    if normalized.contains(kw) {
                        return Err(anyhow!("Only simple SELECT queries are allowed. Blocked keyword: {}", kw));
                    }
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
                cmd.arg(sql);
            }
            _ => return Err(anyhow!("Invalid action: {}", action)),
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(json!({
            "status": if output.status.success() { "success" } else { "error" },
            "stdout": stdout,
            "stderr": stderr,
            "code": output.status.code()
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

        let mut cmd = Command::new("sqlite3");
        crate::config::loader::set_command_cwd(&mut cmd);
        cmd.arg(db_path);
        cmd.arg(sql);

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(json!({
            "status": if output.status.success() { "success" } else { "error" },
            "stdout": stdout,
            "stderr": stderr,
            "code": output.status.code()
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
        let init_status = Command::new("sqlite3")
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
