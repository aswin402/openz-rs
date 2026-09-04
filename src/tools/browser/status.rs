use crate::tools::Tool;
use anyhow::Result;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBackendStatus {
    Running,
    Stopped,
    Missing,
    Broken,
}

impl BrowserBackendStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            BrowserBackendStatus::Running => "running",
            BrowserBackendStatus::Stopped => "stopped",
            BrowserBackendStatus::Missing => "missing",
            BrowserBackendStatus::Broken => "broken",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBackend {
    ChromeCdp,
    GsdBrowser,
    GeckoDriver,
}

impl BrowserBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            BrowserBackend::ChromeCdp => "chrome_cdp",
            BrowserBackend::GsdBrowser => "gsd_browser",
            BrowserBackend::GeckoDriver => "geckodriver",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserHealth {
    pub chrome_cdp: BrowserBackendStatus,
    pub gsd_browser: BrowserBackendStatus,
    pub geckodriver: BrowserBackendStatus,
}

impl BrowserHealth {
    pub fn to_json(&self) -> Value {
        json!({
            "chrome_cdp": self.chrome_cdp.as_str(),
            "gsd_browser": self.gsd_browser.as_str(),
            "geckodriver": self.geckodriver.as_str(),
        })
    }

    pub fn actionable_summary(&self) -> String {
        if recommended_browser_backend(self).is_none() {
            return "No browser backend available. Run inspect_browsers, install/start Chrome CDP, gsd-browser, or geckodriver, then retry browser fallback once.".to_string();
        }
        format!(
            "Browser preflight ok: recommended backend is {:?}.",
            recommended_browser_backend(self).expect("checked above")
        )
    }
}

pub fn recommended_browser_backend(health: &BrowserHealth) -> Option<BrowserBackend> {
    if health.chrome_cdp == BrowserBackendStatus::Running {
        Some(BrowserBackend::ChromeCdp)
    } else if health.gsd_browser == BrowserBackendStatus::Running {
        Some(BrowserBackend::GsdBrowser)
    } else if health.geckodriver == BrowserBackendStatus::Running {
        Some(BrowserBackend::GeckoDriver)
    } else {
        None
    }
}

pub fn browser_preflight_value(health: &BrowserHealth) -> Value {
    json!({
        "health": health.to_json(),
        "recommended_backend": recommended_browser_backend(health).map(|backend| backend.as_str()),
        "summary": health.actionable_summary(),
    })
}

pub fn browser_preflight_error_value(error: &str, health: BrowserHealth) -> Value {
    json!({
        "error": error,
        "browser_preflight": browser_preflight_value(&health),
    })
}

fn status_value_to_backend_status(value: &Value, missing_text: &[&str]) -> BrowserBackendStatus {
    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("stopped");
    let error_or_message = value
        .get("error")
        .or_else(|| value.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    match status {
        "running" => BrowserBackendStatus::Running,
        _ if missing_text
            .iter()
            .any(|needle| error_or_message.contains(needle)) =>
        {
            BrowserBackendStatus::Missing
        }
        _ if !error_or_message.is_empty() => BrowserBackendStatus::Broken,
        _ => BrowserBackendStatus::Stopped,
    }
}

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
    for entry in rows.flatten() {
        errors.push(entry);
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
                if tokio::net::TcpStream::connect("127.0.0.1:4444")
                    .await
                    .is_ok()
                {
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
                if tokio::net::TcpStream::connect("127.0.0.1:9222")
                    .await
                    .is_ok()
                {
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
                            serde_json::from_str::<Value>(&p_stdout)
                                .unwrap_or_else(|_| json!(p_stdout.trim()))
                        }
                        _ => json!([]),
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

        let health = BrowserHealth {
            chrome_cdp: status_value_to_backend_status(
                &obscura_status,
                &["not found", "no such file"],
            ),
            gsd_browser: status_value_to_backend_status(
                &gsd_status,
                &["not found", "failed to run gsd-browser binary"],
            ),
            geckodriver: status_value_to_backend_status(
                &firefox_status,
                &["not found", "no such file", "missing"],
            ),
        };

        Ok(json!({
            "firefox_geckodriver": firefox_status,
            "chrome_obscura": obscura_status,
            "gsd_browser": gsd_status,
            "browser_preflight": browser_preflight_value(&health),
            "managed_tasks": crate::tools::task_manager::list_tasks(),
            "recent_browser_errors": recent_errors
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_preflight_prefers_running_chrome_cdp() {
        let health = BrowserHealth {
            chrome_cdp: BrowserBackendStatus::Running,
            gsd_browser: BrowserBackendStatus::Stopped,
            geckodriver: BrowserBackendStatus::Missing,
        };

        assert_eq!(
            recommended_browser_backend(&health),
            Some(BrowserBackend::ChromeCdp)
        );
    }

    #[test]
    fn browser_preflight_reports_missing_all_backends() {
        let health = BrowserHealth {
            chrome_cdp: BrowserBackendStatus::Missing,
            gsd_browser: BrowserBackendStatus::Missing,
            geckodriver: BrowserBackendStatus::Missing,
        };

        assert_eq!(recommended_browser_backend(&health), None);
        assert!(health
            .actionable_summary()
            .contains("No browser backend available"));
    }

    #[test]
    fn browser_preflight_error_payload_is_machine_readable() {
        let payload = browser_preflight_error_value(
            "geckodriver missing",
            BrowserHealth {
                chrome_cdp: BrowserBackendStatus::Stopped,
                gsd_browser: BrowserBackendStatus::Stopped,
                geckodriver: BrowserBackendStatus::Missing,
            },
        );

        assert_eq!(payload["error"], "geckodriver missing");
        assert_eq!(
            payload["browser_preflight"]["health"]["geckodriver"],
            "missing"
        );
        assert!(payload["browser_preflight"]["summary"]
            .as_str()
            .expect("summary string")
            .contains("No browser backend available"));
    }

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
        assert!(res.get("browser_preflight").is_some());
        assert!(res.get("recent_browser_errors").is_some());
        Ok(())
    }
}
