use crate::tools::Tool;
use crate::tools::browser_common::{connect_to_tab, ensure_browser_running, kill_browser_on_port_9222, send_cdp_cmd};
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
        action: &str,
        script_str: Option<&str>,
        navigate_url: &str,
        timeout_secs: u64,
    ) -> Result<String> {
        let (mut write, mut read) = connect_to_tab(ws_url).await?;
        let mut message_id = 0;

        send_cdp_cmd(&mut write, &mut read, &mut message_id, "Page.enable", json!({})).await?;
        send_cdp_cmd(&mut write, &mut read, &mut message_id, "Page.navigate", json!({ "url": navigate_url })).await?;

        let start_time = Instant::now();
        let max_duration = Duration::from_secs(timeout_secs);

        while start_time.elapsed() < max_duration {
            sleep(Duration::from_millis(300)).await;
            let eval_res = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Runtime.evaluate", json!({
                "expression": "document.readyState",
                "returnByValue": true
            })).await?;

            if let Some(state) = eval_res
                .get("result").and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"))
                .and_then(|v| v.as_str())
            {
                if state == "complete" { break; }
            }
        }

        if action == "eval_js" {
            let script_expr = script_str.ok_or_else(|| anyhow!("Missing 'script' parameter for eval_js action"))?;
            let eval_res = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Runtime.evaluate", json!({
                "expression": script_expr,
                "returnByValue": true
            })).await?;

            let val = eval_res.get("result").and_then(|r| r.get("result")).and_then(|res| res.get("value"));
            match val {
                Some(v) => Ok(v.to_string()),
                None => {
                    if let Some(exception) = eval_res.get("result").and_then(|r| r.get("exceptionDetails")) {
                        return Err(anyhow!("JavaScript exception: {}", exception));
                    }
                    Ok("null".to_string())
                }
            }
        } else {
            let eval_res = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Runtime.evaluate", json!({
                "expression": "document.documentElement.outerHTML",
                "returnByValue": true
            })).await?;

            let html_str = eval_res
                .get("result").and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Failed to retrieve document.documentElement.outerHTML"))?;

            Ok(html2md::parse_html(html_str))
        }
    }
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
                    "description": "The URL to navigate the browser to."
                },
                "action": {
                    "type": "string",
                    "enum": ["render", "eval_js"],
                    "description": "The action to perform: 'render' (default, returns Markdown structure of the page) or 'eval_js' (evaluates custom JavaScript expression)."
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
        let url_str = arguments.get("url").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'url' parameter"))?;
        
        let action = arguments.get("action").and_then(|v| v.as_str()).unwrap_or("render");
        let script_str = arguments.get("script").and_then(|v| v.as_str());
        let timeout_secs = arguments.get("timeout").and_then(|v| v.as_u64()).unwrap_or(15);

        // Ensure browser is running
        ensure_browser_running().await?;

        let client = reqwest::Client::new();
        
        // Open a new tab
        let mut res = client.put("http://127.0.0.1:9222/json/new")
            .send()
            .await;
        
        if res.is_err() || !res.as_ref().unwrap().status().is_success() {
            res = client.get("http://127.0.0.1:9222/json/new")
                .send()
                .await;
        }
        
        if res.is_err() || !res.as_ref().unwrap().status().is_success() {
            tracing::warn!("CDP HTTP API failed, attempting browser restart...");
            kill_browser_on_port_9222();
            sleep(Duration::from_millis(500)).await;
            ensure_browser_running().await?;
            res = client.put("http://127.0.0.1:9222/json/new")
                .send()
                .await;
            if res.is_err() || !res.as_ref().unwrap().status().is_success() {
                res = client.get("http://127.0.0.1:9222/json/new")
                    .send()
                    .await;
            }
        }

        let res = res?;
        if !res.status().is_success() {
            return Err(anyhow!("Failed to create a new tab via CDP HTTP API after restart"));
        }

        let tab_info: Value = res.json().await?;
        let tab_id = tab_info.get("id").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("No tab ID returned from /json/new"))?;
        let ws_url = tab_info.get("webSocketDebuggerUrl").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("No webSocketDebuggerUrl returned from /json/new"))?;

        let tab_id = tab_id.to_string();
        let result = self.execute_in_tab(&client, ws_url, &tab_id, action, script_str, url_str, timeout_secs).await;

        // Always close the tab, even on error
        let close_url = format!("http://127.0.0.1:9222/json/close/{}", tab_id);
        let _ = client.get(&close_url).send().await;

        let output = result?;
        Ok(json!({
            "status": "success",
            "output": output
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_obscura_browser_tool_metadata() -> Result<()> {
        let tool = ObscuraBrowserTool::new();
        assert_eq!(tool.name(), "obscura_browser");
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
        Ok(())
    }
}
