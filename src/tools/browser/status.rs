use super::gsd::{first_non_empty_stream, gsd_browser_bin_path};
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
        match recommended_browser_backend(self) {
            Some(backend) => format!("Browser preflight ok: recommended backend is {:?}.", backend),
            None => "No browser backend available. Run inspect_browsers, install/start Chrome CDP, gsd-browser, or geckodriver, then retry browser fallback once.".to_string(),
        }
    }
}

pub fn recommended_browser_backend(health: &BrowserHealth) -> Option<BrowserBackend> {
    [
        (health.chrome_cdp, BrowserBackend::ChromeCdp),
        (health.gsd_browser, BrowserBackend::GsdBrowser),
        (health.geckodriver, BrowserBackend::GeckoDriver),
    ]
    .into_iter()
    .find_map(|(status, backend)| (status == BrowserBackendStatus::Running).then_some(backend))
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
        .or_else(|| value.get("health"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();

    let is_unhealthy = error_or_message.contains("unhealthy");
    let is_missing = missing_text
        .iter()
        .any(|needle| error_or_message.contains(needle));

    match (status, is_unhealthy, is_missing, !error_or_message.is_empty()) {
        ("running", false, _, _) => BrowserBackendStatus::Running,
        ("unhealthy", _, _, _) | ("running", true, _, _) => BrowserBackendStatus::Broken,
        (_, _, true, _) => BrowserBackendStatus::Missing,
        (_, _, false, true) => BrowserBackendStatus::Broken,
        _ => BrowserBackendStatus::Stopped,
    }
}

pub struct InspectBrowsersTool;

fn get_recent_browser_errors() -> Result<Vec<Value>> {
    let db_path = crate::config::config_dir().join("logs.db");
    match db_path.exists() {
        false => Ok(vec![]),
        true => {
            let conn = rusqlite::Connection::open(&db_path)?;
            let mut stmt = conn.prepare(
                "SELECT timestamp, level, target, message 
                 FROM logs 
                 WHERE (message LIKE '%firefox%' OR message LIKE '%gsd-browser%' OR message LIKE '%obscura%' OR message LIKE '%browser%') 
                   AND (level = 'ERROR' OR level = 'WARN') 
                 ORDER BY timestamp DESC 
                 LIMIT 10",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(json!({
                    "timestamp": row.get::<_, String>(0)?,
                    "level": row.get::<_, String>(1)?,
                    "target": row.get::<_, String>(2)?,
                    "message": row.get::<_, String>(3)?,
                }))
            })?;

            Ok(rows.flatten().collect())
        }
    }
}

async fn probe_json_endpoint(
    client: &reqwest::Client,
    url: &str,
    port: u16,
    ok_key: &str,
    parse_err_msg: &str,
    unresponsive_msg: &str,
) -> Value {
    match client.get(url).send().await {
        Ok(res) => match res.json::<Value>().await {
            Ok(val) => json!({
                "status": "running",
                ok_key: val
            }),
            Err(_) => json!({ "status": "running", "message": parse_err_msg }),
        },
        Err(_) => match tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .is_ok()
        {
            true => json!({ "status": "running", "message": unresponsive_msg }),
            false => json!({ "status": "stopped" }),
        },
    }
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
        let client = crate::core::http::custom_http_client(
            Duration::from_secs(1),
            Duration::from_secs(2),
        );

        let firefox_status = probe_json_endpoint(
            &client,
            "http://127.0.0.1:4444/status",
            4444,
            "details",
            "Failed to parse geckodriver status JSON",
            "Port 4444 open, geckodriver status endpoint unresponsive",
        )
        .await;

        let cdp_port = super::common::browser_cdp_port();
        let obscura_status = probe_json_endpoint(
            &client,
            &format!("http://127.0.0.1:{cdp_port}/json/list"),
            cdp_port,
            "pages",
            "Failed to parse Chrome JSON target list",
            &format!("Port {cdp_port} open, Chrome list endpoint unresponsive"),
        )
        .await;

        let bin_path = gsd_browser_bin_path();
        let mut gsd_health_cmd = Command::new(&bin_path);
        gsd_health_cmd.arg("daemon").arg("health");
        let gsd_status = match gsd_health_cmd.output().await {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                match out.status.success() {
                    true => {
                        let is_unhealthy = stdout.to_lowercase().contains("unhealthy");
                        let mut gsd_pages_cmd = Command::new(&bin_path);
                        gsd_pages_cmd.arg("list-pages").arg("--json");
                        let pages_val = gsd_pages_cmd
                            .output()
                            .await
                            .ok()
                            .filter(|p_out| p_out.status.success())
                            .map(|p_out| {
                                let p_stdout =
                                    String::from_utf8_lossy(&p_out.stdout).to_string();
                                serde_json::from_str::<Value>(&p_stdout)
                                    .unwrap_or_else(|_| json!(p_stdout.trim()))
                            })
                            .unwrap_or_else(|| json!([]));

                        let status_str = match is_unhealthy {
                            true => "unhealthy",
                            false => "running",
                        };
                        json!({
                            "status": status_str,
                            "health": stdout.trim(),
                            "pages": pages_val
                        })
                    }
                    false => json!({
                        "status": "stopped",
                        "error": first_non_empty_stream(&stderr, &stdout)
                    }),
                }
            }
            Err(e) => json!({
                "status": "stopped",
                "error": format!("Failed to run gsd-browser binary: {:?}", e)
            }),
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
#[path = "status_tests.rs"]
mod tests;
