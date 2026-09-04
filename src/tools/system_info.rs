use crate::tools::Tool;
use anyhow::Result;
use serde_json::{json, Value};
use std::process::Command;

pub struct SystemInfoTool;

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

        let disk_usage = if cfg!(target_os = "windows") {
            let out = tokio::task::spawn_blocking(|| {
                Command::new("wmic")
                    .args(["logicaldisk", "get", "size,freespace,caption"])
                    .output()
            })
            .await?;
            out.map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_else(|_| "Unavailable".to_string())
        } else {
            let out = tokio::task::spawn_blocking(|| Command::new("df").arg("-h").output()).await?;
            out.map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_else(|_| "Unavailable".to_string())
        };

        let memory_info = if cfg!(target_os = "linux") {
            std::fs::read_to_string("/proc/meminfo").unwrap_or_else(|_| "Unavailable".to_string())
        } else if cfg!(target_os = "macos") {
            let out = tokio::task::spawn_blocking(|| {
                Command::new("sysctl")
                    .args(["hw.memsize", "vm.page_free_target"])
                    .output()
            })
            .await?;
            out.map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_else(|_| "Unavailable".to_string())
        } else {
            let out = tokio::task::spawn_blocking(|| Command::new("systeminfo").output()).await?;
            out.map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_else(|_| "Unavailable".to_string())
        };

        let mut tools = json!({});
        let tool_checks = vec![
            ("git", vec!["--version"]),
            ("cargo", vec!["--version"]),
            ("npm", vec!["--version"]),
            ("node", vec!["--version"]),
            ("python3", vec!["--version"]),
            ("pip3", vec!["--version"]),
            ("docker", vec!["--version"]),
        ];

        for (tool, args) in tool_checks {
            let tool_name = tool.to_string();
            let args_clone = args.clone();
            let status = tokio::task::spawn_blocking(move || {
                Command::new(&tool_name).args(&args_clone).output()
            })
            .await;
            let available = status.is_ok();
            let version = match status {
                Ok(Ok(ref o)) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
                _ => String::new(),
            };
            tools[tool] = json!({
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_info() -> Result<()> {
        let tool = SystemInfoTool;
        let res = tool.call(&json!({})).await?;
        assert_eq!(res["status"], "success");
        assert!(res["os"].as_str().is_some());
        assert!(res["architecture"].as_str().is_some());
        Ok(())
    }
}
