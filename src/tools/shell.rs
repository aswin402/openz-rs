use crate::tools::Tool;
use anyhow::{Result, anyhow};
use std::process::Command;

pub struct ExecCommandTool;

#[async_trait::async_trait]
impl Tool for ExecCommandTool {
    fn name(&self) -> &str {
        "exec_command"
    }

    fn description(&self) -> &str {
        "Run a shell command on the host system and return its output."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The shell command to execute" }
            },
            "required": ["command"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let command_str = arguments.get("command").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'command' argument"))?;

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", command_str])
                .output()?
        } else {
            Command::new("sh")
                .args(["-c", command_str])
                .output()?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let status_code = output.status.code().unwrap_or(-1);

        Ok(serde_json::json!({
            "status_code": status_code,
            "stdout": stdout,
            "stderr": stderr
        }))
    }
}
