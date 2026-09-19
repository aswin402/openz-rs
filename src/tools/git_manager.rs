use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::process::Command;

pub struct GitManagerTool;

#[async_trait::async_trait]
impl Tool for GitManagerTool {
    fn name(&self) -> &str {
        "git_manager"
    }

    fn description(&self) -> &str {
        "Perform Git version control operations (status, diff, add, commit, log, branch, show) directly within the codebase workspace."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "diff", "add", "commit", "log", "branch", "show"],
                    "description": "The Git subcommand/action to run (defaults to 'status')."
                },
                "files": {
                    "description": "Files to stage/add (array of file paths or single string path/comma-separated). Required for 'add'.",
                    "oneOf": [
                        { "type": "array", "items": { "type": "string" } },
                        { "type": "string" }
                    ]
                },
                "message": {
                    "type": "string",
                    "description": "The commit message. Required for 'commit'."
                },
                "limit": {
                    "description": "Limit the number of commits shown in the log (integer or numeric string). Optional, defaults to 5.",
                    "oneOf": [
                        { "type": "integer" },
                        { "type": "string" }
                    ]
                },
                "ref": {
                    "type": "string",
                    "description": "Target commit hash, revision, or branch for 'show' or 'diff'."
                },
                "staged": {
                    "type": "boolean",
                    "description": "If true for 'diff', compare staged changes (--staged/--cached)."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional working directory to run the git command in (defaults to current directory)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action_raw = if let Some(s) = arguments.as_str() {
            s.to_string()
        } else {
            arguments
                .get("action")
                .or_else(|| arguments.get("command"))
                .or_else(|| arguments.get("subcommand"))
                .or_else(|| arguments.get("cmd"))
                .and_then(|v| v.as_str())
                .unwrap_or("status")
                .to_string()
        };
        let action = action_raw.trim().to_lowercase();

        let mut cmd = Command::new("git");

        if let Some(cwd_str) = arguments
            .get("cwd")
            .or_else(|| arguments.get("dir"))
            .or_else(|| arguments.get("path"))
            .and_then(|v| v.as_str())
        {
            let path = crate::config::loader::resolve_path(cwd_str);
            cmd.current_dir(path);
        } else {
            crate::config::loader::set_tokio_command_cwd(&mut cmd);
        }

        match action.as_str() {
            "status" | "st" | "s" => {
                cmd.arg("status");
            }
            "diff" | "d" => {
                let is_staged = arguments
                    .get("staged")
                    .or_else(|| arguments.get("cached"))
                    .and_then(|v| {
                        v.as_bool().or_else(|| {
                            v.as_str().map(|s| s.trim().eq_ignore_ascii_case("true"))
                        })
                    })
                    .unwrap_or(false);

                if is_staged {
                    cmd.args(["diff", "--staged"]);
                } else if let Some(target_ref) = arguments
                    .get("ref")
                    .or_else(|| arguments.get("commit"))
                    .or_else(|| arguments.get("revision"))
                    .and_then(|v| v.as_str())
                {
                    cmd.args(["diff", target_ref]);
                } else {
                    cmd.args(["diff", "HEAD"]);
                }
            }
            "branch" | "branches" | "br" => {
                cmd.args(["branch", "-a"]);
            }
            "show" => {
                if let Some(target_ref) = arguments
                    .get("ref")
                    .or_else(|| arguments.get("commit"))
                    .or_else(|| arguments.get("revision"))
                    .and_then(|v| v.as_str())
                {
                    cmd.args(["show", target_ref]);
                } else {
                    cmd.args(["show", "HEAD"]);
                }
            }
            "add" | "stage" => {
                let mut files_to_add = Vec::new();
                if let Some(files_val) = arguments
                    .get("files")
                    .or_else(|| arguments.get("file"))
                    .or_else(|| arguments.get("path"))
                {
                    if let Some(arr) = files_val.as_array() {
                        for item in arr {
                            if let Some(s) = item.as_str() {
                                files_to_add.push(s.to_string());
                            }
                        }
                    } else if let Some(s) = files_val.as_str() {
                        if s.contains(',') {
                            for part in s.split(',') {
                                let trimmed = part.trim();
                                if !trimmed.is_empty() {
                                    files_to_add.push(trimmed.to_string());
                                }
                            }
                        } else {
                            files_to_add.push(s.to_string());
                        }
                    }
                }
                if files_to_add.is_empty() {
                    return Err(anyhow!(
                        "At least one file must be specified for 'add' action"
                    ));
                }
                cmd.arg("add");
                for f in files_to_add {
                    cmd.arg(f);
                }
            }
            "commit" | "ci" => {
                let message = arguments
                    .get("message")
                    .or_else(|| arguments.get("msg"))
                    .or_else(|| arguments.get("m"))
                    .or_else(|| arguments.get("description"))
                    .or_else(|| arguments.get("text"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'message' argument for 'commit' action"))?
                    .trim();
                if message.is_empty() {
                    return Err(anyhow!("Commit message cannot be empty"));
                }
                cmd.args(["commit", "-m", message]);
            }
            "log" | "history" | "l" => {
                let limit = arguments
                    .get("limit")
                    .or_else(|| arguments.get("n"))
                    .and_then(|v| {
                        v.as_u64().or_else(|| {
                            v.as_str().and_then(|s| s.trim().parse::<u64>().ok())
                        })
                    })
                    .unwrap_or(5);
                cmd.args(["log", &format!("-n{}", limit), "--oneline"]);
            }
            _ => return Err(anyhow!("Unsupported git action: {}", action)),
        }

        let output = cmd.output().await?;

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
#[path = "git_manager_tests.rs"]
mod tests;

