use crate::tools::Tool;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserBackendChoice {
    ObscuraHeadless,
    FirefoxHeadless,
    GsdChromeGui,
}

impl BrowserBackendChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ObscuraHeadless => "obscura_headless",
            Self::FirefoxHeadless => "firefox_headless",
            Self::GsdChromeGui => "gsd_chrome_gui",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserBrokerResult {
    pub backend: BrowserBackendChoice,
    pub status: String,
    pub output: String,
    pub cleanup: String,
    pub fallbacks_tried: Vec<BrowserBackendChoice>,
    pub errors: Vec<String>,
}

pub fn browser_backend_priority() -> [BrowserBackendChoice; 3] {
    [
        BrowserBackendChoice::ObscuraHeadless,
        BrowserBackendChoice::FirefoxHeadless,
        BrowserBackendChoice::GsdChromeGui,
    ]
}

pub async fn eval_with_browser_broker(
    url: &str,
    script: &str,
    timeout_secs: u64,
) -> Result<BrowserBrokerResult> {
    let mut errors = Vec::new();
    let mut tried = Vec::new();

    for backend in browser_backend_priority() {
        tried.push(backend);
        match eval_with_backend(backend, url, script, timeout_secs).await {
            Ok(output) => {
                register_browser_task(backend, url, cleanup_label(backend));
                cleanup_backend_after_use(backend).await;
                return Ok(BrowserBrokerResult {
                    backend,
                    status: "success".to_string(),
                    output,
                    cleanup: cleanup_label(backend).to_string(),
                    fallbacks_tried: tried,
                    errors,
                });
            }
            Err(err) => {
                errors.push(format!("{}: {err}", backend.as_str()));
                cleanup_backend_after_use(backend).await;
            }
        }
    }

    Err(anyhow!(
        "All browser backends failed: {}",
        errors.join("; ")
    ))
}

pub async fn render_with_browser_broker(
    url: &str,
    timeout_secs: u64,
) -> Result<BrowserBrokerResult> {
    eval_with_browser_broker(
        url,
        "document.documentElement ? document.documentElement.outerHTML : ''",
        timeout_secs,
    )
    .await
}

async fn eval_with_backend(
    backend: BrowserBackendChoice,
    url: &str,
    script: &str,
    timeout_secs: u64,
) -> Result<String> {
    match backend {
        BrowserBackendChoice::ObscuraHeadless => eval_obscura(url, script, timeout_secs).await,
        BrowserBackendChoice::FirefoxHeadless => eval_firefox(url, script).await,
        BrowserBackendChoice::GsdChromeGui => eval_gsd(url, script).await,
    }
}

fn normalize_browser_output(output: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(output.trim()) {
        Ok(serde_json::Value::String(inner)) => inner,
        _ => output.to_string(),
    }
}

async fn eval_obscura(url: &str, script: &str, timeout_secs: u64) -> Result<String> {
    let tool = crate::tools::obscura::ObscuraBrowserTool::new();
    let value = tool
        .call(&json!({
            "url": url,
            "action": "eval_js",
            "script": script,
            "timeout": timeout_secs,
        }))
        .await?;
    Ok(normalize_browser_output(
        value
            .get("output")
            .and_then(|value| value.as_str())
            .unwrap_or(""),
    ))
}

async fn eval_firefox(url: &str, script: &str) -> Result<String> {
    let tool = crate::tools::firefox::FirefoxBrowserTool::new();
    tool.call(&json!({
        "action": "navigate",
        "url": url,
    }))
    .await?;
    let firefox_script = format!("return ({script});");
    let value = tool
        .call(&json!({
            "action": "eval",
            "script": firefox_script,
        }))
        .await?;
    Ok(normalize_browser_output(
        value
            .get("output")
            .and_then(|value| value.as_str())
            .unwrap_or(""),
    ))
}

async fn eval_gsd(url: &str, script: &str) -> Result<String> {
    let tool = crate::tools::gsd_browser::GsdBrowserTool;
    tool.call(&json!({
        "action": "navigate",
        "url": url,
    }))
    .await?;
    let value = tool
        .call(&json!({
            "action": "eval",
            "script": script,
        }))
        .await?;
    Ok(normalize_browser_output(
        value
            .get("output")
            .and_then(|value| value.as_str())
            .unwrap_or(""),
    ))
}

fn cleanup_label(backend: BrowserBackendChoice) -> &'static str {
    match backend {
        BrowserBackendChoice::ObscuraHeadless => "closed_tab",
        BrowserBackendChoice::FirefoxHeadless => "closed_session",
        BrowserBackendChoice::GsdChromeGui => "stopped_daemon",
    }
}

async fn cleanup_backend_after_use(backend: BrowserBackendChoice) {
    match backend {
        BrowserBackendChoice::FirefoxHeadless => {
            let tool = crate::tools::firefox::FirefoxBrowserTool::new();
            let _ = tool.call(&json!({ "action": "close" })).await;
        }
        BrowserBackendChoice::GsdChromeGui => {
            crate::tools::gsd_browser::stop_gsd_browser_daemon().await;
        }
        BrowserBackendChoice::ObscuraHeadless => {}
    }
}

fn register_browser_task(backend: BrowserBackendChoice, url: &str, cleanup: &str) {
    let task = crate::tools::task_manager::ManagedTask::new(
        crate::tools::task_manager::TaskKind::Browser,
        crate::tools::task_manager::TaskOwner::OpenZ,
        format!("browser {} for {}", backend.as_str(), url),
        crate::tools::task_manager::CleanupPolicy::OnTurnEnd,
    )
    .with_session_id(format!("{}:{cleanup}", backend.as_str()))
    .with_ttl(60);
    crate::tools::task_manager::register_task(task);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_backend_priority_prefers_obscura_then_firefox_then_gsd() {
        assert_eq!(
            browser_backend_priority(),
            [
                BrowserBackendChoice::ObscuraHeadless,
                BrowserBackendChoice::FirefoxHeadless,
                BrowserBackendChoice::GsdChromeGui,
            ]
        );
    }

    #[test]
    fn gsd_fallback_cleanup_stops_daemon() {
        assert_eq!(
            cleanup_label(BrowserBackendChoice::GsdChromeGui),
            "stopped_daemon"
        );
    }

    #[test]
    fn broker_result_records_backend_and_cleanup() {
        let result = BrowserBrokerResult {
            backend: BrowserBackendChoice::ObscuraHeadless,
            status: "success".to_string(),
            output: "content".to_string(),
            cleanup: "closed_tab".to_string(),
            fallbacks_tried: vec![BrowserBackendChoice::ObscuraHeadless],
            errors: Vec::new(),
        };

        assert_eq!(result.backend, BrowserBackendChoice::ObscuraHeadless);
        assert_eq!(result.cleanup, "closed_tab");
    }
}
