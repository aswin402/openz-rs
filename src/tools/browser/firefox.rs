use super::common::{
    extract_first_str, PATH_ALIASES, SCRIPT_ALIASES, SELECTOR_ALIASES, TEXT_ALIASES, URL_ALIASES,
};
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
    let status_url = format!("http://127.0.0.1:{}/status", port);

    match client.get(&status_url).send().await.is_ok() {
        true => return Ok(()),
        false => {}
    }

    let mut args = vec!["--port".to_string(), port.to_string()];
    args.extend(connect_existing.then(|| "--connect-existing".to_string()));
    let cmd_desc = match connect_existing {
        true => "geckodriver --connect-existing",
        false => "geckodriver --port 4444",
    };

    match Command::new("geckodriver")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child_handle) => {
            crate::shutdown::register_child_with_metadata(child_handle, cmd_desc, "geckodriver");
            for _ in 0..15 {
                sleep(Duration::from_millis(200)).await;
                match client.get(&status_url).send().await.is_ok() {
                    true => return Ok(()),
                    false => {}
                }
            }
        }
        Err(_) => {}
    }

    Err(anyhow!(geckodriver_missing_preflight_error().to_string()))
}

fn requested_mode(arguments: &Value) -> Result<&str> {
    let mode = arguments
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("headless");
    matches!(mode, "headless" | "visible" | "attach")
        .then_some(mode)
        .ok_or_else(|| {
            anyhow!(
                "Unsupported Firefox mode {}; use headless, visible, or attach",
                mode
            )
        })
}

