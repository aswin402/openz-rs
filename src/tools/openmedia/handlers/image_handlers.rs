//! Image filtering, conversion, and batch-processing handlers.

use crate::tools::openmedia::core::ImageOutput;
use crate::tools::openmedia::process::{
    apply_cpu_operation, apply_gpu_operation, batch_process_files, write_image_with_format,
    FilterChain, ProcessOperation, ResizeMethod,
};
use crate::tools::openmedia::server::*;

impl OpenMediaServer {
    async fn process_and_save(
        &self,
        img: &image::DynamicImage,
        op: &ProcessOperation,
        format: &str,
    ) -> Result<ImageOutput, String> {
        let start = std::time::Instant::now();
        let mut backend_used = "wgpu".to_string();

        let processed = match apply_gpu_operation(img, op) {
            Ok(gpu_img) => gpu_img,
            Err(_) => {
                backend_used = "cpu".to_string();
                apply_cpu_operation(img, op).map_err(|e| e.to_string())?
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

        let bytes = write_image_with_format(&processed, ext, 80)
            .map_err(|e| e.to_string())?;

        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
        let file_size = bytes.len() as u64;

        Ok(ImageOutput {
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

    pub async fn image_apply_filter(
        &self,
        req: ImageApplyFilterRequest,
    ) -> Result<serde_json::Value, String> {
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let op = match req.filter_type.to_lowercase().as_str() {
            "grayscale" => ProcessOperation::Grayscale,
            "invert" => ProcessOperation::Invert,
            "brightness" => ProcessOperation::Brightness {
                value: req.parameter.unwrap_or(0.0) as i32,
            },
            "contrast" => ProcessOperation::Contrast {
                value: req.parameter.unwrap_or(0.0) as i32,
            },
            "saturation" => ProcessOperation::Saturation {
                value: req.parameter.unwrap_or(0.0) as i32,
            },
            "hue_rotate" | "huerotate" => ProcessOperation::HueRotate {
                degrees: req.parameter.unwrap_or(0.0),
            },
            "sepia" => ProcessOperation::Sepia {
                intensity: req.parameter.unwrap_or(1.0),
            },
            "threshold" => ProcessOperation::Threshold {
                value: req.parameter.unwrap_or(128.0) as u8,
            },
            "blur" | "gaussian_blur" => ProcessOperation::GaussianBlur {
                radius: req.parameter.unwrap_or(2.0),
                sigma: None,
            },
            "box_blur" => ProcessOperation::BoxBlur {
                radius: req.parameter.unwrap_or(2.0) as u32,
            },
            "sharpen" => ProcessOperation::Sharpen {
                amount: 1.0,
                radius: req.parameter.unwrap_or(2.0),
                threshold: 0,
            },
            "unsharp_mask" | "unsharp" => ProcessOperation::UnsharpMask {
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
        serde_json::to_value(output).map_err(|e| e.to_string())
    }

    pub async fn image_resize(
        &self,
        req: ImageResizeRequest,
    ) -> Result<serde_json::Value, String> {
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let method = match req
            .algorithm
            .as_deref()
            .unwrap_or("bilinear")
            .to_lowercase()
            .as_str()
        {
            "nearest" => ResizeMethod::Nearest,
            "bilinear" => ResizeMethod::Bilinear,
            "lanczos3" => ResizeMethod::Lanczos3,
            _ => ResizeMethod::Bilinear,
        };
        let op = ProcessOperation::Resize {
            width: req.width,
            height: req.height,
            method,
        };
        let ext = std::path::Path::new(&req.image_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let output = self.process_and_save(&img, &op, ext).await?;
        serde_json::to_value(output).map_err(|e| e.to_string())
    }

    pub async fn image_crop(
        &self,
        req: ImageCropRequest,
    ) -> Result<serde_json::Value, String> {
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let op = ProcessOperation::Crop {
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
        serde_json::to_value(output).map_err(|e| e.to_string())
    }

    pub async fn image_transform(
        &self,
        req: ImageTransformRequest,
    ) -> Result<serde_json::Value, String> {
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let op = match req.transform_type.to_lowercase().as_str() {
            "rotate" => ProcessOperation::Rotate {
                angle: req.angle.unwrap_or(90.0),
                expand: true,
            },
            "flip_horizontal" | "fliph" => ProcessOperation::FlipHorizontal,
            "flip_vertical" | "flipv" => ProcessOperation::FlipVertical,
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
        serde_json::to_value(output).map_err(|e| e.to_string())
    }

    pub async fn image_convert(
        &self,
        req: ImageConvertRequest,
    ) -> Result<serde_json::Value, String> {
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let start = std::time::Instant::now();

        let ext = req.format.trim_start_matches('.').to_lowercase();
        let filename = format!("{}.{}", uuid::Uuid::now_v7(), ext);
        let dest = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let bytes = write_image_with_format(
            &img,
            &ext,
            req.quality.unwrap_or(80),
        )
        .map_err(|e| e.to_string())?;

        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
        let file_size = bytes.len() as u64;

        let output = ImageOutput {
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

        serde_json::to_value(output).map_err(|e| e.to_string())
    }

    pub async fn image_batch_process(
        &self,
        req: ImageBatchProcessRequest,
    ) -> Result<serde_json::Value, String> {
        let mut chain = FilterChain::new();
        for op_val in req.operations {
            let normalized_op_val = normalize_process_operation_value(&op_val);
            let op = serde_json::from_value::<ProcessOperation>(normalized_op_val)
                .map_err(|e| format!("Invalid process operation definition: {}", e))?;
            chain.add(op);
        }
        let output_dir = std::path::Path::new(&req.output_dir);
        let processed_paths =
            batch_process_files(&req.glob_pattern, &chain, output_dir)
                .await
                .map_err(|e| e.to_string())?;

        let outputs: Vec<ImageOutput> = processed_paths
            .into_iter()
            .map(|path| {
                let (w, h) = image::image_dimensions(&path).unwrap_or((0, 0));
                let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png")
                    .to_string();
                ImageOutput {
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

        serde_json::to_value(outputs).map_err(|e| e.to_string())
    }
}

pub fn normalize_process_operation_value(val: &serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::String(s) => match s.to_lowercase().as_str() {
            "invert" => serde_json::Value::String("Invert".to_string()),
            "grayscale" => serde_json::Value::String("Grayscale".to_string()),
            "fliph" | "flip_horizontal" | "fliphorizontal" => {
                serde_json::Value::String("FlipHorizontal".to_string())
            }
            "flipv" | "flip_vertical" | "flipvertical" => {
                serde_json::Value::String("FlipVertical".to_string())
            }
            _ => val.clone(),
        },
        serde_json::Value::Object(map) => {
            let op_name = map
                .get("operation")
                .or_else(|| map.get("type"))
                .or_else(|| map.get("op"))
                .and_then(|v| v.as_str());

            if let Some(name) = op_name {
                match name.to_lowercase().as_str() {
                    "invert" => serde_json::Value::String("Invert".to_string()),
                    "grayscale" => serde_json::Value::String("Grayscale".to_string()),
                    "fliph" | "flip_horizontal" | "fliphorizontal" => {
                        serde_json::Value::String("FlipHorizontal".to_string())
                    }
                    "flipv" | "flip_vertical" | "flipvertical" => {
                        serde_json::Value::String("FlipVertical".to_string())
                    }
                    "resize" => {
                        let mut inner = map.clone();
                        inner.remove("operation");
                        inner.remove("type");
                        inner.remove("op");
                        if !inner.contains_key("method") {
                            inner.insert("method".to_string(), serde_json::json!("lanczos3"));
                        }
                        serde_json::json!({ "Resize": inner })
                    }
                    "blur" | "gaussian_blur" | "gaussianblur" => {
                        let mut inner = map.clone();
                        inner.remove("operation");
                        inner.remove("type");
                        inner.remove("op");
                        serde_json::json!({ "GaussianBlur": inner })
                    }
                    "box_blur" | "boxblur" => {
                        let mut inner = map.clone();
                        inner.remove("operation");
                        inner.remove("type");
                        inner.remove("op");
                        serde_json::json!({ "BoxBlur": inner })
                    }
                    "sharpen" => {
                        let mut inner = map.clone();
                        inner.remove("operation");
                        inner.remove("type");
                        inner.remove("op");
                        serde_json::json!({ "Sharpen": inner })
                    }
                    "crop" => {
                        let mut inner = map.clone();
                        inner.remove("operation");
                        inner.remove("type");
                        inner.remove("op");
                        serde_json::json!({ "Crop": inner })
                    }
                    "rotate" => {
                        let mut inner = map.clone();
                        inner.remove("operation");
                        inner.remove("type");
                        inner.remove("op");
                        serde_json::json!({ "Rotate": inner })
                    }
                    _ => val.clone(),
                }
            } else {
                val.clone()
            }
        }
        _ => val.clone(),
    }
}
