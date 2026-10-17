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
        let action_raw = arguments
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| {
                arguments
                    .get("action")
                    .or_else(|| arguments.get("command"))
                    .or_else(|| arguments.get("subcommand"))
                    .or_else(|| arguments.get("cmd"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("status")
                    .to_string()
            });
        let action = action_raw.trim().to_lowercase();

        let mut cmd = Command::new("git");

        match arguments
            .get("cwd")
            .or_else(|| arguments.get("dir"))
            .or_else(|| arguments.get("path"))
            .and_then(|v| v.as_str())
        {
            Some(cwd_str) => {
                let path = crate::config::loader::resolve_path(cwd_str);
                cmd.current_dir(path);
            }
            None => {
                crate::config::loader::set_tokio_command_cwd(&mut cmd);
            }
        }

        let target_ref_opt = arguments
            .get("ref")
            .or_else(|| arguments.get("commit"))
            .or_else(|| arguments.get("revision"))
            .and_then(|v| v.as_str());

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

                match (is_staged, target_ref_opt) {
                    (true, _) => {
                        cmd.args(["diff", "--staged"]);
                    }
                    (false, Some(target_ref)) => {
                        cmd.args(["diff", target_ref]);
                    }
                    (false, None) => {
                        cmd.args(["diff", "HEAD"]);
                    }
                }
            }
            "branch" | "branches" | "br" => {
                cmd.args(["branch", "-a"]);
            }
            "show" => {
                cmd.args(["show", target_ref_opt.unwrap_or("HEAD")]);
            }
            "add" | "stage" => {
                let files_val_opt = arguments
                    .get("files")
                    .or_else(|| arguments.get("file"))
                    .or_else(|| arguments.get("path"));
                let files_to_add = match files_val_opt {
                    Some(Value::Array(arr)) => arr
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>(),
                    Some(Value::String(s)) => match s.contains(',') {
                        true => s
                            .split(',')
                            .map(str::trim)
                            .filter(|part| !part.is_empty())
                            .map(str::to_string)
                            .collect::<Vec<_>>(),
                        false => vec![s.to_string()],
                    },
                    _ => Vec::new(),
                };
                match files_to_add.is_empty() {
                    true => {
                        return Err(anyhow!(
                            "At least one file must be specified for 'add' action"
                        ));
                    }
                    false => {}
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
                match message.is_empty() {
                    true => return Err(anyhow!("Commit message cannot be empty")),
                    false => {}
                }
                cmd.args(["commit", "-m", message]);
            }
            "log" | "history" | "l" => {
                let limit = arguments
                    .get("limit")
                    .or_else(|| arguments.get("n"))
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
                    })
                    .unwrap_or(5);
                cmd.args(["log", &format!("-n{}", limit), "--oneline"]);
            }
            _ => return Err(anyhow!("Unsupported git action: {}", action)),
        }

        let output = cmd.output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let status = match output.status.success() {
            true => "success",
            false => "error",
        };

        Ok(json!({
            "status": status,
            "stdout": stdout,
            "stderr": stderr,
            "code": output.status.code()
        }))
    }
}

#[cfg(test)]
#[path = "git_manager_tests.rs"]
mod tests;

