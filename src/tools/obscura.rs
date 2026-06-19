use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use futures_util::stream::{SplitSink, SplitStream};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type WsStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub struct ObscuraBrowserTool;

impl ObscuraBrowserTool {
    pub fn new() -> Self {
        ObscuraBrowserTool
    }
}

fn kill_browser_on_port_9222() {
    #[cfg(unix)]
    {
        let _ = Command::new("sh")
            .arg("-c")
            .arg("fuser -k 9222/tcp || kill $(lsof -t -i:9222)")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("cmd")
            .arg("/C")
            .arg("for /f \"tokens=5\" %a in ('netstat -aon ^| findstr 9222') do taskkill /F /PID %a")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

async fn ensure_browser_running() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()?;
    
    // Check if port 9222 is already listening
    if client.get("http://127.0.0.1:9222/json/list").send().await.is_ok() {
        return Ok(());
    }

    // Attempt to start obscura first
    let child = Command::new("obscura")
        .args(&["serve", "--port", "9222"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    if child.is_ok() {
        // Wait up to 3 seconds for it to start
        for _ in 0..15 {
            sleep(Duration::from_millis(200)).await;
            if client.get("http://127.0.0.1:9222/json/list").send().await.is_ok() {
                return Ok(());
            }
        }
    }

    // Obscura failed or not in path, try falling back to chrome/chromium
    let chrome_paths = ["google-chrome", "chrome", "chromium", "chromium-browser"];
    for path in chrome_paths {
        let child = Command::new(path)
            .args(&[
                "--headless",
                "--remote-debugging-port=9222",
                "--disable-gpu",
                "--no-sandbox",
                "--disable-dev-shm-usage",
                "--allow-file-access-from-files",
                "--disable-web-security",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        if child.is_ok() {
            // Wait up to 5 seconds for it to start
            for _ in 0..25 {
                sleep(Duration::from_millis(200)).await;
                if client.get("http://127.0.0.1:9222/json/list").send().await.is_ok() {
                    return Ok(());
                }
            }
            break;
        }
    }

    if client.get("http://127.0.0.1:9222/json/list").send().await.is_ok() {
        Ok(())
    } else {
        Err(anyhow!("Failed to start any headless browser (obscura, google-chrome, chromium) on port 9222"))
    }
}

async fn send_cdp_cmd(
    write: &mut WsSink,
    read: &mut WsStream,
    message_id: &mut u64,
    method: &str,
    params: Value,
) -> Result<Value> {
    *message_id += 1;
    let id = *message_id;
    let req = json!({
        "id": id,
        "method": method,
        "params": params
    });
    
    write.send(Message::Text(req.to_string())).await?;
    
    while let Some(msg) = read.next().await {
        let msg = msg?;
        if let Message::Text(text) = msg {
            if let Ok(resp) = serde_json::from_str::<Value>(&text) {
                if resp.get("id").and_then(|v| v.as_u64()) == Some(id) {
                    return Ok(resp);
                }
            }
        }
    }
    Err(anyhow!("Connection closed before receiving response for ID {}", id))
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

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        let mut message_id = 0;

        // Enable Page domain
        send_cdp_cmd(&mut write, &mut read, &mut message_id, "Page.enable", json!({})).await?;

        // Navigate to URL
        send_cdp_cmd(&mut write, &mut read, &mut message_id, "Page.navigate", json!({ "url": url_str })).await?;

        // Poll document.readyState until complete or timeout
        let start_time = Instant::now();
        let max_duration = Duration::from_secs(timeout_secs);
        let mut is_complete = false;

        while start_time.elapsed() < max_duration {
            sleep(Duration::from_millis(300)).await;
            
            let eval_res = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Runtime.evaluate", json!({
                "expression": "document.readyState",
                "returnByValue": true
            })).await?;

            if let Some(state) = eval_res
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"))
                .and_then(|v| v.as_str()) 
            {
                if state == "complete" {
                    is_complete = true;
                    break;
                }
            }
        }

        // Run the action
        let output = if action == "eval_js" {
            let script_expr = script_str.ok_or_else(|| anyhow!("Missing 'script' parameter for eval_js action"))?;
            let eval_res = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Runtime.evaluate", json!({
                "expression": script_expr,
                "returnByValue": true
            })).await?;

            let val = eval_res
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"));
            
            match val {
                Some(v) => v.to_string(),
                None => {
                    if let Some(exception) = eval_res.get("result").and_then(|r| r.get("exceptionDetails")) {
                        return Err(anyhow!("JavaScript exception: {}", exception));
                    }
                    "null".to_string()
                }
            }
        } else {
            let eval_res = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Runtime.evaluate", json!({
                "expression": "document.documentElement.outerHTML",
                "returnByValue": true
            })).await?;

            let html_str = eval_res
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Failed to retrieve document.documentElement.outerHTML"))?;

            // Convert to Markdown
            html2md::parse_html(html_str)
        };

        // Close the tab
        let close_url = format!("http://127.0.0.1:9222/json/close/{}", tab_id);
        let _ = client.get(&close_url).send().await;

        Ok(json!({
            "status": "success",
            "complete": is_complete,
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
