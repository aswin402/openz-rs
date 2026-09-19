use super::*;

#[tokio::test]
async fn test_db_write() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_db_write_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let db_file = temp_dir.join("test.db");
    let db_path_str = db_file.to_str().unwrap();

    let tool = DbWriteTool;
    let res = tool
        .call(&json!({
            "db_path": db_path_str,
            "sql": "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);"
        }))
        .await?;
    assert_eq!(res["status"], "success");

    let res = tool
        .call(&json!({
            "db_path": db_path_str,
            "sql": "INSERT INTO users (name) VALUES ('Bob');"
        }))
        .await?;
    assert_eq!(res["status"], "success");

    let inspector = DbInspectorTool;
    let res = inspector
        .call(&json!({
            "db_path": db_path_str,
            "action": "query",
            "sql": "SELECT * FROM users;"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("Bob"));

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn db_write_blocks_outside_safe_paths() {
    let tool = DbWriteTool;
    let res = tool
        .call(&json!({
            "db_path": "/etc/passwd",
            "sql": "CREATE TABLE blocked (id INTEGER);"
        }))
        .await;

    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Path traversal prevention"));
}

#[tokio::test]
async fn db_inspector_blocks_outside_safe_paths() {
    let tool = DbInspectorTool;
    let res = tool
        .call(&json!({
            "db_path": "/etc/passwd",
            "action": "schema"
        }))
        .await;

    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Path traversal prevention"));
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
    let res = tool
        .call(&json!({
            "db_path": db_path_str,
            "action": "schema"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"]
        .as_str()
        .unwrap()
        .contains("CREATE TABLE users"));

    // Test action: query
    let res = tool
        .call(&json!({
            "db_path": db_path_str,
            "action": "query",
            "sql": "SELECT * FROM users;"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("Alice"));

    // Test action: query (invalid mutating query)
    let res = tool
        .call(&json!({
            "db_path": db_path_str,
            "action": "query",
            "sql": "DROP TABLE users;"
        }))
        .await;
    assert!(res.is_err());

    // Test auto-inferred query action with parameter aliases
    let res = tool
        .call(&json!({
            "path": db_path_str,
            "query": "SELECT name FROM users;"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("Alice"));

    // Test auto-inferred schema action (no action, no query)
    let res = tool
        .call(&json!({
            "path": db_path_str
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("CREATE TABLE users"));

    // Test direct string calling with db_path
    let res = tool.call(&json!(db_path_str)).await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("CREATE TABLE users"));

    // Test file_path alias and uri with file:// prefix
    let res = tool
        .call(&json!({
            "file_path": format!("file://{}", db_path_str),
            "cmd": "query",
            "select": "SELECT count(*) FROM users;"
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("1"));

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_db_default_memory_database() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_db_default_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let write_tool = DbWriteTool;
            let res = write_tool
                .call(&json!({
                    "sql": "CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, note TEXT);"
                }))
                .await
                .unwrap();
            assert_eq!(res["status"], "success");

            let res = write_tool
                .call(&json!("INSERT INTO notes (note) VALUES ('test_note');"))
                .await
                .unwrap();
            assert_eq!(res["status"], "success");

            let inspector = DbInspectorTool;
            let res = inspector
                .call(&json!("SELECT note FROM notes;"))
                .await
                .unwrap();
            assert_eq!(res["status"], "success");
            assert!(res["stdout"].as_str().unwrap().contains("test_note"));
        })
        .await;

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

