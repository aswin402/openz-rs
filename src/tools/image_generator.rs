use crate::config::resolve_path;
use crate::tools::browser_common::{
    browser_cdp_port, connect_to_tab, ensure_browser_running, kill_browser_on_cdp_port,
    obtain_tab_websocket_url, send_cdp_cmd,
};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use base64::prelude::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub struct GenerateImageTool;

pub(crate) struct LocalHtmlServerGuard(pub(crate) Option<tokio::task::JoinHandle<()>>);

impl Drop for LocalHtmlServerGuard {
    fn drop(&mut self) {
        match self.0 {
            Some(ref handle) => handle.abort(),
            None => {}
        }
    }
}

pub(crate) async fn force_utf8(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(req).await;
    let should_rewrite = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|ct| ct.to_str().ok())
        .map(|s| s.starts_with("text/html") && !s.contains("charset"))
        .unwrap_or(false);
    match should_rewrite {
        true => match axum::http::header::HeaderValue::from_str("text/html; charset=utf-8") {
            Ok(new_val) => {
                response
                    .headers_mut()
                    .insert(axum::http::header::CONTENT_TYPE, new_val);
            }
            Err(_) => {}
        },
        false => {}
    }
    response
}

pub(crate) async fn serve_local_html_file(mut raw_path: PathBuf) -> (String, LocalHtmlServerGuard) {
    match raw_path.is_absolute() {
        true => {}
        false => match std::env::current_dir() {
            Ok(cwd) => {
                raw_path = cwd.join(raw_path);
            }
            Err(_) => {}
        },
    }

    let fallback_url = format!("file://{}", raw_path.to_string_lossy());
    let parent_dir = raw_path.parent().unwrap_or(&raw_path).to_path_buf();
    let filename = raw_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("index.html")
        .to_string();

    match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => match listener.local_addr() {
            Ok(addr) => {
                let port = addr.port();
                let app = axum::Router::new()
                    .nest_service("/", tower_http::services::ServeDir::new(&parent_dir))
                    .layer(axum::middleware::from_fn(force_utf8));
                let handle = tokio::spawn(async move {
                    let _ = axum::serve(listener, app).await;
                });
                (
                    format!("http://127.0.0.1:{}/{}", port, filename),
                    LocalHtmlServerGuard(Some(handle)),
                )
            }
            Err(_) => (fallback_url, LocalHtmlServerGuard(None)),
        },
        Err(_) => (fallback_url, LocalHtmlServerGuard(None)),
    }
}

use crate::agent::style::escape_html;

fn fill_stroke_attrs(fill: bool, color_str: &str) -> (&str, &str, u8) {
    match fill {
        true => (color_str, "none", 0),
        false => ("none", color_str, 2),
    }
}

fn render_shape_svg(shape: &Value) -> String {
    let shape_type = shape.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let color_str = shape
        .get("color")
        .and_then(|v| v.as_str())
        .unwrap_or("#000000");
    let fill = shape.get("fill").and_then(|v| v.as_bool()).unwrap_or(true);
    let (fill_val, stroke_val, stroke_w) = fill_stroke_attrs(fill, color_str);
    match shape_type {
        "rect" => {
            let x = shape.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
            let y = shape.get("y").and_then(|v| v.as_i64()).unwrap_or(0);
            let w = shape.get("w").and_then(|v| v.as_u64()).unwrap_or(10);
            let h = shape.get("h").and_then(|v| v.as_u64()).unwrap_or(10);
            format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}" />"#,
                x, y, w, h, fill_val, stroke_val, stroke_w
            )
        }
        "circle" => {
            let cx = shape.get("cx").and_then(|v| v.as_i64()).unwrap_or(0);
            let cy = shape.get("cy").and_then(|v| v.as_i64()).unwrap_or(0);
            let r = shape.get("r").and_then(|v| v.as_i64()).unwrap_or(10);
            format!(
                r#"<circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="{}" />"#,
                cx, cy, r, fill_val, stroke_val, stroke_w
            )
        }
        "line" => {
            let x1 = shape.get("x1").and_then(|v| v.as_i64()).unwrap_or(0);
            let y1 = shape.get("y1").and_then(|v| v.as_i64()).unwrap_or(0);
            let x2 = shape.get("x2").and_then(|v| v.as_i64()).unwrap_or(0);
            let y2 = shape.get("y2").and_then(|v| v.as_i64()).unwrap_or(0);
            format!(
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2" />"#,
                x1, y1, x2, y2, color_str
            )
        }
        "text" => {
            let x = shape.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
            let y = shape.get("y").and_then(|v| v.as_i64()).unwrap_or(0);
            let text_val = shape.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let size = shape.get("size").and_then(|v| v.as_f64()).unwrap_or(16.0);
            format!(
                r#"<text x="{}" y="{}" fill="{}" font-size="{}px" font-family="sans-serif" dominant-baseline="hanging">{}</text>"#,
                x, y, color_str, size, escape_html(text_val)
            )
        }
        _ => String::new(),
    }
}

