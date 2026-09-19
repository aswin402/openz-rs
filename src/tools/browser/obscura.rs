use crate::tools::browser_common::{
    browser_cdp_port, connect_to_tab, ensure_browser_running, kill_browser_on_cdp_port,
    send_cdp_cmd,
};
use crate::tools::web::is_safe_ip;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub struct ObscuraBrowserTool;

impl Default for ObscuraBrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ObscuraBrowserTool {
    pub fn new() -> Self {
        ObscuraBrowserTool
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_in_tab(
        &self,
        _client: &reqwest::Client,
        ws_url: &str,
        _tab_id: &str,
        is_eval: bool,
        script_str: Option<&str>,
        navigate_url: &str,
        timeout_secs: u64,
    ) -> Result<String> {
        let (mut write, mut read) = connect_to_tab(ws_url).await?;
        let mut message_id = 0;

        send_cdp_cmd(
            &mut write,
            &mut read,
            &mut message_id,
            "Page.enable",
            json!({}),
        )
        .await?;
        send_cdp_cmd(
            &mut write,
            &mut read,
            &mut message_id,
            "Page.navigate",
            json!({ "url": navigate_url }),
        )
        .await?;

        let start_time = Instant::now();
        let max_duration = Duration::from_secs(timeout_secs);

        while start_time.elapsed() < max_duration {
            sleep(Duration::from_millis(300)).await;
            let eval_res = send_cdp_cmd(
                &mut write,
                &mut read,
                &mut message_id,
                "Runtime.evaluate",
                json!({
                    "expression": "document.readyState",
                    "returnByValue": true
                }),
            )
            .await?;

            if let Some(state) = eval_res
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"))
                .and_then(|v| v.as_str())
            {
                if state == "complete" {
                    break;
                }
            }
        }

        if is_eval {
            let script_expr = script_str
                .ok_or_else(|| anyhow!("Missing 'script' parameter for eval_js action"))?;
            let eval_res = send_cdp_cmd(
                &mut write,
                &mut read,
                &mut message_id,
                "Runtime.evaluate",
                json!({
                    "expression": script_expr,
                    "returnByValue": true
                }),
            )
            .await?;

            let val = eval_res
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"));
            match val {
                Some(v) => Ok(v.to_string()),
                None => {
                    if let Some(exception) = eval_res
                        .get("result")
                        .and_then(|r| r.get("exceptionDetails"))
                    {
                        return Err(anyhow!("JavaScript exception: {}", exception));
                    }
                    Ok("null".to_string())
                }
            }
        } else {
            let eval_res = send_cdp_cmd(
                &mut write,
                &mut read,
                &mut message_id,
                "Runtime.evaluate",
                json!({
                    "expression": "document.documentElement.outerHTML",
                    "returnByValue": true
                }),
            )
            .await?;

            let html_str = eval_res
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Failed to retrieve document.documentElement.outerHTML"))?;

            Ok(html2md::parse_html(html_str))
        }
    }
}

pub fn extract_obscura_url(arguments: &Value) -> Result<String> {
    let raw_url = if let Some(s) = arguments.as_str() {
        s.trim()
    } else {
        arguments
            .get("url")
            .or_else(|| arguments.get("target_url"))
            .or_else(|| arguments.get("targetUrl"))
            .or_else(|| arguments.get("uri"))
            .or_else(|| arguments.get("link"))
            .or_else(|| arguments.get("target"))
            .or_else(|| arguments.get("page"))
            .or_else(|| arguments.get("endpoint"))
            .or_else(|| arguments.get("href"))
            .or_else(|| arguments.get("address"))
            .or_else(|| arguments.get("site"))
            .or_else(|| arguments.get("website"))
            .or_else(|| arguments.get("domain"))
            .or_else(|| arguments.get("host"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'url' parameter"))?
            .trim()
    };

    if raw_url.is_empty() {
        return Err(anyhow!("Missing 'url' parameter (received empty string)"));
    }

    Ok(crate::tools::web::normalize_web_url(raw_url))
}

pub fn is_obscura_eval_action(action: &str) -> bool {
    let norm = action.trim().to_lowercase().replace('-', "_");
    matches!(
        norm.as_str(),
        "eval_js" | "eval" | "evaluate" | "js" | "script" | "expr"
    )
}

#[async_trait::async_trait]
impl Tool for ObscuraBrowserTool {
    fn name(&self) -> &str {
        "obscura_browser"
    }

    fn description(&self) -> &str {
        "Interact with a local headless browser (obscura or Chrome) using Chrome DevTools Protocol (CDP) to navigate, render JS-heavy pages as Markdown, or execute custom JavaScript."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to navigate the browser to (supports bare domains, href/target aliases, or direct string)."
                },
                "action": {
                    "type": "string",
                    "enum": ["render", "eval_js"],
                    "description": "The action to perform: 'render' (default, returns Markdown structure; aliases: view, markdown, read, content) or 'eval_js' (evaluates JavaScript; aliases: eval, evaluate, js, script)."
                },
                "script": {
                    "type": "string",
                    "description": "The JavaScript expression to evaluate (required when action is 'eval_js')."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Maximum page load timeout in seconds (default: 15)."
                }
            },
            "required": ["url"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let normalized_url = extract_obscura_url(arguments)?;
        let url_str = &normalized_url;

        validate_url(url_str).await?;

        let raw_action = arguments
            .get("action")
            .or_else(|| arguments.get("act"))
            .or_else(|| arguments.get("command"))
            .or_else(|| arguments.get("cmd"))
            .and_then(|v| v.as_str())
            .unwrap_or("render");
        let is_eval = is_obscura_eval_action(raw_action);

        let script_str = arguments
            .get("script")
            .or_else(|| arguments.get("expression"))
            .or_else(|| arguments.get("code"))
            .or_else(|| arguments.get("js"))
            .or_else(|| arguments.get("expr"))
            .and_then(|v| v.as_str());

        let timeout_secs = arguments
            .get("timeout")
            .or_else(|| arguments.get("timeout_secs"))
            .or_else(|| arguments.get("timeoutSecs"))
            .and_then(|v| {
                v.as_u64().or_else(|| {
                    v.as_str()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                })
            })
            .unwrap_or(15)
            .clamp(1, 300);

        // Ensure browser is running
        if let Err(e) = ensure_browser_running().await {
            let payload = crate::tools::browser_status::browser_preflight_error_value(
                &format!("Chrome CDP browser startup failed: {}", e),
                crate::tools::browser_status::BrowserHealth {
                    chrome_cdp: crate::tools::browser_status::BrowserBackendStatus::Broken,
                    gsd_browser: crate::tools::browser_status::BrowserBackendStatus::Stopped,
                    geckodriver: crate::tools::browser_status::BrowserBackendStatus::Stopped,
                },
            );
            return Err(anyhow!(payload.to_string()));
        }

        let cdp_port = browser_cdp_port();
        let client = crate::core::http::default_http_client();
        let new_tab_url = format!("http://127.0.0.1:{cdp_port}/json/new");

        // Open a new tab
        let mut res = client.put(&new_tab_url).send().await;

        if !matches!(&res, Ok(r) if r.status().is_success()) {
            res = client.get(&new_tab_url).send().await;
        }

        if !matches!(&res, Ok(r) if r.status().is_success()) {
            tracing::warn!("CDP HTTP API failed, attempting browser restart...");
            kill_browser_on_cdp_port();
            sleep(Duration::from_millis(500)).await;
            ensure_browser_running().await?;
            res = client.put(&new_tab_url).send().await;
            if !matches!(&res, Ok(r) if r.status().is_success()) {
                res = client.get(&new_tab_url).send().await;
            }
        }

        let res = res?;
        if !res.status().is_success() {
            return Err(anyhow!(
                "Failed to create a new tab via CDP HTTP API after restart"
            ));
        }

        let tab_info: Value = res.json().await?;
        let tab_id = tab_info
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("No tab ID returned from /json/new"))?;
        let ws_url = tab_info
            .get("webSocketDebuggerUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("No webSocketDebuggerUrl returned from /json/new"))?;

        let tab_id = tab_id.to_string();
        let result = self
            .execute_in_tab(
                &client,
                ws_url,
                &tab_id,
                is_eval,
                script_str,
                url_str,
                timeout_secs,
            )
            .await;

        // Always close the tab, even on error
        let close_url = format!("http://127.0.0.1:{cdp_port}/json/close/{}", tab_id);
        let _ = client.get(&close_url).send().await;

        let output = result?;
        Ok(json!({
            "status": "success",
            "output": output
        }))
    }
}

async fn validate_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("URL has no host"))?
        .to_lowercase();

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(anyhow::anyhow!(
            "SSRF blocked: only http/https URLs are allowed (got '{}')",
            parsed.scheme()
        ));
    }

    if host == "169.254.169.254" || host == "metadata.google.internal" {
        return Err(anyhow::anyhow!(
            "SSRF blocked: cloud metadata endpoints are not allowed"
        ));
    }

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if !is_safe_ip(&ip) {
            return Err(anyhow::anyhow!(
                "SSRF blocked: private/reserved IP addresses are not allowed"
            ));
        }
    }

    let resolved: Vec<_> = match tokio::net::lookup_host(format!("{}:0", host)).await {
        Ok(iter) => iter.map(|addr| addr.ip()).collect(),
        Err(_) => Vec::new(),
    };

    for ip in &resolved {
        if !is_safe_ip(ip) {
            return Err(anyhow::anyhow!(
                "SSRF blocked: hostname '{}' resolved to private/reserved IP {}",
                host,
                ip
            ));
        }
    }

    if resolved.is_empty() {
        return Err(anyhow::anyhow!(
            "SSRF blocked: hostname '{}' could not be resolved",
            host
        ));
    }

    Ok(())
}

#[cfg(test)]
#[path = "obscura_tests.rs"]
mod tests;

