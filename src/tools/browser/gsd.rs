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

fn resolve_gsd_browser_action_and_url(arguments: &Value) -> (String, Option<String>) {
    if let Some(s) = arguments.as_str() {
        return ("navigate".to_string(), Some(s.trim().to_string()));
    }

    let url_opt = arguments
        .get("url")
        .or_else(|| arguments.get("target_url"))
        .or_else(|| arguments.get("targetUrl"))
        .or_else(|| arguments.get("uri"))
        .or_else(|| arguments.get("link"))
        .or_else(|| arguments.get("target"))
        .or_else(|| arguments.get("site"))
        .or_else(|| arguments.get("website"))
        .or_else(|| arguments.get("domain"))
        .or_else(|| arguments.get("host"))
        .or_else(|| arguments.get("page"))
        .or_else(|| arguments.get("endpoint"))
        .or_else(|| arguments.get("href"))
        .or_else(|| arguments.get("address"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let explicit_action = arguments
        .get("action")
        .or_else(|| arguments.get("act"))
        .or_else(|| arguments.get("command"))
        .or_else(|| arguments.get("cmd"))
        .or_else(|| arguments.get("method"))
        .and_then(|v| v.as_str())
        .map(str::trim);

    let action = match explicit_action {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => {
            if url_opt.is_some() {
                "navigate".to_string()
            } else {
                "snapshot".to_string()
            }
        }
    };

    (action, url_opt)
}

fn build_gsd_browser_command(bin_path: &PathBuf, arguments: &Value) -> Result<Command> {
    let (raw_action, url_arg) = resolve_gsd_browser_action_and_url(arguments);
    let action_normalized = raw_action.trim().to_lowercase().replace('-', "_");

    let mut cmd = Command::new(bin_path);

    match action_normalized.as_str() {
        "navigate" | "goto" | "open" | "url" | "visit" => {
            let raw_url = url_arg
                .ok_or_else(|| anyhow!("Missing 'url' parameter for navigate action"))?;
            let normalized_url = crate::tools::web::normalize_web_url(&raw_url);
            cmd.arg("navigate").arg(normalized_url);
        }
        "snapshot" | "interactive" | "elements" | "dom" => {
            cmd.arg("snapshot");
        }
        "click" | "click_ref" | "press" => {
            let ref_id = arguments
                .get("ref_id")
                .or_else(|| arguments.get("refId"))
                .or_else(|| arguments.get("ref"))
                .or_else(|| arguments.get("element"))
                .or_else(|| arguments.get("id"))
                .or_else(|| arguments.get("target_ref"))
                .or_else(|| arguments.get("targetRef"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for click action"))?
                .trim();
            cmd.arg("click-ref").arg(ref_id);
        }
        "hover" | "hover_ref" => {
            let ref_id = arguments
                .get("ref_id")
                .or_else(|| arguments.get("refId"))
                .or_else(|| arguments.get("ref"))
                .or_else(|| arguments.get("element"))
                .or_else(|| arguments.get("id"))
                .or_else(|| arguments.get("target_ref"))
                .or_else(|| arguments.get("targetRef"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for hover action"))?
                .trim();
            cmd.arg("hover-ref").arg(ref_id);
        }
        "fill" | "fill_ref" | "type" | "input" => {
            let ref_id = arguments
                .get("ref_id")
                .or_else(|| arguments.get("refId"))
                .or_else(|| arguments.get("ref"))
                .or_else(|| arguments.get("element"))
                .or_else(|| arguments.get("id"))
                .or_else(|| arguments.get("target_ref"))
                .or_else(|| arguments.get("targetRef"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'ref_id' parameter for fill action"))?
                .trim();
            let text = arguments
                .get("text")
                .or_else(|| arguments.get("value"))
                .or_else(|| arguments.get("query"))
                .or_else(|| arguments.get("content"))
                .or_else(|| arguments.get("input"))
                .or_else(|| arguments.get("string"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    anyhow!(
                        "Missing fill text. Pass 'text' (preferred), or alias 'value', 'query', or 'content' for fill action"
                    )
                })?;
            cmd.arg("fill-ref").arg(ref_id).arg(text);
        }
        "screenshot" | "capture" => {
            let path = arguments
                .get("path")
                .or_else(|| arguments.get("output"))
                .or_else(|| arguments.get("output_path"))
                .or_else(|| arguments.get("outputPath"))
                .or_else(|| arguments.get("file_path"))
                .or_else(|| arguments.get("file"))
                .or_else(|| arguments.get("dest"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'path' parameter for screenshot action"))?
                .trim();
            let resolved = crate::config::resolve_path(path);
            cmd.arg("screenshot").arg("--output").arg(resolved);
        }
        "eval" | "eval_js" | "evaluate" | "js" => {
            let script = arguments
                .get("script")
                .or_else(|| arguments.get("expression"))
                .or_else(|| arguments.get("code"))
                .or_else(|| arguments.get("js"))
                .or_else(|| arguments.get("expr"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'script' parameter for eval action"))?;
            cmd.arg("eval").arg(script);
        }
        "accessibility_tree" | "a11y" | "a11y_tree" => {
            cmd.arg("accessibility-tree");
        }
        "page_source" | "source" | "html" => {
            cmd.arg("page-source");
        }
        "save_pdf" | "pdf" => {
            let path = arguments
                .get("path")
                .or_else(|| arguments.get("output"))
                .or_else(|| arguments.get("output_path"))
                .or_else(|| arguments.get("outputPath"))
                .or_else(|| arguments.get("file_path"))
                .or_else(|| arguments.get("file"))
                .or_else(|| arguments.get("dest"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing 'path' parameter for save_pdf action"))?
                .trim();
            let resolved = crate::config::resolve_path(path);
            cmd.arg("save-pdf").arg("--output").arg(resolved);
        }
        _ => return Err(anyhow!("Unsupported browser action: {}", raw_action)),
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
                    "description": "The browser action (automatically inferred if omitted: defaults to 'navigate' when url is present, else 'snapshot'): 'navigate' (aliases: goto, open, visit), 'snapshot' (aliases: dom, elements), 'click' (alias: press), 'hover', 'fill' (aliases: type, input), 'screenshot' (alias: capture), 'eval', 'accessibility_tree' (alias: a11y), 'page_source' (alias: html, source), 'save_pdf' (alias: pdf)."
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (supports bare domains, href/target aliases, or direct string; automatically selects 'navigate' action)."
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
            }
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
#[path = "gsd_tests.rs"]
mod tests;

