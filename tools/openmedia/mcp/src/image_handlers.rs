//! Image filtering, conversion, and batch-processing handlers.

use super::{
    ImageApplyFilterRequest, ImageBatchProcessRequest, ImageConvertRequest, ImageCropRequest,
    ImageResizeRequest, ImageTransformRequest, Json, McpObject, OpenMediaServer, Parameters,
};
use rmcp::handler::server::tool::ToolRouter;

pub(crate) fn router() -> ToolRouter<OpenMediaServer> {
    OpenMediaServer::image_router()
}

#[rmcp::tool_router(router = image_router)]
impl OpenMediaServer {
    async fn process_and_save(
        &self,
        img: &image::DynamicImage,
        op: &openmedia_process::ProcessOperation,
        format: &str,
    ) -> Result<openmedia_core::ImageOutput, String> {
        let start = std::time::Instant::now();
        let mut backend_used = "wgpu".to_string();

        let processed = match openmedia_process::apply_gpu_operation(img, op) {
            Ok(gpu_img) => gpu_img,
            Err(_) => {
                backend_used = "cpu".to_string();
                openmedia_process::apply_cpu_operation(img, op).map_err(|e| e.to_string())?
            }
        };

        let ext = match format.to_lowercase().as_str() {
            "png" => "png",
            "jpeg" | "jpg" => "jpg",
            "webp" => "webp",
            "avif" => "avif",
            _ => "png",
        };

        let filename = format!("{}.{}", uuid::Uuid::now_v7(), ext);
        let dest = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let bytes = openmedia_process::write_image_with_format(&processed, ext, 80)
            .map_err(|e| e.to_string())?;

        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
        let file_size = bytes.len() as u64;

        Ok(openmedia_core::ImageOutput {
            path: dest,
            width: processed.width(),
            height: processed.height(),
            seed: 0,
            format: ext.to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "none".to_string(),
            backend_used,
            generation_time: start.elapsed().as_secs_f64(),
        })
    }

