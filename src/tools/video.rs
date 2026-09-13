use crate::tools::Tool;
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::fs;

use wavyte::{
    create_backend, render_to_mp4, BackendKind, Composition, FrameIndex, FrameRange,
    RenderSettings, RenderToMp4Opts,
};

pub struct VideoGeneratorTool;

#[async_trait::async_trait]
impl Tool for VideoGeneratorTool {
    fn name(&self) -> &str {
        "generate_video"
    }

    fn description(&self) -> &str {
        "Generate a simple video (MP4) from a programmatic timeline composition specified in JSON. Uses vector paths or text layers."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "composition_json": {
                    "type": "string",
                    "description": "The JSON string representation of the wavyte::Composition timeline."
                },
                "output_path": {
                    "type": "string",
                    "description": "The file path to save the generated MP4 video (defaults to 'output.mp4' in the current workspace)."
                },
                "bg_rgba": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "description": "Optional background color [r, g, b, a], defaults to [18, 20, 28, 255]."
                }
            },
            "required": ["composition_json"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let comp_json = arguments
            .get("composition_json")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'composition_json' parameter"))?;

        let output_path_str = arguments
            .get("output_path")
            .and_then(|v| v.as_str())
            .unwrap_or("output.mp4");
        let output_path = crate::config::resolve_path(output_path_str);
        crate::tools::resource_policy::ensure_artifact_write_allowed("generate_video")?;

        let bg_rgba = if let Some(arr) = arguments.get("bg_rgba").and_then(|v| v.as_array()) {
            if arr.len() == 4 {
                [
                    arr[0].as_u64().unwrap_or(18) as u8,
                    arr[1].as_u64().unwrap_or(20) as u8,
                    arr[2].as_u64().unwrap_or(28) as u8,
                    arr[3].as_u64().unwrap_or(255) as u8,
                ]
            } else {
                [18, 20, 28, 255]
            }
        } else {
            [18, 20, 28, 255]
        };

        // Deserialize composition from JSON
        let comp: Composition =
            serde_json::from_str(comp_json).context("Failed to deserialize composition JSON")?;

        comp.validate().context("Composition validation failed")?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Run rendering using wavyte
        let settings = RenderSettings {
            clear_rgba: Some(bg_rgba),
        };

        // Run blocking tasks in spawned threads since wavyte rendering is CPU heavy and synchronous
        let output_path_clone = output_path.clone();
        let render_future = tokio::task::spawn_blocking(move || -> Result<()> {
            let mut backend = create_backend(BackendKind::Cpu, &settings)
                .map_err(|e| anyhow!("Failed to create rendering backend: {}", e))?;

            let assets = wavyte::PreparedAssetStore::prepare(&comp, ".")
                .map_err(|e| anyhow!("Failed to prepare composition assets: {}", e))?;

            render_to_mp4(
                &comp,
                &output_path_clone,
                RenderToMp4Opts {
                    range: FrameRange::new(FrameIndex(0), comp.duration)
                        .map_err(|e| anyhow!("Failed to create frame range: {}", e))?,
                    bg_rgba,
                    overwrite: true,
                    threading: wavyte::RenderThreading::default(),
                },
                backend.as_mut(),
                &assets,
            )
            .map_err(|e| anyhow!("Failed to render composition to MP4: {}", e))?;

            Ok(())
        });

        match tokio::time::timeout(std::time::Duration::from_secs(120), render_future).await {
            Ok(Ok(res)) => res?,
            Ok(Err(join_err)) => return Err(anyhow!("Rendering task panicked: {}", join_err)),
            Err(_) => return Err(anyhow!("Rendering timed out after 120 seconds")),
        }

        Ok(json!({
            "status": "success",
            "output_path": output_path.to_string_lossy(),
            "message": format!("Video successfully generated and saved to '{}'.", output_path_str)
        }))
    }
}

#[cfg(test)]
#[path = "video_tests.rs"]
mod tests;

