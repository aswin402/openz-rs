//! Headless Browser Broker backed by OpenZ's native low-RAM WebKit2GTK engine.
//!
//! Spawns an ephemeral offscreen WebKit2GTK worker (`GtkOffscreenWindow`), validates
//! target URLs against SSRF, evaluates JavaScript or extracts rendered HTML/Markdown,
//! and tears down the worker immediately after completion so idle RAM returns to 0 MB.

use super::client::BrowserSessionClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserBackendChoice {
    OpenzNativeHeadless,
}

impl BrowserBackendChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenzNativeHeadless => "openz_native_headless",
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

pub fn browser_backend_priority() -> [BrowserBackendChoice; 1] {
    [BrowserBackendChoice::OpenzNativeHeadless]
}

pub async fn eval_with_browser_broker(
    url: &str,
    script: &str,
    timeout_secs: u64,
) -> Result<BrowserBrokerResult> {
    crate::tools::web::validate_url(url).await?;

    let clamped_secs = timeout_secs.clamp(3, 45);
    let backend = BrowserBackendChoice::OpenzNativeHeadless;
    let client = BrowserSessionClient::spawn_ephemeral_headless(1280, 800).await?;
    let nav_timeout_ms = (clamped_secs * 1000).min(25_000);

    let nav_res = client.navigate_url(url, nav_timeout_ms, 220).await;
    match nav_res {
        Ok(_) => {}
        Err(err) => {
            let _ = client.close().await;
            return Err(err);
        }
    }

    let eval_res = client
        .eval_js(script, Duration::from_secs(clamped_secs))
        .await;
    let _ = client.close().await;

    let data = eval_res?;
    let raw_output = data
        .get("output")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let output = normalize_browser_output(&raw_output);
    register_browser_task(backend, url, cleanup_label(backend));

    Ok(BrowserBrokerResult {
        backend,
        status: "success".to_string(),
        output,
        cleanup: cleanup_label(backend).to_string(),
        fallbacks_tried: vec![backend],
        errors: Vec::new(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedBrowserPage {
    pub html: String,
    pub final_url: String,
    pub broker: BrowserBrokerResult,
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

pub async fn render_page_with_browser_broker(
    url: &str,
    timeout_secs: u64,
) -> Result<RenderedBrowserPage> {
    let broker = eval_with_browser_broker(
        url,
        "JSON.stringify({ html: document.documentElement ? document.documentElement.outerHTML : '', final_url: String(location.href || '') })",
        timeout_secs,
    )
    .await?;

    let (html, final_url) = match serde_json::from_str::<serde_json::Value>(broker.output.trim()) {
        Ok(val) => {
            let parsed = match val {
                serde_json::Value::String(inner) => {
                    serde_json::from_str::<serde_json::Value>(&inner).ok()
                }
                other => Some(other),
            };
            match parsed {
                Some(obj) => {
                    let html = obj
                        .get("html")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let final_url = obj
                        .get("final_url")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or(url)
                        .to_string();
                    (html, final_url)
                }
                None => (broker.output.clone(), url.to_string()),
            }
        }
        Err(_) => (broker.output.clone(), url.to_string()),
    };

    Ok(RenderedBrowserPage {
        html,
        final_url,
        broker,
    })
}

fn normalize_browser_output(output: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(output.trim()) {
        Ok(serde_json::Value::String(inner)) => inner,
        _ => output.to_string(),
    }
}

fn cleanup_label(backend: BrowserBackendChoice) -> &'static str {
    match backend {
        BrowserBackendChoice::OpenzNativeHeadless => "closed_ephemeral_worker",
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
#[path = "broker_tests.rs"]
mod tests;
