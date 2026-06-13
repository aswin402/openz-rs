use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;

pub struct CargoManagerTool;

#[async_trait::async_trait]
impl Tool for CargoManagerTool {
    fn name(&self) -> &str {
        "cargo_manager"
    }

    fn description(&self) -> &str {
        "Execute cargo toolchain commands (build, test, clippy, fmt) in a workspace."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["build", "test", "clippy", "fmt"],
                    "description": "The cargo command to execute."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional working directory to run the cargo command in (defaults to current directory)."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let mut cmd = Command::new("cargo");

        if let Some(cwd_str) = arguments.get("cwd").and_then(|v| v.as_str()) {
            let path = crate::config::loader::resolve_path(cwd_str);
            cmd.current_dir(path);
        } else {
            crate::config::loader::set_command_cwd(&mut cmd);
        }

        match action {
            "build" => {
                cmd.arg("build");
            }
            "test" => {
                cmd.arg("test");
            }
            "clippy" => {
                cmd.args(&["clippy", "--message-format=json"]);
            }
            "fmt" => {
                cmd.args(&["fmt", "--", "--check"]);
            }
            _ => return Err(anyhow!("Unsupported cargo action: {}", action)),
        }

        let output = cmd.output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if action == "clippy" {
            let mut diagnostics = Vec::new();
            for line in stdout.lines() {
                if let Ok(msg) = serde_json::from_str::<Value>(line) {
                    if let Some(reason) = msg.get("reason").and_then(|v| v.as_str()) {
                        if reason == "compiler-message" {
                            if let Some(message) = msg.get("message") {
                                let level = message.get("level").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let msg_text = message.get("message").and_then(|v| v.as_str()).unwrap_or("");
                                let spans = message.get("spans").and_then(|v| v.as_array());
                                
                                let mut file_path = String::new();
                                let mut line_num = 0;

                                if let Some(spans_arr) = spans {
                                    if !spans_arr.is_empty() {
                                        let first_span = &spans_arr[0];
                                        file_path = first_span.get("file_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        line_num = first_span.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0);
                                    }
                                }

                                diagnostics.push(json!({
                                    "level": level,
                                    "message": msg_text,
                                    "file": file_path,
                                    "line": line_num
                                }));
                            }
                        }
                    }
                }
            }

            return Ok(json!({
                "status": if output.status.success() { "success" } else { "error" },
                "diagnostics": diagnostics,
                "code": output.status.code()
            }));
        }

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
    async fn test_cargo_manager() -> Result<()> {
        let tool = CargoManagerTool;
        let res = tool.call(&json!({
            "action": "clippy"
        })).await?;

        assert_eq!(res["status"], "success");
        assert!(res["diagnostics"].is_array());
        
        Ok(())
    }
}
