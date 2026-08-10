use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::process::Command;

pub struct GsdBrowserTool;

fn gsd_browser_bin_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".cargo").join("bin").join("gsd-browser");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("gsd-browser")
}

fn is_gsd_browser_disconnected_error(text: &str) -> bool {
    let normalized = text.to_lowercase();
    [
        "receiver is gone",
        "send failed",
        "browser disconnected",
        "target closed",
        "connection closed",
        "websocket closed",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn gsd_browser_disconnected_error_value(error: &str, recovered: bool) -> Value {
    json!({
        "error": error.trim(),
        "error_kind": "browser_disconnected",
        "retryable": !recovered,
        "recovered": recovered,
        "next_step": if recovered {
            "The gsd-browser daemon was restarted and the action was retried once, but the retry still failed. Use inspect_browsers, or switch to firefox_browser/obscura_browser for this task."
        } else {
            "The gsd-browser daemon connection is stale. Restart it with `gsd-browser daemon stop` then `gsd-browser daemon start`, or use inspect_browsers."
        }
    })
}

pub async fn stop_gsd_browser_daemon() {
    let bin_path = gsd_browser_bin_path();
    let _ = Command::new(bin_path)
        .arg("daemon")
        .arg("stop")
        .output()
        .await;
}

async fn restart_gsd_browser_daemon(bin_path: &PathBuf) {
    let _ = Command::new(bin_path)
        .arg("daemon")
        .arg("stop")
        .output()
        .await;
    let _ = Command::new(bin_path)
        .arg("daemon")
        .arg("start")
        .output()
        .await;
}

fn build_gsd_browser_command(bin_path: &PathBuf, arguments: &Value) -> Result<Command> {
    let action = arguments
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

    let mut cmd = Command::new(bin_path);

    match action {
        "navigate" => {
            let url = arguments
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'url' parameter for navigate action"))?;
            cmd.arg("navigate").arg(url);
        }
        "snapshot" => {
            cmd.arg("snapshot");
        }
        "click" => {
            let ref_id = arguments
                .get("ref_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for click action"))?;
            cmd.arg("click-ref").arg(ref_id);
        }
        "hover" => {
            let ref_id = arguments
                .get("ref_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for hover action"))?;
            cmd.arg("hover-ref").arg(ref_id);
        }
        "fill" => {
            let ref_id = arguments
                .get("ref_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for fill action"))?;
            let text = arguments
                .get("text")
                .or_else(|| arguments.get("value"))
                .or_else(|| arguments.get("query"))
                .or_else(|| arguments.get("content"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    anyhow!(
                        "Missing fill text. Pass 'text' (preferred), or alias 'value', 'query', or 'content' for fill action"
                    )
                })?;
            cmd.arg("fill-ref").arg(ref_id).arg(text);
        }
        "screenshot" => {
            let path = arguments
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'path' parameter for screenshot action"))?;
            let resolved = crate::config::resolve_path(path);
            cmd.arg("screenshot").arg("--output").arg(resolved);
        }
        "eval" => {
            let script = arguments
                .get("script")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'script' parameter for eval action"))?;
            cmd.arg("eval").arg(script);
        }
        "accessibility_tree" => {
            cmd.arg("accessibility-tree");
        }
        "page_source" => {
            cmd.arg("page-source");
        }
        "save_pdf" => {
            let path = arguments
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'path' parameter for save_pdf action"))?;
            let resolved = crate::config::resolve_path(path);
            cmd.arg("save-pdf").arg("--output").arg(resolved);
        }
        _ => return Err(anyhow!("Unsupported browser action: {}", action)),
    }

    Ok(cmd)
}

#[async_trait::async_trait]
impl Tool for GsdBrowserTool {
    fn name(&self) -> &str {
        "gsd_browser"
    }

