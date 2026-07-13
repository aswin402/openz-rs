use crate::config::resolve_path;
use crate::tools::browser_common::{
    connect_to_tab, ensure_browser_running, kill_browser_on_port_9222, send_cdp_cmd,
};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use base64::prelude::*;
use serde_json::{json, Value};
use std::fs;
use std::time::Duration;
use tokio::time::sleep;

pub struct HtmlToVideoTool;

const DEFAULT_MAX_DIRECT_FRAMES: usize = 300;

#[derive(Debug, Clone, Copy)]
struct HtmlVideoRenderPlan {
    total_frames: usize,
    duration_seconds: f64,
    fps: i64,
    settle_ms: u64,
    load_delay_ms: i64,
}

impl HtmlVideoRenderPlan {
    fn new(duration_seconds: f64, fps: i64, settle_ms: u64, load_delay_ms: i64) -> Result<Self> {
        if duration_seconds <= 0.0 {
            return Err(anyhow!("duration_seconds must be greater than zero"));
        }
        if fps <= 0 {
            return Err(anyhow!("fps must be greater than zero"));
        }
        let total_frames = (duration_seconds * fps as f64).round() as usize;
        if total_frames == 0 {
            return Err(anyhow!(
                "Total frames cannot be zero. Adjust duration or FPS."
            ));
        }
        Ok(Self {
            total_frames,
            duration_seconds,
            fps,
            settle_ms,
            load_delay_ms,
        })
    }

    fn exceeds_default_direct_limit(&self) -> bool {
        self.total_frames > DEFAULT_MAX_DIRECT_FRAMES
    }

    fn minimum_settle_seconds(&self) -> f64 {
        (self.total_frames as f64 * self.settle_ms as f64 + self.load_delay_ms.max(0) as f64)
            / 1000.0
    }

    fn guidance(&self) -> String {
        format!(
            "html_to_video would capture {} frames ({:.1}s at {} FPS). Minimum settle/load wait is ~{:.1}s before screenshot and ffmpeg overhead. This likely exceeds the default tool timeout for long videos. Use render segments (for example 10s chunks), lower fps, use OpenMedia templates for long declarative videos, or set allow_long_render=true only when tool_timeout_secs is high enough.",
            self.total_frames,
            self.duration_seconds,
            self.fps,
            self.minimum_settle_seconds()
        )
    }
}

struct ServerGuard(Option<tokio::task::JoinHandle<()>>);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(ref handle) = self.0 {
            handle.abort();
        }
    }
}

#[async_trait::async_trait]
impl Tool for HtmlToVideoTool {
    fn name(&self) -> &str {
        "html_to_video"
    }

