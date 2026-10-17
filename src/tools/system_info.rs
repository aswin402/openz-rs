use crate::tools::Tool;
use anyhow::Result;
use serde_json::{json, Value};
use std::process::Command;

pub struct SystemInfoTool;

async fn run_command_stdout(program: &'static str, args: &'static [&'static str]) -> Result<String> {
    let out = tokio::task::spawn_blocking(move || Command::new(program).args(args).output()).await?;
    Ok(out
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|_| "Unavailable".to_string()))
}

#[async_trait::async_trait]
impl Tool for SystemInfoTool {
    fn name(&self) -> &str {
        "system_info"
    }

    fn description(&self) -> &str {
        "Retrieve system diagnostics, including host OS details, CPU/memory statistics, disk usage, and running processes."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let os_type = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();

        let disk_usage = match cfg!(target_os = "windows") {
            true => {
                run_command_stdout("wmic", &["logicaldisk", "get", "size,freespace,caption"]).await?
            }
            false => run_command_stdout("df", &["-h"]).await?,
        };

        let memory_info = match (cfg!(target_os = "linux"), cfg!(target_os = "macos")) {
            (true, _) => {
                std::fs::read_to_string("/proc/meminfo").unwrap_or_else(|_| "Unavailable".to_string())
            }
            (false, true) => {
                run_command_stdout("sysctl", &["hw.memsize", "vm.page_free_target"]).await?
            }
            (false, false) => run_command_stdout("systeminfo", &[]).await?,
        };

        let mut tools = json!({});
        let tool_checks: &[(&str, &[&str])] = &[
            ("git", &["--version"]),
            ("cargo", &["--version"]),
            ("npm", &["--version"]),
            ("node", &["--version"]),
            ("python3", &["--version"]),
            ("pip3", &["--version"]),
            ("docker", &["--version"]),
        ];

        for (tool, args) in tool_checks {
            let tool_name = (*tool).to_string();
            let args_slice = *args;
            let status = tokio::task::spawn_blocking(move || {
                Command::new(&tool_name).args(args_slice).output()
            })
            .await;
            let available = status.is_ok();
            let version = match status {
                Ok(Ok(ref o)) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
                _ => String::new(),
            };
            tools[*tool] = json!({
                "available": available,
                "version": version
            });
        }

        Ok(json!({
            "status": "success",
            "os": os_type,
            "architecture": arch,
            "disk_usage": disk_usage,
            "memory": memory_info,
            "developer_tools": tools
        }))
    }
}

#[cfg(test)]
#[path = "system_info_tests.rs"]
mod tests;
