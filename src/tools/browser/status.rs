//! Native OpenZ Browser health & session inspection helpers (used by `browser(action="status")`
//! and `diagnose_system`).

use super::client::list_active_browser_sessions;
use super::webkit_ffi::probe_webkit_runtime;
use anyhow::Result;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBackendStatus {
    Running,
    Ready,
    Missing,
    Broken,
}

impl BrowserBackendStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::Broken => "broken",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBackend {
    OpenzNativeWebKit,
}

impl BrowserBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenzNativeWebKit => "openz_webkit2gtk",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserHealth {
    pub native_webkit: BrowserBackendStatus,
    pub active_sessions: usize,
}

impl BrowserHealth {
    pub fn to_json(&self) -> Value {
        json!({
            "openz_webkit2gtk": self.native_webkit.as_str(),
            "active_sessions": self.active_sessions,
        })
    }

    pub fn actionable_summary(&self) -> String {
        match self.native_webkit {
            BrowserBackendStatus::Running => format!(
                "Native OpenZ WebKit2GTK browser running ({} active session(s)).",
                self.active_sessions
            ),
            BrowserBackendStatus::Ready => {
                "Native OpenZ WebKit2GTK browser runtime ready (0 MB idle RAM; spawns on demand)."
                    .to_string()
            }
            BrowserBackendStatus::Missing | BrowserBackendStatus::Broken => {
                "Native WebKit2GTK runtime unavailable. Install libwebkit2gtk-4.1-0 and libgtk-3-0 (and ensure WAYLAND_DISPLAY or DISPLAY is available), then check browser(action=\"status\")."
                    .to_string()
            }
        }
    }
}

pub fn recommended_browser_backend(health: &BrowserHealth) -> Option<BrowserBackend> {
    matches!(
        health.native_webkit,
        BrowserBackendStatus::Running | BrowserBackendStatus::Ready
    )
    .then_some(BrowserBackend::OpenzNativeWebKit)
}

pub fn browser_preflight_value(health: &BrowserHealth) -> Value {
    json!({
        "health": health.to_json(),
        "recommended_backend": recommended_browser_backend(health).map(BrowserBackend::as_str),
        "summary": health.actionable_summary(),
    })
}

pub fn browser_preflight_error_value(error: &str, health: BrowserHealth) -> Value {
    json!({
        "error": error,
        "browser_preflight": browser_preflight_value(&health),
    })
}

fn get_recent_browser_errors() -> Result<Vec<Value>> {
    let db_path = crate::config::config_dir().join("logs.db");
    match db_path.exists() {
        false => Ok(vec![]),
        true => {
            let conn = rusqlite::Connection::open(&db_path)?;
            let mut stmt = conn.prepare(
                "SELECT timestamp, level, target, message
                 FROM logs
                 WHERE (message LIKE '%browser%' OR message LIKE '%webkit%')
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

pub async fn collect_browser_status_report() -> Result<Value> {
    let probe = probe_webkit_runtime();
    let sessions = list_active_browser_sessions().await;
    let cfg = crate::config::loader::load_config().unwrap_or_default();

    let backend_state = match (probe.available, !sessions.is_empty()) {
        (true, true) => BrowserBackendStatus::Running,
        (true, false) => BrowserBackendStatus::Ready,
        (false, _) => BrowserBackendStatus::Missing,
    };

    let health = BrowserHealth {
        native_webkit: backend_state,
        active_sessions: sessions.len(),
    };
    let recent_errors = get_recent_browser_errors().unwrap_or_default();

    Ok(json!({
        "status": "success",
        "engine": "openz_webkit2gtk",
        "runtime_probe": probe,
        "config": {
            "default_mode": cfg.browser.default_mode,
            "default_width": cfg.browser.default_width,
            "default_height": cfg.browser.default_height,
            "memory_limit_mb": cfg.browser.memory_limit_mb,
            "idle_timeout_secs": cfg.browser.idle_timeout_secs,
        },
        "active_sessions": sessions,
        "browser_preflight": browser_preflight_value(&health),
        "managed_tasks": crate::tools::task_manager::list_tasks(),
        "recent_browser_errors": recent_errors,
    }))
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