fn webdriver_port(mode: &str) -> u16 {
    let config = crate::config::loader::load_config().ok();
    let (variable, configured) = match mode {
        "attach" => (
            "OPENZ_FIREFOX_ATTACH_PORT",
            config
                .map(|value| value.browser.firefox_attach_port)
                .unwrap_or(4445),
        ),
        _ => (
            "OPENZ_FIREFOX_WEBDRIVER_PORT",
            config
                .map(|value| value.browser.firefox_webdriver_port)
                .unwrap_or(4444),
        ),
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

    match guard.as_ref() {
        Some(session) => {
            let reusable = session.mode == mode && session.driver.title().await.is_ok();
            match reusable {
                true => return Ok(session.driver.clone()),
                false => {}
            }
        }
        None => {}
    }
    match guard.take() {
        Some(session) => {
            let _ = session.driver.quit().await;
        }
        None => {}
    }

    let connect_existing = mode == "attach";
    let port = webdriver_port(mode);
    ensure_geckodriver_running(port, connect_existing).await?;

    let mut caps = DesiredCapabilities::firefox();
    match mode {
        "headless" => caps.add_arg("--headless")?,
        _ => {}
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
    match guard.take() {
        Some(session) => {
            let _ = session.driver.quit().await;
        }
        None => {}
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
            Err(error) => match tokio::time::Instant::now() < deadline {
                true => {
                    let _ = error;
                    sleep(Duration::from_millis(250)).await;
                }
                false => {
                    return Err(anyhow!(
                        "Element {} was not found within {} seconds: {}",
                        selector,
                        timeout_secs,
                        error
                    ));
                }
            },
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

pub fn resolve_firefox_action_and_url(arguments: &Value) -> (String, Option<String>) {
    match arguments.as_str() {
        Some(s) => {
            let trimmed = s.trim();
            let lower = trimmed.to_lowercase();
            return match lower.as_str() {
                "close" | "quit" | "stop" | "exit" => ("close".to_string(), None),
                "render" | "view" | "source" | "snapshot" => ("render".to_string(), None),
                "media_status" | "media" | "status" => ("media_status".to_string(), None),
                _ => ("navigate".to_string(), Some(trimmed.to_string())),
            };
        }
        None => {}
    }

    super::common::resolve_browser_object_action_and_url(arguments, "render")
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
        let (raw_action, url_arg) = resolve_firefox_action_and_url(arguments);
        let action = raw_action.trim().to_lowercase().replace('-', "_");
        let mode = requested_mode(arguments)?;
        let timeout_secs = arguments
            .get("timeout_secs")
            .or_else(|| arguments.get("timeout"))
            .or_else(|| arguments.get("timeoutSecs"))
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
            })
            .unwrap_or(10);

        match action.as_str() {
            "close" => {
                reset_driver().await;
                return Ok(json!({
                    "status": "success",
                    "message": "Successfully closed the Firefox browser instance"
                }));
            }
            _ => {}
        }

        let driver = match get_or_create_driver(mode).await {
            Ok(d) => d,
            Err(e) => {
                reset_driver().await;
                return Err(anyhow!("Failed to initialize Firefox WebDriver: {:?}", e));
            }
        };

        let result = match action.as_str() {
            "navigate" | "goto" | "open" => {
                let raw_url = url_arg
                    .ok_or_else(|| anyhow!("Missing 'url' parameter for navigate action"))?;
                let url = crate::tools::web::normalize_web_url(&raw_url);
                match driver.goto(&url).await {
                    Ok(()) => {}
                    Err(e) => {
                        reset_driver().await;
                        return Err(anyhow!("Navigation failed: {:?}", e));
                    }
                }
                json!({
                    "status": "success",
                    "message": format!("Successfully navigated to {}", url)
                })
            }
            "wait_for" => {
                let selector = extract_first_str(arguments, SELECTOR_ALIASES)
                    .ok_or_else(|| anyhow!("Missing selector for wait_for"))?
                    .trim();
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
                let selector = extract_first_str(arguments, SELECTOR_ALIASES)
                    .ok_or_else(|| anyhow!("Missing 'selector' parameter for click action"))?
                    .trim();
                let elem = wait_for_element(&driver, selector, timeout_secs).await?;
                elem.click().await?;
                json!({
                    "status": "success",
                    "message": format!("Successfully clicked element '{}'", selector)
                })
            }
            "fill" => {
                let selector = extract_first_str(arguments, SELECTOR_ALIASES)
                    .ok_or_else(|| anyhow!("Missing 'selector' parameter for fill action"))?
                    .trim();
                let text = extract_first_str(arguments, TEXT_ALIASES)
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
                let path = extract_first_str(arguments, PATH_ALIASES)
                    .ok_or_else(|| anyhow!("Missing 'path' parameter for screenshot action"))?
                    .trim();
                driver.screenshot(std::path::Path::new(path)).await?;
                let opened = arguments
                    .get("open_after")
                    .or_else(|| arguments.get("openAfter"))
                    .and_then(|v| {
                        v.as_bool().or_else(|| {
                            v.as_str().map(|s| {
                                matches!(s.trim().to_lowercase().as_str(), "true" | "1" | "yes")
                            })
                        })
                    })
                    .unwrap_or(false);
                let (device_inventory_recorded, message) = match opened {
                    true => {
                        let target = path.to_string();
                        tokio::task::spawn_blocking(move || open::that(&target)).await??;
                        let rec = crate::tools::device_inventory::record_successful_default_open(
                            path,
                        )
                        .ok()
                        .flatten();
                        (
                            rec,
                            format!(
                                "Screenshot saved and opened with the system viewer: {}",
                                path
                            ),
                        )
                    }
                    false => (None, format!("Screenshot saved to {}", path)),
                };
                json!({
                    "status": "success",
                    "path": path,
                    "opened": opened,
                    "device_inventory_recorded": device_inventory_recorded,
                    "message": message
                })
            }
            "eval" | "eval_js" | "evaluate" | "js" => {
                let script = extract_first_str(arguments, SCRIPT_ALIASES)
                    .ok_or_else(|| anyhow!("Missing 'script' parameter for eval action"))?;
                let val = driver.execute(script, vec![]).await?;
                json!({
                    "status": "success",
                    "output": val.json().to_string()
                })
            }
            "render" | "snapshot" => {
                let url_opt = url_arg.or_else(|| {
                    extract_first_str(arguments, URL_ALIASES).map(ToString::to_string)
                });
                match url_opt {
                    Some(ref raw_url) => {
                        let url = crate::tools::web::normalize_web_url(raw_url);
                        match driver.goto(&url).await {
                            Ok(()) => {}
                            Err(e) => {
                                reset_driver().await;
                                return Err(anyhow!("Navigation failed: {:?}", e));
                            }
                        }
                    }
                    None => {}
                }
                let html = driver.source().await?;
                let md = html2md::parse_html(&html);
                json!({
                    "status": "success",
                    "output": md
                })
            }
            _ => return Err(anyhow!("Unknown action: {}", raw_action)),
        };

        Ok(result)
    }
}

#[cfg(test)]
#[path = "firefox_tests.rs"]
mod tests;
