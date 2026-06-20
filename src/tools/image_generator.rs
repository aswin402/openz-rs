use crate::tools::Tool;
use crate::config::resolve_path;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use futures_util::stream::{SplitSink, SplitStream};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use base64::prelude::*;

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type WsStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub struct GenerateImageTool;

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
        .args(["serve", "--port", "9222"])
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
            .args([
                "--headless",
                "--remote-debugging-port=9222",
                "--disable-gpu",
                "--no-sandbox",
                "--disable-dev-shm-usage",
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

struct ServerGuard(Option<tokio::task::JoinHandle<()>>);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(ref handle) = self.0 {
            handle.abort();
        }
    }
}

fn escape_html(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[async_trait::async_trait]
impl Tool for GenerateImageTool {
    fn name(&self) -> &str {
        "generate_image"
    }

    fn description(&self) -> &str {
        "Generates a premium, high-fidelity PNG image from HTML/CSS, a local template file, or an online URL using headless Chromium. Supports legacy geometric shape inputs as fallback."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "html": {
                    "type": "string",
                    "description": "Raw HTML content to render. Can contain Tailwind CSS/CDN, Google Fonts, inline styles, flex/grid layouts, SVG, canvas, gradients, drop-shadows, etc."
                },
                "html_path": {
                    "type": "string",
                    "description": "Local file path (e.g., 'templates/card.html') or absolute file:// URL to render."
                },
                "url": {
                    "type": "string",
                    "description": "A web URL (e.g. 'https://google.com') to navigate to and render."
                },
                "css": {
                    "type": "string",
                    "description": "Custom CSS to inject into the page before taking the screenshot."
                },
                "width": {
                    "type": "integer",
                    "description": "Width of the viewport/canvas in pixels (default: 800)",
                    "default": 800
                },
                "height": {
                    "type": "integer",
                    "description": "Height of the viewport/canvas in pixels (default: 800)",
                    "default": 800
                },
                "device_scale_factor": {
                    "type": "number",
                    "description": "Device scale factor for rendering (default: 2.0 for Retina/high-DPI crispness)",
                    "default": 2.0
                },
                "background_color": {
                    "type": "string",
                    "description": "Hex color code for the background (used if rendering legacy shapes, e.g. '#ffffff')",
                    "default": "#ffffff"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector of the specific element to capture (e.g., '.card', '#widget'). If provided, screenshots only that element's bounding rect."
                },
                "settle_ms": {
                    "type": "integer",
                    "description": "Delay in milliseconds to wait after page load before taking the screenshot, allowing layout and fonts to settle (default: 300)",
                    "default": 300
                },
                "output_path": {
                    "type": "string",
                    "description": "Path where the generated PNG will be saved (default: 'output.png')",
                    "default": "output.png"
                },
                "shapes": {
                    "type": "array",
                    "description": "Legacy fallback: List of geometric shapes and text to draw sequentially on the canvas",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "description": "The type of drawing operation: 'rect', 'circle', 'line', or 'text'"
                            },
                            "color": {
                                "type": "string",
                                "description": "Hex color code for this operation (e.g. '#ff0000')"
                            },
                            "fill": {
                                "type": "boolean",
                                "description": "Whether to fill the shape (only applicable for 'rect' and 'circle', default: true)"
                            },
                            "x": { "type": "integer", "description": "X coordinate for rectangle or text" },
                            "y": { "type": "integer", "description": "Y coordinate for rectangle or text" },
                            "w": { "type": "integer", "description": "Width for rectangle" },
                            "h": { "type": "integer", "description": "Height for rectangle" },
                            "cx": { "type": "integer", "description": "Center X coordinate for circle" },
                            "cy": { "type": "integer", "description": "Center Y coordinate for circle" },
                            "r": { "type": "integer", "description": "Radius for circle" },
                            "x1": { "type": "integer", "description": "Start X for line" },
                            "y1": { "type": "integer", "description": "Start Y for line" },
                            "x2": { "type": "integer", "description": "End X for line" },
                            "y2": { "type": "integer", "description": "End Y for line" },
                            "text": { "type": "string", "description": "Text content to draw" },
                            "size": { "type": "number", "description": "Font size for text (default: 16.0)" }
                        },
                        "required": ["type"]
                    }
                }
            },
            "required": ["output_path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let width = arguments.get("width").and_then(|v| v.as_i64()).unwrap_or(800);
        let height = arguments.get("height").and_then(|v| v.as_i64()).unwrap_or(800);
        let device_scale_factor = arguments.get("device_scale_factor").and_then(|v| v.as_f64()).unwrap_or(2.0);
        let bg_color_str = arguments.get("background_color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
        let output_path_str = arguments.get("output_path").and_then(|v| v.as_str()).unwrap_or("output.png");
        let output_path = resolve_path(output_path_str);
        let selector = arguments.get("selector").and_then(|v| v.as_str());
        let settle_ms = arguments.get("settle_ms").and_then(|v| v.as_i64()).unwrap_or(300);

        let mut temp_file_path = None;

        // Resolve target URL to load
        let target_url = if let Some(html_content) = arguments.get("html").and_then(|v| v.as_str()) {
            let temp_html_path = std::env::temp_dir().join(format!("openz_img_{}.html", uuid::Uuid::new_v4()));
            fs::write(&temp_html_path, html_content)?;
            temp_file_path = Some(temp_html_path.clone());
            format!("file://{}", temp_html_path.to_string_lossy())
        } else if let Some(html_path_str) = arguments.get("html_path").and_then(|v| v.as_str()) {
            if html_path_str.starts_with("http://") || html_path_str.starts_with("https://") || html_path_str.starts_with("file://") {
                html_path_str.to_string()
            } else {
                let path = resolve_path(html_path_str);
                format!("file://{}", path.to_string_lossy())
            }
        } else if let Some(url_str) = arguments.get("url").and_then(|v| v.as_str()) {
            url_str.to_string()
        } else if let Some(shapes_val) = arguments.get("shapes").and_then(|v| v.as_array()) {
            // Compile shapes to premium anti-aliased SVG
            let mut svg_elements = String::new();
            for shape in shapes_val {
                let shape_type = shape.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let color_str = shape.get("color").and_then(|v| v.as_str()).unwrap_or("#000000");
                let fill = shape.get("fill").and_then(|v| v.as_bool()).unwrap_or(true);
                match shape_type {
                    "rect" => {
                        let x = shape.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
                        let y = shape.get("y").and_then(|v| v.as_i64()).unwrap_or(0);
                        let w = shape.get("w").and_then(|v| v.as_u64()).unwrap_or(10);
                        let h = shape.get("h").and_then(|v| v.as_u64()).unwrap_or(10);
                        let fill_val = if fill { color_str } else { "none" };
                        let stroke_val = if fill { "none" } else { color_str };
                        let stroke_w = if fill { 0 } else { 2 };
                        svg_elements.push_str(&format!(
                            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}" />"#,
                            x, y, w, h, fill_val, stroke_val, stroke_w
                        ));
                    }
                    "circle" => {
                        let cx = shape.get("cx").and_then(|v| v.as_i64()).unwrap_or(0);
                        let cy = shape.get("cy").and_then(|v| v.as_i64()).unwrap_or(0);
                        let r = shape.get("r").and_then(|v| v.as_i64()).unwrap_or(10);
                        let fill_val = if fill { color_str } else { "none" };
                        let stroke_val = if fill { "none" } else { color_str };
                        let stroke_w = if fill { 0 } else { 2 };
                        svg_elements.push_str(&format!(
                            r#"<circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="{}" />"#,
                            cx, cy, r, fill_val, stroke_val, stroke_w
                        ));
                    }
                    "line" => {
                        let x1 = shape.get("x1").and_then(|v| v.as_i64()).unwrap_or(0);
                        let y1 = shape.get("y1").and_then(|v| v.as_i64()).unwrap_or(0);
                        let x2 = shape.get("x2").and_then(|v| v.as_i64()).unwrap_or(0);
                        let y2 = shape.get("y2").and_then(|v| v.as_i64()).unwrap_or(0);
                        svg_elements.push_str(&format!(
                            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2" />"#,
                            x1, y1, x2, y2, color_str
                        ));
                    }
                    "text" => {
                        let x = shape.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
                        let y = shape.get("y").and_then(|v| v.as_i64()).unwrap_or(0);
                        let text_val = shape.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        let size = shape.get("size").and_then(|v| v.as_f64()).unwrap_or(16.0);
                        svg_elements.push_str(&format!(
                            r#"<text x="{}" y="{}" fill="{}" font-size="{}px" font-family="sans-serif" dominant-baseline="hanging">{}</text>"#,
                            x, y, color_str, size, escape_html(text_val)
                        ));
                    }
                    _ => {}
                }
            }
            let html_content = format!(
                r#"<!DOCTYPE html>
                <html>
                <head>
                <style>
                  body, html {{
                    margin: 0;
                    padding: 0;
                    width: {}px;
                    height: {}px;
                    background-color: {};
                    overflow: hidden;
                  }}
                  svg {{
                    display: block;
                    width: 100%;
                    height: 100%;
                  }}
                </style>
                </head>
                <body>
                  <svg width="{}" height="{}" viewBox="0 0 {} {}" xmlns="http://www.w3.org/2000/svg">
                    {}
                  </svg>
                </body>
                </html>"#,
                width, height, bg_color_str, width, height, width, height, svg_elements
            );
            let temp_html_path = std::env::temp_dir().join(format!("openz_img_{}.html", uuid::Uuid::new_v4()));
            fs::write(&temp_html_path, html_content)?;
            temp_file_path = Some(temp_html_path.clone());
            format!("file://{}", temp_html_path.to_string_lossy())
        } else {
            // Default blank white page if nothing specified
            let default_html = r#"<!DOCTYPE html><html><body style="margin:0; background:white;"></body></html>"#;
            let temp_html_path = std::env::temp_dir().join(format!("openz_img_{}.html", uuid::Uuid::new_v4()));
            fs::write(&temp_html_path, default_html)?;
            temp_file_path = Some(temp_html_path.clone());
            format!("file://{}", temp_html_path.to_string_lossy())
        };

        let mut _server_guard = ServerGuard(None);
        let target_url = if let Some(file_path_str) = target_url.strip_prefix("file://") {
            let mut raw_path = std::path::PathBuf::from(file_path_str);
            if !raw_path.is_absolute() {
                if let Ok(cwd) = std::env::current_dir() {
                    raw_path = cwd.join(raw_path);
                }
            }
            let parent_dir = raw_path.parent().unwrap_or(&raw_path).to_path_buf();
            let filename = raw_path.file_name().and_then(|n| n.to_str()).unwrap_or("index.html").to_string();

            match tokio::net::TcpListener::bind("127.0.0.1:0").await {
                Ok(listener) => {
                    if let Ok(addr) = listener.local_addr() {
                        let port = addr.port();
                        let app = axum::Router::new()
                            .nest_service("/", tower_http::services::ServeDir::new(&parent_dir))
                            .layer(axum::middleware::from_fn(force_utf8));
                        let handle = tokio::spawn(async move {
                            let _ = axum::serve(listener, app).await;
                        });
                        _server_guard.0 = Some(handle);
                        format!("http://127.0.0.1:{}/{}", port, filename)
                    } else {
                        target_url
                    }
                }
                Err(_) => {
                    target_url
                }
            }
        } else {
            target_url
        };

        // Ensure browser is running
        ensure_browser_running().await?;

        let client = reqwest::Client::new();
        let mut res = client.put("http://127.0.0.1:9222/json/new")
            .send()
            .await;
        
        if res.is_err() || !res.as_ref().unwrap().status().is_success() {
            res = client.get("http://127.0.0.1:9222/json/new")
                .send()
                .await;
        }
        
        if res.is_err() || !res.as_ref().unwrap().status().is_success() {
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
            if let Some(path) = &temp_file_path {
                let _ = fs::remove_file(path);
            }
            return Err(anyhow!("Failed to create a new tab via CDP HTTP API"));
        }

        let tab_info: Value = res.json().await?;
        let tab_id = tab_info.get("id").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("No tab ID returned from /json/new"))?;
        let ws_url = tab_info.get("webSocketDebuggerUrl").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("No webSocketDebuggerUrl returned from /json/new"))?;

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, mut read) = ws_stream.split();
        let mut message_id = 0u64;

        // Enable Page and Runtime domains
        let _ = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Page.enable", json!({})).await?;
        let _ = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Runtime.enable", json!({})).await?;

        // Set device metrics
        let _ = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Emulation.setDeviceMetricsOverride", json!({
            "width": width,
            "height": height,
            "deviceScaleFactor": device_scale_factor,
            "mobile": false
        })).await?;

        // Navigate to target URL
        let _ = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Page.navigate", json!({ "url": target_url })).await?;

        // Poll document.readyState until complete or timeout (max 10 seconds)
        let start_time = Instant::now();
        let max_duration = Duration::from_secs(10);
        let mut is_complete = false;

        while start_time.elapsed() < max_duration {
            sleep(Duration::from_millis(200)).await;
            
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

        // Inject custom CSS if provided
        if let Some(css_str) = arguments.get("css").and_then(|v| v.as_str()) {
            let inject_css_js = format!(
                r#"(() => {{
                    const style = document.createElement('style');
                    style.textContent = `{}`;
                    document.head.appendChild(style);
                }})()"#,
                css_str.replace('\\', "\\\\").replace('`', "\\`").replace("${", "\\${")
            );
            let _ = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Runtime.evaluate", json!({
                "expression": inject_css_js,
                "returnByValue": true
            })).await?;
        }

        // Settle delay
        if settle_ms > 0 {
            sleep(Duration::from_millis(settle_ms as u64)).await;
        }

        // Determine screenshot bounding box if selector is provided
        let mut screenshot_params = json!({
            "format": "png"
        });

        if let Some(sel) = selector {
            // Properly escape the selector for safe JS injection
            let escaped_sel = sel.replace('\\', "\\\\").replace('"', "\\\"").replace('\'', "\\'").replace('\n', "\\n").replace('\r', "\\r");
            let js_expr = format!(
                r#"((sel) => {{
                    const el = document.querySelector(sel);
                    if (!el) return null;
                    const rect = el.getBoundingClientRect();
                    return {{
                        x: rect.left + window.scrollX,
                        y: rect.top + window.scrollY,
                        width: rect.width,
                        height: rect.height,
                        scale: 1.0
                    }};
                }})('{}')"#,
                escaped_sel
            );

            let eval_res = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Runtime.evaluate", json!({
                "expression": js_expr,
                "returnByValue": true
            })).await?;

            if let Some(rect_val) = eval_res
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"))
            {
                if !rect_val.is_null() {
                    screenshot_params["clip"] = rect_val.clone();
                }
            }
        }

        // Capture screenshot
        let screenshot_res = send_cdp_cmd(&mut write, &mut read, &mut message_id, "Page.captureScreenshot", screenshot_params).await?;

        let base64_data = screenshot_res.pointer("/result/data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Failed to capture screenshot data"))?;

        let image_bytes = BASE64_STANDARD.decode(base64_data)?;

        // Ensure parent directories exist
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write screenshot to file
        fs::write(&output_path, image_bytes)?;

        // Close the tab
        let close_url = format!("http://127.0.0.1:9222/json/close/{}", tab_id);
        let _ = client.get(&close_url).send().await;

        // Clean up temp file if created
        if let Some(path) = &temp_file_path {
            let _ = fs::remove_file(path);
        }

        Ok(json!({
            "status": "success",
            "complete": is_complete,
            "output_path": output_path.to_string_lossy(),
            "message": format!("High-fidelity image successfully generated and saved to '{}'.", output_path_str)
        }))
    }
}

