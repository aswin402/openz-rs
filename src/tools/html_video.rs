use crate::config::resolve_path;
use crate::tools::browser_common::{
    browser_cdp_port, connect_to_tab, ensure_browser_running, kill_browser_on_cdp_port,
    obtain_tab_websocket_url, send_cdp_cmd,
};
use crate::tools::image_generator::{serve_local_html_file, LocalHtmlServerGuard};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use base64::prelude::*;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
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
        match duration_seconds > 0.0 {
            false => return Err(anyhow!("duration_seconds must be greater than zero")),
            true => {}
        }
        match fps > 0 {
            false => return Err(anyhow!("fps must be greater than zero")),
            true => {}
        }
        let total_frames = (duration_seconds * fps as f64).round() as usize;
        match total_frames > 0 {
            false => Err(anyhow!(
                "Total frames cannot be zero. Adjust duration or FPS."
            )),
            true => Ok(Self {
                total_frames,
                duration_seconds,
                fps,
                settle_ms,
                load_delay_ms,
            }),
        }
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

async fn resolve_video_target_url(html_path_str: &str) -> (String, LocalHtmlServerGuard) {
    let is_remote = html_path_str.starts_with("http://") || html_path_str.starts_with("https://");
    match is_remote {
        true => (html_path_str.to_string(), LocalHtmlServerGuard(None)),
        false => {
            let raw_path = match html_path_str.strip_prefix("file://") {
                Some(stripped) => PathBuf::from(stripped),
                None => resolve_path(html_path_str),
            };
            serve_local_html_file(raw_path).await
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
        match render_plan.exceeds_default_direct_limit() && !allow_long_render {
            true => return Err(anyhow!(render_plan.guidance())),
            false => {}
        }
        let total_frames = render_plan.total_frames;

        let (target_url, _server_guard) = resolve_video_target_url(html_path_str).await;

        let uuid = uuid::Uuid::new_v4().to_string();
        let temp_frames_dir = std::env::temp_dir().join(format!("openz_html_video_{}", uuid));
        fs::create_dir_all(&temp_frames_dir)?;

        ensure_browser_running().await?;

        let cdp_port = browser_cdp_port();
        let (tab_id, web_socket_debugger_url) = match obtain_tab_websocket_url(cdp_port).await {
            Ok(pair) => pair,
            Err(_) => {
                kill_browser_on_cdp_port();
                sleep(Duration::from_millis(500)).await;
                ensure_browser_running().await?;
                obtain_tab_websocket_url(cdp_port).await?
            }
        };

        let (mut write, mut read) = connect_to_tab(&web_socket_debugger_url).await?;
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
            match tick_js {
                Some(js) => {
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
                None => {}
            }

            match settle_ms > 0 {
                true => sleep(Duration::from_millis(settle_ms)).await,
                false => {}
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

        let client = crate::core::http::default_http_client();
        let _ = client
            .get(format!("http://127.0.0.1:{cdp_port}/json/close/{}", tab_id))
            .send()
            .await;

        match output_path.parent() {
            Some(parent) => fs::create_dir_all(parent)?,
            None => {}
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

        match ffmpeg_output.status.success() {
            false => {
                let stderr = String::from_utf8_lossy(&ffmpeg_output.stderr);
                Err(anyhow!("ffmpeg failed to render video: {}", stderr))
            }
            true => Ok(json!({
                "status": "success",
                "output_path": output_path.to_string_lossy(),
                "total_frames": total_frames,
                "message": format!("Video successfully generated from HTML template at {} FPS ({} frames) and saved to '{}'.", fps, total_frames, output_path_str)
            })),
        }
    }
}

#[cfg(test)]
#[path = "html_video_tests.rs"]
mod tests;

