use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;

pub struct GitManagerTool;

#[async_trait::async_trait]
impl Tool for GitManagerTool {
    fn name(&self) -> &str {
        "git_manager"
    }

    fn description(&self) -> &str {
        "Perform Git version control operations (status, diff, add, commit, log) directly within the codebase workspace."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "diff", "add", "commit", "log"],
                    "description": "The Git subcommand/action to run."
                },
                "files": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Files to stage/add. Required for 'add'."
                },
                "message": {
                    "type": "string",
                    "description": "The commit message. Required for 'commit'."
                },
                "limit": {
                    "type": "integer",
                    "description": "Limit the number of commits shown in the log. Optional, defaults to 5."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional working directory to run the git command in (defaults to current directory)."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let mut cmd = Command::new("git");

        if let Some(cwd_str) = arguments.get("cwd").and_then(|v| v.as_str()) {
            let path = crate::config::loader::resolve_path(cwd_str);
            cmd.current_dir(path);
        } else {
            crate::config::loader::set_command_cwd(&mut cmd);
        }

        match action {
            "status" => {
                cmd.arg("status");
            }
            "diff" => {
                cmd.args(["diff", "HEAD"]);
            }
            "add" => {
                let files_arr = arguments.get("files").and_then(|v| v.as_array())
                    .ok_or_else(|| anyhow!("Missing 'files' argument for 'add' action"))?;
                if files_arr.is_empty() {
                    return Err(anyhow!("At least one file must be specified for 'add' action"));
                }
                cmd.arg("add");
                for f in files_arr {
                    if let Some(s) = f.as_str() {
                        cmd.arg(s);
                    }
                }
            }
            "commit" => {
                let message = arguments.get("message").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'message' argument for 'commit' action"))?.trim();
                if message.is_empty() {
                    return Err(anyhow!("Commit message cannot be empty"));
                }
                cmd.args(["commit", "-m", message]);
            }
            "log" => {
                let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(5);
                cmd.args(["log", &format!("-n{}", limit), "--oneline"]);
            }
            _ => return Err(anyhow!("Unsupported git action: {}", action)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_git_manager_actions() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("openz_git_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        // Initialize git repo
        let init_status = Command::new("git")
            .arg("init")
            .current_dir(&temp_dir)
            .status()?;
        
        if !init_status.success() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Ok(());
        }

        // Configure git locally
        let _ = Command::new("git").args(&["config", "user.name", "Test User"]).current_dir(&temp_dir).status();
        let _ = Command::new("git").args(&["config", "user.email", "test@example.com"]).current_dir(&temp_dir).status();

        let file_path = temp_dir.join("test.txt");
        std::fs::write(&file_path, "initial content")?;

        let tool = GitManagerTool;
        let cwd_str = temp_dir.to_str().unwrap();

        // 1. Git Status
        let res = tool.call(&json!({
            "action": "status",
            "cwd": cwd_str
        })).await?;
        assert_eq!(res["status"], "success");
        assert!(res["stdout"].as_str().unwrap().contains("test.txt"));

        // 2. Git Add
        let res = tool.call(&json!({
            "action": "add",
            "files": ["test.txt"],
            "cwd": cwd_str
        })).await?;
        assert_eq!(res["status"], "success");

        // 3. Git Commit
        let res = tool.call(&json!({
            "action": "commit",
            "message": "initial commit",
            "cwd": cwd_str
        })).await?;
        assert_eq!(res["status"], "success");

        // 4. Git Log
        let res = tool.call(&json!({
            "action": "log",
            "limit": 1,
            "cwd": cwd_str
        })).await?;
        assert_eq!(res["status"], "success");
        assert!(res["stdout"].as_str().unwrap().contains("initial commit"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