fn write_temp_html_file(html_content: &str) -> Result<(String, Option<PathBuf>)> {
    let temp_html_path =
        std::env::temp_dir().join(format!("openz_img_{}.html", uuid::Uuid::new_v4()));
    fs::write(&temp_html_path, html_content)?;
    let url = format!("file://{}", temp_html_path.to_string_lossy());
    Ok((url, Some(temp_html_path)))
}

fn resolve_target_url_from_arguments(
    arguments: &Value,
    width: i64,
    height: i64,
    bg_color_str: &str,
) -> Result<(String, Option<PathBuf>)> {
    match arguments.get("html").and_then(|v| v.as_str()) {
        Some(html_content) => write_temp_html_file(html_content),
        None => match arguments.get("html_path").and_then(|v| v.as_str()) {
            Some(html_path_str) => {
                let has_scheme = html_path_str.starts_with("http://")
                    || html_path_str.starts_with("https://")
                    || html_path_str.starts_with("file://");
                let url = match has_scheme {
                    true => html_path_str.to_string(),
                    false => {
                        let path = resolve_path(html_path_str);
                        format!("file://{}", path.to_string_lossy())
                    }
                };
                Ok((url, None))
            }
            None => match arguments.get("url").and_then(|v| v.as_str()) {
                Some(url_str) => Ok((url_str.to_string(), None)),
                None => match arguments.get("shapes").and_then(|v| v.as_array()) {
                    Some(shapes_val) => {
                        let svg_elements: String =
                            shapes_val.iter().map(render_shape_svg).collect();
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
                        write_temp_html_file(&html_content)
                    }
                    None => {
                        let default_html = r#"<!DOCTYPE html><html><body style="margin:0; background:white;"></body></html>"#;
                        write_temp_html_file(default_html)
                    }
                },
            },
        },
    }
}

fn cleanup_temp_file(temp_file_path: &Option<PathBuf>) {
    match temp_file_path {
        Some(path) => {
            let _ = fs::remove_file(path);
        }
        None => {}
    }
}

fn ensure_parent_dir(output_path: &Path) -> Result<()> {
    match output_path.parent() {
        Some(parent) => fs::create_dir_all(parent)?,
        None => {}
    }
    Ok(())
}