    fn description(&self) -> &str {
        "Render a video (MP4) from an HTML page frame-by-frame using headless Chrome. Useful for rendering high-fidelity animations, transitions, and HTML/CSS timelines (similar to Remotion)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "html_path": {
                    "type": "string",
                    "description": "Local path (e.g. 'index.html') or absolute URL to render."
                },
                "output_path": {
                    "type": "string",
                    "description": "File path to save the generated MP4 (defaults to 'output.mp4')."
                },
                "width": {
                    "type": "integer",
                    "description": "Viewport width in pixels (default: 1920)."
                },
                "height": {
                    "type": "integer",
                    "description": "Viewport height in pixels (default: 1080)."
                },
                "fps": {
                    "type": "integer",
                    "description": "Frames per second to render (default: 30)."
                },
                "duration_seconds": {
                    "type": "number",
                    "description": "Total video duration in seconds (default: 5.0)."
                },
                "tick_js": {
                    "type": "string",
                    "description": "JavaScript code to execute for each frame. The token '{frame}' will be replaced with the current frame index (0-indexed). E.g. 'if(window.seekToFrame) window.seekToFrame({frame});'"
                },
                "settle_ms": {
                    "type": "integer",
                    "description": "Delay in milliseconds to wait for frame to settle before capturing (default: 30)."
                },
                "load_delay_ms": {
                    "type": "integer",
                    "description": "Initial delay in milliseconds to wait for the page to load before capturing frames (default: 1500)."
                },
                "allow_long_render": {
                    "type": "boolean",
                    "description": "Set true to bypass the default direct-render frame guard. Use only when tool_timeout_secs is high enough; otherwise render in <=10s segments."
                }
            },
            "required": ["html_path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let html_path_str = arguments
            .get("html_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'html_path' parameter"))?;

        let output_path_str = arguments
            .get("output_path")
            .and_then(|v| v.as_str())
            .unwrap_or("output.mp4");
        let output_path = resolve_path(output_path_str);
        crate::tools::resource_policy::ensure_artifact_write_allowed("html_to_video")?;

        let width = arguments
            .get("width")
            .and_then(|v| v.as_i64())
            .unwrap_or(1920);
        let height = arguments
            .get("height")
            .and_then(|v| v.as_i64())
            .unwrap_or(1080);
        let fps = arguments.get("fps").and_then(|v| v.as_i64()).unwrap_or(30);
        let duration_secs = arguments
            .get("duration_seconds")
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0);
        let tick_js = arguments.get("tick_js").and_then(|v| v.as_str());
        let settle_ms = arguments
            .get("settle_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(30)
            .max(0) as u64;
        let load_delay_ms = arguments
            .get("load_delay_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(1500);

        let render_plan = HtmlVideoRenderPlan::new(duration_secs, fps, settle_ms, load_delay_ms)?;
        let allow_long_render = arguments
            .get("allow_long_render")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if render_plan.exceeds_default_direct_limit() && !allow_long_render {
            return Err(anyhow!(render_plan.guidance()));
        }
        let total_frames = render_plan.total_frames;

        let mut _server_guard = ServerGuard(None);
        let target_url =
            if html_path_str.starts_with("http://") || html_path_str.starts_with("https://") {
                html_path_str.to_string()
            } else {
                let mut raw_path = if let Some(stripped) = html_path_str.strip_prefix("file://") {
                    std::path::PathBuf::from(stripped)
                } else {
                    resolve_path(html_path_str)
                };
                if !raw_path.is_absolute() {
                    if let Ok(cwd) = std::env::current_dir() {
                        raw_path = cwd.join(raw_path);
                    }
                }

                let parent_dir = raw_path.parent().unwrap_or(&raw_path).to_path_buf();
                let filename = raw_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("index.html")
                    .to_string();

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
                            format!("file://{}", raw_path.to_string_lossy())
                        }
                    }
                    Err(_) => {
                        format!("file://{}", raw_path.to_string_lossy())
                    }
                }
            };

        let uuid = uuid::Uuid::new_v4().to_string();
        let temp_frames_dir = std::env::temp_dir().join(format!("openz_html_video_{}", uuid));
        fs::create_dir_all(&temp_frames_dir)?;

        ensure_browser_running().await?;

        let client = reqwest::Client::new();
        let mut res = client.put("http://127.0.0.1:9222/json/new").send().await;

        if !matches!(&res, Ok(r) if r.status().is_success()) {
            res = client.get("http://127.0.0.1:9222/json/new").send().await;
        }

        if !matches!(&res, Ok(r) if r.status().is_success()) {
            kill_browser_on_port_9222();
            sleep(Duration::from_millis(500)).await;
            ensure_browser_running().await?;
            res = client.put("http://127.0.0.1:9222/json/new").send().await;
            if !matches!(&res, Ok(r) if r.status().is_success()) {
                res = client.get("http://127.0.0.1:9222/json/new").send().await;
            }
        }

        let tab_info: Value = res?.json().await?;
        let web_socket_debugger_url = tab_info
            .get("webSocketDebuggerUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("No webSocketDebuggerUrl returned from browser tab"))?;

        let (mut write, mut read) = connect_to_tab(web_socket_debugger_url).await?;
        let mut message_id = 0u64;

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

        let _ = send_cdp_cmd(
            &mut write,
            &mut read,
            &mut message_id,
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": width,
                "height": height,
                "deviceScaleFactor": 1,
                "mobile": false
            }),
        )
        .await?;

        let _ = send_cdp_cmd(
            &mut write,
            &mut read,
            &mut message_id,
            "Page.navigate",
            json!({
                "url": target_url
            }),
        )
        .await?;

        sleep(Duration::from_millis(load_delay_ms as u64)).await;

        for frame in 0..total_frames {
            if let Some(js) = tick_js {
                let js_injected = js.replace("{frame}", &frame.to_string());
                let _ = send_cdp_cmd(
                    &mut write,
                    &mut read,
                    &mut message_id,
                    "Runtime.evaluate",
                    json!({
                        "expression": js_injected,
                        "returnByValue": true
                    }),
                )
                .await?;
            }

            if settle_ms > 0 {
                sleep(Duration::from_millis(settle_ms)).await;
            }

            let screenshot_res = send_cdp_cmd(
                &mut write,
                &mut read,
                &mut message_id,
                "Page.captureScreenshot",
                json!({
                    "format": "png"
                }),
            )
            .await?;

            let base64_data = screenshot_res
                .pointer("/result/data")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Failed to capture screenshot data for frame {}", frame))?;

            let image_bytes = BASE64_STANDARD.decode(base64_data)?;
            let frame_file = temp_frames_dir.join(format!("frame_{:05}.png", frame));
            fs::write(&frame_file, image_bytes)?;
        }

        let target_id = tab_info.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let _ = client
            .get(format!("http://127.0.0.1:9222/json/close/{}", target_id))
            .send()
            .await;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut ffmpeg_cmd = tokio::process::Command::new("ffmpeg");
        ffmpeg_cmd
            .arg("-y")
            .arg("-framerate")
            .arg(fps.to_string())
            .arg("-i")
            .arg(temp_frames_dir.join("frame_%05d.png"))
            .arg("-c:v")
            .arg("libx264")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg(&output_path);

        ffmpeg_cmd.kill_on_drop(true);
        let ffmpeg_output = ffmpeg_cmd.output().await?;

        let _ = fs::remove_dir_all(&temp_frames_dir);

        if !ffmpeg_output.status.success() {
            let stderr = String::from_utf8_lossy(&ffmpeg_output.stderr);
            return Err(anyhow!("ffmpeg failed to render video: {}", stderr));
        }

        Ok(json!({
            "status": "success",
            "output_path": output_path.to_string_lossy(),
            "total_frames": total_frames,
            "message": format!("Video successfully generated from HTML template at {} FPS ({} frames) and saved to '{}'.", fps, total_frames, output_path_str)
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
                if let Ok(new_val) =
                    axum::http::header::HeaderValue::from_str("text/html; charset=utf-8")
                {
                    response
                        .headers_mut()
                        .insert(axum::http::header::CONTENT_TYPE, new_val);
                }
            }
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_plan_flags_30_second_30fps_as_too_large_for_direct_default_render() {
        let plan = HtmlVideoRenderPlan::new(30.0, 30, 30, 1500).unwrap();
        assert_eq!(plan.total_frames, 900);
        assert!(plan.exceeds_default_direct_limit());
        assert!(plan.guidance().contains("render segments"));
        assert!(plan.guidance().contains("900 frames"));
    }

    #[test]
    fn render_plan_allows_10_second_30fps_direct_render() {
        let plan = HtmlVideoRenderPlan::new(10.0, 30, 30, 1500).unwrap();
        assert_eq!(plan.total_frames, 300);
        assert!(!plan.exceeds_default_direct_limit());
    }
}
