use crate::tools::Tool;
use anyhow::Result;
use serde_json::{json, Value};
use tokio::process::Command;
use std::time::Duration;

pub struct InspectBrowsersTool;

fn get_recent_browser_errors() -> Result<Vec<Value>> {
    let db_path = crate::config::config_dir().join("logs.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }
    let conn = rusqlite::Connection::open(&db_path)?;
    let mut stmt = conn.prepare(
        "SELECT timestamp, level, target, message 
         FROM logs 
         WHERE (message LIKE '%firefox%' OR message LIKE '%gsd-browser%' OR message LIKE '%obscura%' OR message LIKE '%browser%') 
           AND (level = 'ERROR' OR level = 'WARN') 
         ORDER BY timestamp DESC 
         LIMIT 10"
    )?;
    
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "timestamp": row.get::<_, String>(0)?,
            "level": row.get::<_, String>(1)?,
            "target": row.get::<_, String>(2)?,
            "message": row.get::<_, String>(3)?,
        }))
    })?;

    let mut errors = Vec::new();
    for row in rows {
        if let Ok(entry) = row {
            errors.push(entry);
        }
    }
    Ok(errors)
}

#[async_trait::async_trait]
impl Tool for InspectBrowsersTool {
    fn name(&self) -> &str {
        "inspect_browsers"
    }

    fn description(&self) -> &str {
        "Inspect active browser status, running background daemons, open pages/tabs, displaying URLs, and recent browser errors."
    }

    fn metadata(&self) -> crate::tools::ToolMetadata {
        let mut m = crate::tools::ToolMetadata::infer(self.name());
        m.domain = "browser";
        m.risk = crate::tools::ToolRisk::Low;
        m.uses_network = true;
        m
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(800))
            .build()?;

        // 1. Check Firefox (GeckoDriver)
        let firefox_status = match client.get("http://127.0.0.1:4444/status").send().await {
            Ok(res) => {
                if let Ok(val) = res.json::<Value>().await {
                    json!({
                        "status": "running",
                        "details": val
                    })
                } else {
                    json!({ "status": "running", "message": "Failed to parse geckodriver status JSON" })
                }
            }
            Err(_) => {
                if tokio::net::TcpStream::connect("127.0.0.1:4444").await.is_ok() {
                    json!({ "status": "running", "message": "Port 4444 open, geckodriver status endpoint unresponsive" })
                } else {
                    json!({ "status": "stopped" })
                }
            }
        };

        // 2. Check Chrome (Obscura CDP)
        let obscura_status = match client.get("http://127.0.0.1:9222/json/list").send().await {
            Ok(res) => {
                if let Ok(val) = res.json::<Value>().await {
                    json!({
                        "status": "running",
                        "pages": val
                    })
                } else {
                    json!({ "status": "running", "message": "Failed to parse Chrome JSON target list" })
                }
            }
            Err(_) => {
                if tokio::net::TcpStream::connect("127.0.0.1:9222").await.is_ok() {
                    json!({ "status": "running", "message": "Port 9222 open, Chrome list endpoint unresponsive" })
                } else {
                    json!({ "status": "stopped" })
                }
            }
        };

        // 3. Check gsd_browser
        let bin_path = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("gsd-browser");
            if p.exists() {
                p
            } else {
                std::path::PathBuf::from("gsd-browser")
            }
        } else {
            std::path::PathBuf::from("gsd-browser")
        };

        let mut gsd_health_cmd = Command::new(&bin_path);
        gsd_health_cmd.arg("daemon").arg("health");
        let gsd_status = match gsd_health_cmd.output().await {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if out.status.success() {
                    let mut gsd_pages_cmd = Command::new(&bin_path);
                    gsd_pages_cmd.arg("list-pages").arg("--json");
                    let pages_val = match gsd_pages_cmd.output().await {
                        Ok(p_out) if p_out.status.success() => {
                            let p_stdout = String::from_utf8_lossy(&p_out.stdout).to_string();
                            serde_json::from_str::<Value>(&p_stdout).unwrap_or_else(|_| json!(p_stdout.trim()))
                        }
                        _ => json!([])
                    };

                    json!({
                        "status": "running",
                        "health": stdout.trim(),
                        "pages": pages_val
                    })
                } else {
                    json!({
                        "status": "stopped",
                        "error": if stderr.trim().is_empty() { stdout.trim().to_string() } else { stderr.trim().to_string() }
                    })
                }
            }
            Err(e) => {
                json!({
                    "status": "stopped",
                    "error": format!("Failed to run gsd-browser binary: {:?}", e)
                })
            }
        };

        let recent_errors = get_recent_browser_errors().unwrap_or_else(|_| vec![]);

        Ok(json!({
            "firefox_geckodriver": firefox_status,
            "chrome_obscura": obscura_status,
            "gsd_browser": gsd_status,
            "recent_browser_errors": recent_errors
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inspect_browsers_metadata() -> Result<()> {
        let tool = InspectBrowsersTool;
        assert_eq!(tool.name(), "inspect_browsers");
        assert_eq!(tool.metadata().domain, "browser");
        Ok(())
    }

    #[tokio::test]
    async fn test_inspect_browsers_execution() -> Result<()> {
        let tool = InspectBrowsersTool;
        let res = tool.call(&serde_json::json!({})).await?;
        assert!(res.get("firefox_geckodriver").is_some());
        assert!(res.get("chrome_obscura").is_some());
        assert!(res.get("gsd_browser").is_some());
        assert!(res.get("recent_browser_errors").is_some());
        Ok(())
    }
}
