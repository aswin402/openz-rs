use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;
use thirtyfour::prelude::*;
use tokio::sync::Mutex;
use tokio::time::sleep;

struct FirefoxSession {
    driver: WebDriver,
    mode: String,
}

static DRIVER: OnceLock<Mutex<Option<FirefoxSession>>> = OnceLock::new();

fn get_driver_mutex() -> &'static Mutex<Option<FirefoxSession>> {
    DRIVER.get_or_init(|| Mutex::new(None))
}

fn geckodriver_missing_message() -> String {
    format!(
        "Browser preflight failed: geckodriver is missing or not runnable on ports {}/{}. Run inspect_browsers, install geckodriver, or use Chrome CDP/obscura browser fallback instead.",
        webdriver_port("headless"),
        webdriver_port("attach")
    )
}

fn geckodriver_missing_preflight_error() -> serde_json::Value {
    crate::tools::browser_status::browser_preflight_error_value(
        &geckodriver_missing_message(),
        crate::tools::browser_status::BrowserHealth {
            chrome_cdp: crate::tools::browser_status::BrowserBackendStatus::Stopped,
            gsd_browser: crate::tools::browser_status::BrowserBackendStatus::Stopped,
            geckodriver: crate::tools::browser_status::BrowserBackendStatus::Missing,
        },
    )
}

async fn ensure_geckodriver_running(port: u16, connect_existing: bool) -> Result<()> {
    let client = crate::core::http::custom_http_client(
        Duration::from_secs(2),
        Duration::from_secs(3),
    );

    if client
        .get(format!("http://127.0.0.1:{}/status", port))
        .send()
        .await
        .is_ok()
    {
        return Ok(());
    }

    if let Ok(child_handle) = Command::new("geckodriver")
        .args({
            let mut args = vec!["--port".to_string(), port.to_string()];
            if connect_existing {
                args.push("--connect-existing".to_string());
            }
            args
        })
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        crate::shutdown::register_child_with_metadata(
            child_handle,
            if connect_existing {
                "geckodriver --connect-existing"
            } else {
                "geckodriver --port 4444"
            },
            "geckodriver",
        );
        for _ in 0..15 {
            sleep(Duration::from_millis(200)).await;
            if client
                .get(format!("http://127.0.0.1:{}/status", port))
                .send()
                .await
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    Err(anyhow!(geckodriver_missing_preflight_error().to_string()))
}

fn requested_mode(arguments: &Value) -> Result<&str> {
    let mode = arguments
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("headless");
    if !matches!(mode, "headless" | "visible" | "attach") {
        return Err(anyhow!(
            "Unsupported Firefox mode {}; use headless, visible, or attach",
            mode
        ));
    }
    Ok(mode)
}

fn webdriver_port(mode: &str) -> u16 {
    let config = crate::config::loader::load_config().ok();
    let (variable, configured) = if mode == "attach" {
        (
            "OPENZ_FIREFOX_ATTACH_PORT",
            config
                .map(|value| value.browser.firefox_attach_port)
                .unwrap_or(4445),
        )
    } else {
        (
            "OPENZ_FIREFOX_WEBDRIVER_PORT",
            config
                .map(|value| value.browser.firefox_webdriver_port)
                .unwrap_or(4444),
        )
    };
    std::env::var(variable)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(configured)
}

async fn get_or_create_driver(mode: &str) -> Result<WebDriver> {
    let mutex = get_driver_mutex();
    let mut guard = mutex.lock().await;

    if let Some(session) = guard.as_ref() {
        if session.mode == mode && session.driver.title().await.is_ok() {
            return Ok(session.driver.clone());
        }
    }
    if let Some(session) = guard.take() {
        let _ = session.driver.quit().await;
    }

    let connect_existing = mode == "attach";
    let port = webdriver_port(mode);
    ensure_geckodriver_running(port, connect_existing).await?;

    let mut caps = DesiredCapabilities::firefox();
    if mode == "headless" {
        caps.add_arg("--headless")?;
    }

    let driver = WebDriver::new(&format!("http://localhost:{}", port), caps).await?;
    *guard = Some(FirefoxSession {
        driver: driver.clone(),
        mode: mode.to_string(),
    });

    Ok(driver)
}

async fn reset_driver() {
    let mutex = get_driver_mutex();
    let mut guard = mutex.lock().await;
    if let Some(session) = guard.take() {
        let _ = session.driver.quit().await;
    }
}

async fn wait_for_element(
    driver: &WebDriver,
    selector: &str,
    timeout_secs: u64,
) -> Result<WebElement> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs.clamp(1, 60));
    loop {
        match driver.find(By::Css(selector)).await {
            Ok(element) => return Ok(element),
            Err(error) if tokio::time::Instant::now() < deadline => {
                let _ = error;
                sleep(Duration::from_millis(250)).await;
            }
            Err(error) => {
                return Err(anyhow!(
                    "Element {} was not found within {} seconds: {}",
                    selector,
                    timeout_secs,
                    error
                ))
            }
        }
    }
}

pub struct FirefoxBrowserTool;