    fn description(&self) -> &str {
        "Last-resort GUI Chrome browser control for interactive pages. Prefer searchxyz_browser_search, obscura_browser, or firefox_browser for search/research because those use headless-first cleanup."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "navigate",
                        "snapshot",
                        "click",
                        "hover",
                        "fill",
                        "screenshot",
                        "eval",
                        "accessibility_tree",
                        "page_source",
                        "save_pdf"
                    ],
                    "description": "The browser action: 'navigate' to a URL, 'snapshot' to get interactive elements, 'click' or 'hover' on an element ref, 'fill' text input ref, 'screenshot' to capture image, 'eval' to run custom JavaScript, 'accessibility_tree' for roles/a11y tree, 'page_source' for HTML, 'save_pdf' to save as PDF."
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (required for 'navigate')."
                },
                "ref_id": {
                    "type": "string",
                    "description": "Element reference ID from snapshot, e.g. '@v1:e5' (required for 'click', 'hover', and 'fill')."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type into input element (required for 'fill')."
                },
                "value": {
                    "type": "string",
                    "description": "Alias for text; accepted for 'fill'."
                },
                "query": {
                    "type": "string",
                    "description": "Alias for text; accepted for search-box 'fill'."
                },
                "path": {
                    "type": "string",
                    "description": "Output file path (required for 'screenshot' and 'save_pdf')."
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript expression to evaluate (required for 'eval')."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let bin_path = gsd_browser_bin_path();
        let mut cmd = build_gsd_browser_command(&bin_path, arguments)?;

        let output = match cmd.output().await {
            Ok(output) => output,
            Err(e) => {
                let status = if e.kind() == std::io::ErrorKind::NotFound {
                    crate::tools::browser_status::BrowserBackendStatus::Missing
                } else {
                    crate::tools::browser_status::BrowserBackendStatus::Broken
                };
                let payload = crate::tools::browser_status::browser_preflight_error_value(
                    &format!("gsd-browser startup failed: {}", e),
                    crate::tools::browser_status::BrowserHealth {
                        chrome_cdp: crate::tools::browser_status::BrowserBackendStatus::Stopped,
                        gsd_browser: status,
                        geckodriver: crate::tools::browser_status::BrowserBackendStatus::Stopped,
                    },
                );
                return Err(anyhow!(payload.to_string()));
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            let error_text = if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            if is_gsd_browser_disconnected_error(&error_text) {
                restart_gsd_browser_daemon(&bin_path).await;
                let retry_output = build_gsd_browser_command(&bin_path, arguments)?
                    .output()
                    .await;
                if let Ok(retry_output) = retry_output {
                    let retry_stdout = String::from_utf8_lossy(&retry_output.stdout).to_string();
                    let retry_stderr = String::from_utf8_lossy(&retry_output.stderr).to_string();
                    if retry_output.status.success() {
                        return Ok(json!({
                            "status": "success",
                            "recovered": true,
                            "recovery_action": "restarted_gsd_browser_daemon",
                            "output": retry_stdout.trim()
                        }));
                    }
                    let retry_error_text = if retry_stderr.trim().is_empty() {
                        retry_stdout.trim().to_string()
                    } else {
                        retry_stderr.trim().to_string()
                    };
                    return Err(anyhow!(
                        "{}",
                        gsd_browser_disconnected_error_value(&retry_error_text, true)
                    ));
                }
                return Err(anyhow!(
                    "{}",
                    gsd_browser_disconnected_error_value(&error_text, false)
                ));
            }
            return Err(anyhow!("gsd-browser error: {}", error_text));
        }

        Ok(json!({
            "status": "success",
            "output": stdout.trim()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gsd_browser_struct() -> Result<()> {
        let tool = GsdBrowserTool;
        assert_eq!(tool.name(), "gsd_browser");
        Ok(())
    }

    #[test]
    fn gsd_browser_missing_error_payload_is_structured() {
        let payload = crate::tools::browser_status::browser_preflight_error_value(
            "gsd-browser startup failed: not found",
            crate::tools::browser_status::BrowserHealth {
                chrome_cdp: crate::tools::browser_status::BrowserBackendStatus::Stopped,
                gsd_browser: crate::tools::browser_status::BrowserBackendStatus::Missing,
                geckodriver: crate::tools::browser_status::BrowserBackendStatus::Stopped,
            },
        );
        assert_eq!(
            payload["browser_preflight"]["health"]["gsd_browser"],
            "missing"
        );
    }

    #[test]
    fn gsd_browser_schema_exposes_fill_aliases() {
        let tool = GsdBrowserTool;
        let params = tool.parameters();
        let props = params
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("properties object");
        assert!(props.contains_key("text"));
        assert!(props.contains_key("value"));
        assert!(props.contains_key("query"));
    }

    #[test]
    fn gsd_browser_description_marks_gui_as_last_resort() {
        let tool = GsdBrowserTool;
        let description = tool.description().to_lowercase();
        assert!(description.contains("last-resort"));
        assert!(description.contains("headless"));
    }

    #[test]
    fn gsd_browser_detects_stale_receiver_errors() {
        assert!(is_gsd_browser_disconnected_error(
            "snapshot error: send failed because receiver is gone"
        ));
        assert!(is_gsd_browser_disconnected_error("browser disconnected"));
        assert!(is_gsd_browser_disconnected_error("Target closed"));
        assert!(!is_gsd_browser_disconnected_error("selector not found"));
    }

    #[test]
    fn gsd_browser_disconnected_payload_is_machine_readable() {
        let payload = gsd_browser_disconnected_error_value("receiver is gone", true);
        assert_eq!(payload["error_kind"], "browser_disconnected");
        assert_eq!(payload["recovered"], true);
        assert_eq!(payload["retryable"], false);
        assert!(payload["next_step"]
            .as_str()
            .unwrap()
            .contains("inspect_browsers"));
    }
}
