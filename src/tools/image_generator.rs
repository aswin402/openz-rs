use crate::config::resolve_path;
use crate::tools::browser::render_target_to_png;
use crate::tools::Tool;
use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

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

use crate::tools::wavyte::ensure_parent_dir;

#[async_trait::async_trait]
impl Tool for GenerateImageTool {
    fn name(&self) -> &str {
        "generate_image"
    }

    fn description(&self) -> &str {
        "Generates a premium, high-fidelity PNG image from HTML/CSS, a local template file, or an online URL using OpenZ's native headless WebKit2GTK renderer. Supports legacy geometric shape inputs as fallback."
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

        ensure_parent_dir(&output_path)?;
        let custom_css = arguments.get("css").and_then(|v| v.as_str());
        let render_res = render_target_to_png(
            Some(&target_url),
            None,
            custom_css,
            width as i32,
            height as i32,
            device_scale_factor,
            settle_ms.max(0) as u64,
            selector,
            false,
            &output_path,
        )
        .await;

        cleanup_temp_file(&temp_file_path);
        let snap = render_res?;

        Ok(json!({
            "status": "success",
            "complete": true,
            "backend": "openz_webkit2gtk",
            "output_path": output_path.to_string_lossy(),
            "width": snap.get("width").cloned().unwrap_or_else(|| json!(width)),
            "height": snap.get("height").cloned().unwrap_or_else(|| json!(height)),
            "message": format!("High-fidelity image successfully generated and saved to '{}'.", output_path_str)
        }))
    }
}

#[cfg(test)]
#[path = "image_generator_tests.rs"]
mod tests;