impl Default for FirefoxBrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

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
        "Control Firefox through WebDriver (thirtyfour). Use mode=headless for background automation or mode=visible to launch a user-visible Firefox window."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["navigate", "click", "fill", "wait_for", "media_status", "screenshot", "eval", "render", "close"],
                    "description": "The browser action. Use wait_for before interacting with dynamic elements; mode=visible launches a user-visible Firefox window."
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (required for 'navigate', optional for 'render')."
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for the element (required for 'click' and 'fill')."
                },
                "timeout_secs": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 60,
                    "description": "Maximum seconds to wait for a selector. Defaults to 10."
                },
                "media_selector": {
                    "type": "string",
                    "description": "Optional CSS selector for a video or audio element; defaults to the first media element."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type into input element (required for 'fill')."
                },
                "path": {
                    "type": "string",
                    "description": "Output file path for screenshot (required for 'screenshot')."
                },
                "open_after": {
                    "type": "boolean",
                    "description": "Open the saved screenshot with the system image viewer after capture."
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript expression to evaluate (required for 'eval')."
                },
                "mode": {
                    "type": "string",
                    "enum": ["headless", "visible", "attach"],
                    "description": "Browser session mode. Defaults to headless; attach connects to an existing Firefox started with Marionette support."
                },
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;
        let mode = requested_mode(arguments)?;
        let timeout_secs = arguments
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);

        if action == "close" {
            reset_driver().await;
            return Ok(json!({
                "status": "success",
                "message": "Successfully closed the Firefox browser instance"
            }));
        }

        let driver = match get_or_create_driver(mode).await {
            Ok(d) => d,
            Err(e) => {
                reset_driver().await;
                return Err(anyhow!("Failed to initialize Firefox WebDriver: {:?}", e));
            }
        };

        let result = match action {
            "navigate" => {
                let url = arguments
                    .get("url")
                    .and_then(|v| v.as_str())
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
            "wait_for" => {
                let selector = arguments
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing selector for wait_for"))?;
                let _ = wait_for_element(&driver, selector, timeout_secs).await?;
                json!({ "status": "success", "selector": selector, "message": format!("Element {} is ready", selector) })
            }
            "media_status" => {
                let selector = arguments
                    .get("media_selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("video, audio");
                let script = format!("(() => {{ const media = document.querySelector({:?}); if (!media) return {{found:false, selector:{:?}}}; return {{found:true, selector:{:?}, paused:media.paused, currentTime:media.currentTime, duration:media.duration, readyState:media.readyState, ended:media.ended, src:media.currentSrc || media.src || null}}; }})()", selector, selector, selector);
                let value = driver.execute(&script, vec![]).await?;
                json!({ "status": "success", "media": value.json() })
            }
            "click" => {
                let selector = arguments
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'selector' parameter for click action"))?;
                let elem = wait_for_element(&driver, selector, timeout_secs).await?;
                elem.click().await?;
                json!({
                    "status": "success",
                    "message": format!("Successfully clicked element '{}'", selector)
                })
            }
            "fill" => {
                let selector = arguments
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'selector' parameter for fill action"))?;
                let text = arguments
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'text' parameter for fill action"))?;
                let elem = wait_for_element(&driver, selector, timeout_secs).await?;
                elem.clear().await?;
                elem.send_keys(text).await?;
                json!({
                    "status": "success",
                    "message": format!("Successfully typed text into element '{}'", selector)
                })
            }
            "screenshot" => {
                let path = arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'path' parameter for screenshot action"))?;
                driver.screenshot(std::path::Path::new(path)).await?;
                let opened = arguments
                    .get("open_after")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let device_inventory_recorded = if opened {
                    let target = path.to_string();
                    tokio::task::spawn_blocking(move || open::that(&target)).await??;
                    crate::tools::device_inventory::record_successful_default_open(path)
                        .ok()
                        .flatten()
                } else {
                    None
                };
                json!({
                    "status": "success",
                    "path": path,
                    "opened": opened,
                    "device_inventory_recorded": device_inventory_recorded,
                    "message": if opened { format!("Screenshot saved and opened with the system viewer: {}", path) } else { format!("Screenshot saved to {}", path) }
                })
            }
            "eval" => {
                let script = arguments
                    .get("script")
                    .and_then(|v| v.as_str())
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
            _ => return Err(anyhow!("Unknown action: {}", action)),
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geckodriver_missing_message_mentions_preflight_and_inspection() {
        let msg = geckodriver_missing_message();
        assert!(msg.contains("Browser preflight failed"));
        assert!(msg.contains("inspect_browsers"));
        assert!(msg.contains("Chrome CDP"));
    }

    #[test]
    fn geckodriver_missing_preflight_error_is_structured() {
        let payload = geckodriver_missing_preflight_error();
        assert_eq!(
            payload["browser_preflight"]["health"]["geckodriver"],
            "missing"
        );
        assert!(payload["error"]
            .as_str()
            .expect("error string")
            .contains("Browser preflight failed"));
    }

    #[test]
    fn firefox_mode_defaults_and_validates() {
        assert_eq!(
            requested_mode(&json!({})).expect("default mode"),
            "headless"
        );
        assert_eq!(
            requested_mode(&json!({"mode": "visible"})).expect("visible mode"),
            "visible"
        );
        assert_eq!(
            requested_mode(&json!({"mode": "attach"})).expect("attach mode"),
            "attach"
        );
        assert!(requested_mode(&json!({"mode": "unknown"})).is_err());
    }

    #[test]
    fn firefox_attach_mode_uses_separate_webdriver_port() {
        assert_eq!(webdriver_port("headless"), 4444);
        assert_eq!(webdriver_port("visible"), 4444);
        assert_eq!(webdriver_port("attach"), 4445);
    }

    #[tokio::test]
    async fn test_firefox_browser_tool_metadata() -> Result<()> {
        let tool = FirefoxBrowserTool::new();
        assert_eq!(tool.name(), "firefox_browser");
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
        let modes = params["properties"]["mode"]["enum"]
            .as_array()
            .expect("mode enum");
        assert!(modes.iter().any(|mode| mode == "attach"));
        Ok(())
    }
}
