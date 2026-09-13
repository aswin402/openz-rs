use super::*;

#[tokio::test]
async fn test_git_manager_actions() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_git_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    // Initialize git repo
    let init_status = Command::new("git")
        .arg("init")
        .current_dir(&temp_dir)
        .status()
        .await?;

    if !init_status.success() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Ok(());
    }

    // Configure git locally
    let _ = Command::new("git")
        .args(&["config", "user.name", "Test User"])
        .current_dir(&temp_dir)
        .status()
        .await;
    let _ = Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(&temp_dir)
        .status()
        .await;

    let file_path = temp_dir.join("test.txt");
    std::fs::write(&file_path, "initial content")?;

    let tool = GitManagerTool;
    let cwd_str = temp_dir.to_str().unwrap();

    // 1. Git Status
    let res = tool
        .call(&json!({
            "action": "status",
            "cwd": cwd_str
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("test.txt"));

    // 2. Git Add
    let res = tool
        .call(&json!({
            "action": "add",
            "files": ["test.txt"],
            "cwd": cwd_str
        }))
        .await?;
    assert_eq!(res["status"], "success");

    // 3. Git Commit
    let res = tool
        .call(&json!({
            "action": "commit",
            "message": "initial commit",
            "cwd": cwd_str
        }))
        .await?;
    assert_eq!(res["status"], "success");

    // 4. Git Log
    let res = tool
        .call(&json!({
            "action": "log",
            "limit": 1,
            "cwd": cwd_str
        }))
        .await?;
    assert_eq!(res["status"], "success");
    assert!(res["stdout"].as_str().unwrap().contains("initial commit"));

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
