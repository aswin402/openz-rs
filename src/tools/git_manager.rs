use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::process::Command;

pub struct GitManagerTool;

fn validate_safe_branch_name<'a>(raw: &'a str, field_label: &str) -> Result<&'a str> {
    let trimmed = raw.trim();
    let invalid = trimmed.is_empty()
        || trimmed.starts_with('-')
        || trimmed.contains("..")
        || trimmed.chars().any(|c| {
            c.is_whitespace() || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
        });
    match invalid {
        true => Err(anyhow!("Invalid {} '{}'", field_label, trimmed)),
        false => Ok(trimmed),
    }
}

fn parse_positive_line_arg(arguments: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| arguments.get(*key))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
        })
        .filter(|&n| n > 0)
}

#[async_trait::async_trait]
impl Tool for GitManagerTool {
    fn name(&self) -> &str {
        "git_manager"
    }

    fn description(&self) -> &str {
        "Perform Git version control operations (status, diff, add, commit, log, branch, create_branch, switch, blame, stash, show) directly within the codebase workspace."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "status",
                        "diff",
                        "add",
                        "commit",
                        "log",
                        "branch",
                        "create_branch",
                        "switch",
                        "blame",
                        "stash",
                        "show"
                    ],
                    "description": "The Git subcommand/action to run (defaults to 'status')."
                },
                "files": {
                    "description": "Files to stage/add (array of file paths or single string path/comma-separated). Required for 'add'.",
                    "oneOf": [
                        { "type": "array", "items": { "type": "string" } },
                        { "type": "string" }
                    ]
                },
                "file": {
                    "type": "string",
                    "description": "Target file path for 'blame' (or single-file 'add')."
                },
                "start_line": {
                    "type": "integer",
                    "description": "Optional 1-indexed start line for 'blame'."
                },
                "end_line": {
                    "type": "integer",
                    "description": "Optional 1-indexed end line for 'blame'."
                },
                "branch": {
                    "type": "string",
                    "description": "Target branch name for 'create_branch' or 'switch' (alias: name)."
                },
                "stash_action": {
                    "type": "string",
                    "enum": ["list", "push", "pop", "show"],
                    "description": "Sub-action when action='stash' (defaults to 'list')."
                },
                "message": {
                    "type": "string",
                    "description": "Commit message (required for 'commit', optional label for 'stash' push)."
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
                    "description": "Target commit hash, revision, or base branch for 'show', 'diff', 'blame', or 'create_branch'."
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
            "create_branch" | "new_branch" => {
                let raw_branch = arguments
                    .get("branch")
                    .or_else(|| arguments.get("name"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        anyhow!("Missing 'branch' argument for 'create_branch' action")
                    })?;
                let safe_branch = validate_safe_branch_name(raw_branch, "branch")?;
                cmd.args(["checkout", "-b", safe_branch]);
                match target_ref_opt {
                    Some(base_ref) => {
                        let safe_base = validate_safe_branch_name(base_ref, "ref")?;
                        cmd.arg(safe_base);
                    }
                    None => {}
                }
            }
            "switch" | "checkout_branch" => {
                let raw_branch = arguments
                    .get("branch")
                    .or_else(|| arguments.get("name"))
                    .or_else(|| arguments.get("ref"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("Missing 'branch' argument for 'switch' action"))?;
                let safe_branch = validate_safe_branch_name(raw_branch, "branch")?;
                cmd.args(["checkout", safe_branch]);
            }
            "blame" => {
                let target_file = arguments
                    .get("file")
                    .or_else(|| arguments.get("file_path"))
                    .and_then(Value::as_str)
                    .or_else(|| {
                        arguments
                            .get("files")
                            .and_then(Value::as_array)
                            .and_then(|arr| arr.first())
                            .and_then(Value::as_str)
                    })
                    .or_else(|| arguments.get("files").and_then(Value::as_str))
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && !s.starts_with('-'))
                    .ok_or_else(|| anyhow!("Missing 'file' argument for 'blame' action"))?;

                cmd.args(["blame", "-n"]);

                let start_line = parse_positive_line_arg(arguments, &["start_line", "startLine"]);
                let end_line = parse_positive_line_arg(arguments, &["end_line", "endLine"]);
                match (start_line, end_line) {
                    (Some(s), Some(e)) => {
                        cmd.args(["-L", &format!("{},{}", s, e.max(s))]);
                    }
                    (Some(s), None) => {
                        cmd.args(["-L", &format!("{},{}", s, s + 20)]);
                    }
                    (None, _) => {}
                }

                match target_ref_opt {
                    Some(rev) => {
                        let safe_rev = validate_safe_branch_name(rev, "ref")?;
                        cmd.arg(safe_rev);
                    }
                    None => {}
                }
                cmd.args(["--", target_file]);
            }
            "stash" => {
                let sub = arguments
                    .get("stash_action")
                    .or_else(|| arguments.get("mode"))
                    .and_then(Value::as_str)
                    .unwrap_or("list")
                    .trim()
                    .to_ascii_lowercase();
                match sub.as_str() {
                    "list" => {
                        cmd.args(["stash", "list"]);
                    }
                    "push" | "save" => {
                        cmd.args(["stash", "push"]);
                        match arguments
                            .get("message")
                            .or_else(|| arguments.get("msg"))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            Some(msg) => {
                                cmd.args(["-m", msg]);
                            }
                            None => {}
                        }
                    }
                    "pop" => {
                        cmd.args(["stash", "pop"]);
                    }
                    "show" => {
                        cmd.args(["stash", "show", "-p"]);
                    }
                    _ => return Err(anyhow!("Unsupported stash_action '{}'", sub)),
                }
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

