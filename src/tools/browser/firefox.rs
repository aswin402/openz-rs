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

pub fn resolve_firefox_action_and_url(arguments: &Value) -> (String, Option<String>) {
    if let Some(s) = arguments.as_str() {
        let trimmed = s.trim();
        let lower = trimmed.to_lowercase();
        if lower == "close" || lower == "quit" || lower == "stop" || lower == "exit" {
            return ("close".to_string(), None);
        } else if lower == "render" || lower == "view" || lower == "source" || lower == "snapshot" {
            return ("render".to_string(), None);
        } else if lower == "media_status" || lower == "media" || lower == "status" {
            return ("media_status".to_string(), None);
        } else {
            return ("navigate".to_string(), Some(trimmed.to_string()));
        }
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
        .or_else(|| arguments.get("href"))
        .or_else(|| arguments.get("page"))
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
                "render".to_string()
            }
        }
    };

    (action, url_opt)
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
                v.as_u64().or_else(|| {
                    v.as_str()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                })
            })
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

        let result = match action.as_str() {
            "navigate" | "goto" | "open" => {
                let raw_url = url_arg
                    .ok_or_else(|| anyhow!("Missing 'url' parameter for navigate action"))?;
                let url = crate::tools::web::normalize_web_url(&raw_url);
                if let Err(e) = driver.goto(&url).await {
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
                    .or_else(|| arguments.get("css_selector"))
                    .or_else(|| arguments.get("cssSelector"))
                    .or_else(|| arguments.get("css"))
                    .or_else(|| arguments.get("element"))
                    .or_else(|| arguments.get("query"))
                    .or_else(|| arguments.get("target"))
                    .and_then(|v| v.as_str())
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
                let selector = arguments
                    .get("selector")
                    .or_else(|| arguments.get("css_selector"))
                    .or_else(|| arguments.get("cssSelector"))
                    .or_else(|| arguments.get("css"))
                    .or_else(|| arguments.get("element"))
                    .or_else(|| arguments.get("query"))
                    .or_else(|| arguments.get("target"))
                    .and_then(|v| v.as_str())
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
                let selector = arguments
                    .get("selector")
                    .or_else(|| arguments.get("css_selector"))
                    .or_else(|| arguments.get("cssSelector"))
                    .or_else(|| arguments.get("css"))
                    .or_else(|| arguments.get("element"))
                    .or_else(|| arguments.get("query"))
                    .or_else(|| arguments.get("target"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'selector' parameter for fill action"))?
                    .trim();
                let text = arguments
                    .get("text")
                    .or_else(|| arguments.get("value"))
                    .or_else(|| arguments.get("query"))
                    .or_else(|| arguments.get("content"))
                    .or_else(|| arguments.get("input"))
                    .or_else(|| arguments.get("string"))
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
                    .or_else(|| arguments.get("output"))
                    .or_else(|| arguments.get("output_path"))
                    .or_else(|| arguments.get("outputPath"))
                    .or_else(|| arguments.get("file_path"))
                    .or_else(|| arguments.get("file"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'path' parameter for screenshot action"))?
                    .trim();
                driver.screenshot(std::path::Path::new(path)).await?;
                let opened = arguments
                    .get("open_after")
                    .or_else(|| arguments.get("openAfter"))
                    .and_then(|v| {
                        v.as_bool().or_else(|| {
                            v.as_str().map(|s| {
                                let trimmed = s.trim().to_lowercase();
                                trimmed == "true" || trimmed == "1" || trimmed == "yes"
                            })
                        })
                    })
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
            "eval" | "eval_js" | "evaluate" | "js" => {
                let script = arguments
                    .get("script")
                    .or_else(|| arguments.get("expression"))
                    .or_else(|| arguments.get("code"))
                    .or_else(|| arguments.get("js"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'script' parameter for eval action"))?;
                let val = driver.execute(script, vec![]).await?;
                json!({
                    "status": "success",
                    "output": val.json().to_string()
                })
            }
            "render" | "snapshot" => {
                let url_opt = url_arg.or_else(|| {
                    arguments
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
                        .or_else(|| arguments.get("href"))
                        .or_else(|| arguments.get("page"))
                        .or_else(|| arguments.get("address"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                });
                if let Some(ref raw_url) = url_opt {
                    let url = crate::tools::web::normalize_web_url(raw_url);
                    if let Err(e) = driver.goto(&url).await {
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
            _ => return Err(anyhow!("Unknown action: {}", raw_action)),
        };

        Ok(result)
    }
}

#[cfg(test)]
#[path = "firefox_tests.rs"]
mod tests;