    #[rmcp::tool(
        name = "image_apply_filter",
        description = "Apply a visual filter to an image (grayscale, invert, brightness, contrast, saturation, hue_rotate, sepia, threshold, blur, sharpen, unsharp_mask)."
    )]
    pub async fn image_apply_filter(
        &self,
        params: Parameters<ImageApplyFilterRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let op = match req.filter_type.to_lowercase().as_str() {
            "grayscale" => openmedia_process::ProcessOperation::Grayscale,
            "invert" => openmedia_process::ProcessOperation::Invert,
            "brightness" => openmedia_process::ProcessOperation::Brightness {
                value: req.parameter.unwrap_or(0.0) as i32,
            },
            "contrast" => openmedia_process::ProcessOperation::Contrast {
                value: req.parameter.unwrap_or(0.0) as i32,
            },
            "saturation" => openmedia_process::ProcessOperation::Saturation {
                value: req.parameter.unwrap_or(0.0) as i32,
            },
            "hue_rotate" | "huerotate" => openmedia_process::ProcessOperation::HueRotate {
                degrees: req.parameter.unwrap_or(0.0),
            },
            "sepia" => openmedia_process::ProcessOperation::Sepia {
                intensity: req.parameter.unwrap_or(1.0),
            },
            "threshold" => openmedia_process::ProcessOperation::Threshold {
                value: req.parameter.unwrap_or(128.0) as u8,
            },
            "blur" | "gaussian_blur" => openmedia_process::ProcessOperation::GaussianBlur {
                radius: req.parameter.unwrap_or(2.0),
                sigma: None,
            },
            "box_blur" => openmedia_process::ProcessOperation::BoxBlur {
                radius: req.parameter.unwrap_or(2.0) as u32,
            },
            "sharpen" => openmedia_process::ProcessOperation::Sharpen {
                amount: 1.0,
                radius: req.parameter.unwrap_or(2.0),
                threshold: 0,
            },
            "unsharp_mask" | "unsharp" => openmedia_process::ProcessOperation::UnsharpMask {
                amount: 1.0,
                radius: req.parameter.unwrap_or(2.0),
                threshold: 0,
            },
            _ => return Err(format!("Unsupported filter type: {}", req.filter_type)),
        };
        let ext = std::path::Path::new(&req.image_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let output = self.process_and_save(&img, &op, ext).await?;
        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "image_resize",
        description = "Resize an image to specific dimensions with configurable algorithm."
    )]
    pub async fn image_resize(
        &self,
        params: Parameters<ImageResizeRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let method = match req
            .algorithm
            .as_deref()
            .unwrap_or("bilinear")
            .to_lowercase()
            .as_str()
        {
            "nearest" => openmedia_process::ResizeMethod::Nearest,
            "bilinear" => openmedia_process::ResizeMethod::Bilinear,
            "lanczos3" => openmedia_process::ResizeMethod::Lanczos3,
            _ => openmedia_process::ResizeMethod::Bilinear,
        };
        let op = openmedia_process::ProcessOperation::Resize {
            width: req.width,
            height: req.height,
            method,
        };
        let ext = std::path::Path::new(&req.image_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let output = self.process_and_save(&img, &op, ext).await?;
        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "image_crop",
        description = "Crop an image using top-left coordinates and dimensions."
    )]
    pub async fn image_crop(
        &self,
        params: Parameters<ImageCropRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let op = openmedia_process::ProcessOperation::Crop {
            x: req.x,
            y: req.y,
            width: req.width,
            height: req.height,
        };
        let ext = std::path::Path::new(&req.image_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let output = self.process_and_save(&img, &op, ext).await?;
        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "image_transform",
        description = "Apply geometric transform like rotate or flip."
    )]
    pub async fn image_transform(
        &self,
        params: Parameters<ImageTransformRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let op = match req.transform_type.to_lowercase().as_str() {
            "rotate" => openmedia_process::ProcessOperation::Rotate {
                angle: req.angle.unwrap_or(90.0),
                expand: true,
            },
            "flip_horizontal" | "fliph" => openmedia_process::ProcessOperation::FlipHorizontal,
            "flip_vertical" | "flipv" => openmedia_process::ProcessOperation::FlipVertical,
            _ => {
                return Err(format!(
                    "Unsupported transform type: {}",
                    req.transform_type
                ))
            }
        };
        let ext = std::path::Path::new(&req.image_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let output = self.process_and_save(&img, &op, ext).await?;
        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "image_convert",
        description = "Convert an image to another format with quality settings."
    )]
    pub async fn image_convert(
        &self,
        params: Parameters<ImageConvertRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let start = std::time::Instant::now();

        let ext = req.format.trim_start_matches('.').to_lowercase();
        let filename = format!("{}.{}", uuid::Uuid::now_v7(), ext);
        let dest = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let bytes = openmedia_process::write_image_with_format(
            &img,
            &ext,
            req.quality.unwrap_or(80),
        )
        .map_err(|e| e.to_string())?;

        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
        let file_size = bytes.len() as u64;

        let output = openmedia_core::ImageOutput {
            path: dest,
            width: img.width(),
            height: img.height(),
            seed: 0,
            format: ext,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "none".to_string(),
            backend_used: "cpu".to_string(),
            generation_time: start.elapsed().as_secs_f64(),
        };

        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "image_batch_process",
        description = "Process a set of files using glob pattern and a sequential filter chain."
    )]
    pub async fn image_batch_process(
        &self,
        params: Parameters<ImageBatchProcessRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let mut chain = openmedia_process::FilterChain::new();
        for op_val in req.operations {
            let op = serde_json::from_value::<openmedia_process::ProcessOperation>(op_val)
                .map_err(|e| format!("Invalid process operation definition: {}", e))?;
            chain.add(op);
        }
        let output_dir = std::path::Path::new(&req.output_dir);
        let processed_paths =
            openmedia_process::batch_process_files(&req.glob_pattern, &chain, output_dir)
                .await
                .map_err(|e| e.to_string())?;

        let outputs: Vec<openmedia_core::ImageOutput> = processed_paths
            .into_iter()
            .map(|path| {
                let (w, h) = image::image_dimensions(&path).unwrap_or((0, 0));
                let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png")
                    .to_string();
                openmedia_core::ImageOutput {
                    path,
                    width: w,
                    height: h,
                    seed: 0,
                    format: ext,
                    file_size,
                    generation_id: uuid::Uuid::now_v7().to_string(),
                    clip_score: None,
                    aesthetic_score: None,
                    model_used: "none".to_string(),
                    backend_used: "cpu/wgpu".to_string(),
                    generation_time: 0.0,
                }
            })
            .collect();

        serde_json::to_value(outputs)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }
}