async fn force_utf8(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(req).await;
    if let Some(content_type) = response.headers().get(axum::http::header::CONTENT_TYPE) {
        if let Ok(content_type_str) = content_type.to_str() {
            if content_type_str.starts_with("text/html") && !content_type_str.contains("charset") {
                if let Ok(new_val) = axum::http::header::HeaderValue::from_str("text/html; charset=utf-8") {
                    response.headers_mut().insert(axum::http::header::CONTENT_TYPE, new_val);
                }
            }
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_image_tool_metadata() -> Result<()> {
        let tool = GenerateImageTool;
        assert_eq!(tool.name(), "generate_image");
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
        Ok(())
    }

    #[tokio::test]
    async fn test_generate_image_tool_execution() -> Result<()> {
        let tool = GenerateImageTool;
        let temp_png = std::env::temp_dir().join("test_output_img.png");
        let _ = std::fs::remove_file(&temp_png);

        let args = json!({
            "html": "<html><body style='margin:0; background:linear-gradient(to right, #ff7e5f, #feb47b); width:100vw; height:100vh; display:flex; align-items:center; justify-content:center;'><h1 style='color:white; font-family:sans-serif;'>Hello High-Fidelity OpenZ!</h1></body></html>",
            "width": 400,
            "height": 300,
            "device_scale_factor": 1.0,
            "output_path": temp_png.to_string_lossy()
        });

        // Skip execution if browser is not available/runnable in the sandbox context
        if let Err(e) = ensure_browser_running().await {
            eprintln!("Skipping execution test as headless browser is unavailable: {}", e);
            return Ok(());
        }

        let result = tool.call(&args).await?;
        assert_eq!(result.get("status").and_then(|v| v.as_str()), Some("success"));
        assert!(temp_png.exists());

        let _ = std::fs::remove_file(&temp_png);
        Ok(())
    }
}

