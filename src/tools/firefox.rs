use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use thirtyfour::prelude::*;
use std::sync::OnceLock;
use tokio::sync::Mutex;

static DRIVER: OnceLock<Mutex<Option<WebDriver>>> = OnceLock::new();

fn get_driver_mutex() -> &'static Mutex<Option<WebDriver>> {
    DRIVER.get_or_init(|| Mutex::new(None))
}

async fn ensure_geckodriver_running() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()?;
    
    if client.get("http://127.0.0.1:4444/status").send().await.is_ok() {
        return Ok(());
    }

    let child = Command::new("geckodriver")
        .args(&["--port", "4444"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    if child.is_ok() {
        for _ in 0..15 {
            sleep(Duration::from_millis(200)).await;
            if client.get("http://127.0.0.1:4444/status").send().await.is_ok() {
                return Ok(());
            }
        }
    }

    Err(anyhow!("Failed to start geckodriver on port 4444. Please ensure geckodriver is installed and in your PATH."))
}

async fn get_or_create_driver() -> Result<WebDriver> {
    let mutex = get_driver_mutex();
    let mut guard = mutex.lock().await;
    
    if let Some(ref driver) = *guard {
        if driver.title().await.is_ok() {
            return Ok(driver.clone());
        }
    }
    
    ensure_geckodriver_running().await?;
    
    let mut caps = DesiredCapabilities::firefox();
    caps.add_arg("--headless")?;
    
    let driver = WebDriver::new("http://localhost:4444", caps).await?;
    *guard = Some(driver.clone());
    
    Ok(driver)
}

async fn reset_driver() {
    let mutex = get_driver_mutex();
    let mut guard = mutex.lock().await;
    if let Some(driver) = guard.take() {
        let _ = driver.quit().await;
    }
}

pub struct FirefoxBrowserTool;

impl FirefoxBrowserTool {
    pub fn new() -> Self {
        FirefoxBrowserTool
    }
}

#[async_trait::async_trait]
impl Tool for FirefoxBrowserTool {
    fn name(&self) -> &str {
        "firefox_browser"
    }

    fn description(&self) -> &str {
        "Control a headless Firefox browser instance using WebDriver (thirtyfour) to navigate, interact with elements, evaluate JS, take screenshots, or render page as Markdown."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["navigate", "click", "fill", "screenshot", "eval", "render", "close"],
                    "description": "The browser action: 'navigate' to a URL, 'click' on a CSS selector, 'fill' text into an input selector, 'screenshot' to capture image, 'eval' to run custom JavaScript, 'render' to get page Markdown, or 'close' to close the browser session."
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (required for 'navigate', optional for 'render')."
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for the element (required for 'click' and 'fill')."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type into input element (required for 'fill')."
                },
                "path": {
                    "type": "string",
                    "description": "Output file path for screenshot (required for 'screenshot')."
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
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        if action == "close" {
            reset_driver().await;
            return Ok(json!({
                "status": "success",
                "message": "Successfully closed the Firefox browser instance"
            }));
        }

        let driver = match get_or_create_driver().await {
            Ok(d) => d,
            Err(e) => {
                reset_driver().await;
                return Err(anyhow!("Failed to initialize Firefox WebDriver: {:?}", e));
            }
        };

        let result = match action {
            "navigate" => {
                let url = arguments.get("url").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'url' parameter for navigate action"))?;
                if let Err(e) = driver.goto(url).await {
                    reset_driver().await;
                    return Err(anyhow!("Navigation failed: {:?}", e));
                }
                json!({
                    "status": "success",
                    "message": format!("Successfully navigated to {}", url)
                })
            }
            "click" => {
                let selector = arguments.get("selector").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'selector' parameter for click action"))?;
                let elem = driver.find(By::Css(selector)).await?;
                elem.click().await?;
                json!({
                    "status": "success",
                    "message": format!("Successfully clicked element '{}'", selector)
                })
            }
            "fill" => {
                let selector = arguments.get("selector").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'selector' parameter for fill action"))?;
                let text = arguments.get("text").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'text' parameter for fill action"))?;
                let elem = driver.find(By::Css(selector)).await?;
                elem.clear().await?;
                elem.send_keys(text).await?;
                json!({
                    "status": "success",
                    "message": format!("Successfully typed text into element '{}'", selector)
                })
            }
            "screenshot" => {
                let path = arguments.get("path").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'path' parameter for screenshot action"))?;
                driver.screenshot(std::path::Path::new(path)).await?;
                json!({
                    "status": "success",
                    "message": format!("Screenshot saved to {}", path)
                })
            }
            "eval" => {
                let script = arguments.get("script").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'script' parameter for eval action"))?;
                let val = driver.execute(script, vec![]).await?;
                json!({
                    "status": "success",
                    "output": val.json().to_string()
                })
            }
            "render" => {
                if let Some(url) = arguments.get("url").and_then(|v| v.as_str()) {
                    if let Err(e) = driver.goto(url).await {
                        reset_driver().await;
                        return Err(anyhow!("Navigation failed: {:?}", e));
                    }
                }
                let html = driver.source().await?;
                let md = html2md::parse_html(&html);
                json!({
                    "status": "success",
                    "output": md
                })
            }
            _ => return Err(anyhow!("Unknown action: {}", action))
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_firefox_browser_tool_metadata() -> Result<()> {
        let tool = FirefoxBrowserTool::new();
        assert_eq!(tool.name(), "firefox_browser");
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
        Ok(())
    }
}