async fn try_openmedia_fallback(
    html_content: &str,
    width: i64,
    height: i64,
    device_scale_factor: f64,
    output_path: &Path,
    temp_file_path: &Option<PathBuf>,
) -> Option<Value> {
    let server = crate::tools::openmedia::get_server().await.ok()?;
    let req = crate::tools::openmedia::server::HtmlToImageRequest {
        html: html_content.to_string(),
        width: Some(width as u32),
        height: Some(height as u32),
        device_scale_factor: Some(device_scale_factor),
        output_format: Some("png".to_string()),
    };
    let val = server.html_to_image(req).await.ok()?;
    let path_str = val.get("path").and_then(|v| v.as_str())?;
    let gen_path = Path::new(path_str);
    gen_path.exists().then_some(())?;
    match output_path.parent() {
        Some(parent) => {
            let _ = fs::create_dir_all(parent);
        }
        None => {}
    }
    let _ = fs::copy(gen_path, output_path);
    cleanup_temp_file(temp_file_path);
    Some(json!({
        "status": "success",
        "complete": true,
        "output_path": output_path.to_string_lossy(),
        "backend": "openmedia_chromiumoxide_fallback",
        "width": width,
        "height": height,
    }))
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
        let width = arguments
            .get("width")
            .and_then(|v| v.as_i64())
            .unwrap_or(800);
        let height = arguments
            .get("height")
            .and_then(|v| v.as_i64())
            .unwrap_or(800);
        let device_scale_factor = arguments
            .get("device_scale_factor")
            .and_then(|v| v.as_f64())
            .unwrap_or(2.0);
        let bg_color_str = arguments
            .get("background_color")
            .and_then(|v| v.as_str())
            .unwrap_or("#ffffff");
        let output_path_str = arguments
            .get("output_path")
            .and_then(|v| v.as_str())
            .unwrap_or("output.png");
        let output_path = resolve_path(output_path_str);
        crate::tools::resource_policy::ensure_artifact_write_allowed("generate_image")?;
        let selector = arguments.get("selector").and_then(|v| v.as_str());
        let settle_ms = arguments
            .get("settle_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(300);

        let (initial_target_url, temp_file_path) =
            resolve_target_url_from_arguments(arguments, width, height, bg_color_str)?;

        let (target_url, _server_guard) = match initial_target_url.strip_prefix("file://") {
            Some(file_path_str) => serve_local_html_file(PathBuf::from(file_path_str)).await,
            None => (initial_target_url, LocalHtmlServerGuard(None)),
        };

        // Ensure browser is running
        ensure_browser_running().await?;

        let cdp_port = browser_cdp_port();
        let (tab_id, ws_url) = match obtain_tab_websocket_url(cdp_port).await {
            Ok(pair) => pair,
            Err(_) => {
                kill_browser_on_cdp_port();
                sleep(Duration::from_millis(500)).await;
                ensure_browser_running().await?;
                obtain_tab_websocket_url(cdp_port).await?
            }
        };

        // Connect to WebSocket
        let (mut write, mut read) = connect_to_tab(&ws_url).await?;
        let mut message_id = 0u64;

        // Enable Page and Runtime domains
        let _ = send_cdp_cmd(
            &mut write,
            &mut read,
            &mut message_id,
            "Page.enable",
            json!({}),
        )
        .await?;
        let _ = send_cdp_cmd(
            &mut write,
            &mut read,
            &mut message_id,
            "Runtime.enable",
            json!({}),
        )
        .await?;

        // Set device metrics
        let _ = send_cdp_cmd(
            &mut write,
            &mut read,
            &mut message_id,
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": width,
                "height": height,
                "deviceScaleFactor": device_scale_factor,
                "mobile": false
            }),
        )
        .await?;

        // Navigate to target URL
        let _ = send_cdp_cmd(
            &mut write,
            &mut read,
            &mut message_id,
            "Page.navigate",
            json!({ "url": target_url }),
        )
        .await?;

        // Poll document.readyState until complete or timeout (max 10 seconds)
        let start_time = Instant::now();
        let max_duration = Duration::from_secs(10);
        let mut is_complete = false;

        while start_time.elapsed() < max_duration {
            sleep(Duration::from_millis(200)).await;

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

            let ready = eval_res
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|res| res.get("value"))
                .and_then(|v| v.as_str())
                == Some("complete");
            match ready {
                true => {
                    is_complete = true;
                    break;
                }
                false => {}
            }
        }

        // Inject custom CSS if provided
        match arguments.get("css").and_then(|v| v.as_str()) {
            Some(css_str) => {
                let inject_css_js = format!(
                    r#"(() => {{
                        const style = document.createElement('style');
                        style.textContent = `{}`;
                        document.head.appendChild(style);
                    }})()"#,
                    css_str
                        .replace('\\', "\\\\")
                        .replace('`', "\\`")
                        .replace("${", "\\${")
                );
                let _ = send_cdp_cmd(
                    &mut write,
                    &mut read,
                    &mut message_id,
                    "Runtime.evaluate",
                    json!({
                        "expression": inject_css_js,
                        "returnByValue": true
                    }),
                )
                .await?;
            }
            None => {}
        }

        // Settle delay
        match settle_ms > 0 {
            true => sleep(Duration::from_millis(settle_ms as u64)).await,
            false => {}
        }

        // Determine screenshot bounding box if selector is provided
        let mut screenshot_params = json!({
            "format": "png"
        });

        match selector {
            Some(sel) => {
                let escaped_sel = sel
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\'', "\\'")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r");
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

                let eval_res = send_cdp_cmd(
                    &mut write,
                    &mut read,
                    &mut message_id,
                    "Runtime.evaluate",
                    json!({
                        "expression": js_expr,
                        "returnByValue": true
                    }),
                )
                .await?;

                match eval_res
                    .get("result")
                    .and_then(|r| r.get("result"))
                    .and_then(|res| res.get("value"))
                    .filter(|v| !v.is_null())
                {
                    Some(rect_val) => {
                        screenshot_params["clip"] = rect_val.clone();
                    }
                    None => {}
                }
            }
            None => {}
        }

        // Capture screenshot
        let screenshot_res = send_cdp_cmd(
            &mut write,
            &mut read,
            &mut message_id,
            "Page.captureScreenshot",
            screenshot_params,
        )
        .await?;

        let image_bytes = match screenshot_res
            .pointer("/result/data")
            .and_then(|v| v.as_str())
        {
            Some(base64_data) => BASE64_STANDARD.decode(base64_data)?,
            None => match arguments.get("html").and_then(|v| v.as_str()) {
                Some(html_content) => {
                    match try_openmedia_fallback(
                        html_content,
                        width,
                        height,
                        device_scale_factor,
                        &output_path,
                        &temp_file_path,
                    )
                    .await
                    {
                        Some(fallback_res) => return Ok(fallback_res),
                        None => {
                            return Err(anyhow!(
                                "Failed to capture screenshot data via CDP and fallback renderer"
                            ));
                        }
                    }
                }
                None => return Err(anyhow!("Failed to capture screenshot data")),
            },
        };

        ensure_parent_dir(&output_path)?;
        fs::write(&output_path, image_bytes)?;

        // Close the tab
        let client = crate::core::http::default_http_client();
        let close_url = format!("http://127.0.0.1:{cdp_port}/json/close/{}", tab_id);
        let _ = client.get(&close_url).send().await;

        cleanup_temp_file(&temp_file_path);

        Ok(json!({
            "status": "success",
            "complete": is_complete,
            "output_path": output_path.to_string_lossy(),
            "message": format!("High-fidelity image successfully generated and saved to '{}'.", output_path_str)
        }))
    }
}

#[cfg(test)]
#[path = "image_generator_tests.rs"]
mod tests;
