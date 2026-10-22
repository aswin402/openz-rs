use super::error::{OpenMediaError, Result};
use super::progress::ProgressReporter;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Information about a single model file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Unique model identifier (e.g., "clip-vit-b32-text")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Model category
    pub category: ModelCategory,
    /// File path on disk
    pub path: PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
    /// Model format
    pub format: ModelFormat,
    /// Quantization level (if applicable)
    pub quantization: Option<String>,
    /// SHA-256 checksum
    pub checksum: Option<String>,
    /// Whether the model is verified (checksum matches)
    pub verified: bool,
    /// Minimum VRAM required (bytes), 0 for CPU-only
    pub min_vram: u64,
    /// Supported resolutions
    pub supported_resolutions: Vec<(u32, u32)>,
    /// Default resolution
    pub default_resolution: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelCategory {
    Diffusion,
    Upscale,
    Segmentation,
    Clip,
    Aesthetic,
    Vae,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelFormat {
    Gguf,
    Onnx,
    SafeTensors,
    Bin,
}

/// Registry of all available models on disk
pub struct ModelRegistry {
    models: Vec<ModelInfo>,
    model_dir: PathBuf,
}

impl ModelRegistry {
    /// Get the scanned model directory path
    pub fn model_dir(&self) -> &PathBuf {
        &self.model_dir
    }

    /// Scan model directory and build registry
    pub async fn scan(model_dir: &Path) -> Result<Self> {
        let mut models = vec![
            Self::onnx_model_info(
                "clip-vit-b32-text",
                "CLIP ViT-B/32 Text Encoder",
                ModelCategory::Clip,
                model_dir.join("clip/text_model.onnx"),
                350_000_000,
            ),
            Self::onnx_model_info(
                "clip-vit-b32-vision",
                "CLIP ViT-B/32 Vision Encoder",
                ModelCategory::Clip,
                model_dir.join("clip/vision_model.onnx"),
                350_000_000,
            ),
            Self::onnx_model_info(
                "aesthetic-predictor",
                "LAION Aesthetic Predictor",
                ModelCategory::Aesthetic,
                model_dir.join("clip/aesthetic-predictor.onnx"),
                25_000_000,
            ),
        ];

        // Update verified status based on file existence
        for model in &mut models {
            model.verified = model.path.exists();
        }

        Ok(Self {
            models,
            model_dir: model_dir.to_path_buf(),
        })
    }

    fn onnx_model_info(
        id: &str,
        name: &str,
        category: ModelCategory,
        path: PathBuf,
        size_bytes: u64,
    ) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            name: name.to_string(),
            category,
            path,
            size_bytes,
            format: ModelFormat::Onnx,
            quantization: None,
            checksum: None,
            verified: true,
            min_vram: 0,
            supported_resolutions: vec![],
            default_resolution: (0, 0),
        }
    }

    /// Get a model by ID
    pub fn get(&self, id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == id)
    }

    /// List all models, optionally filtered by category
    pub fn list(&self, category: Option<ModelCategory>) -> Vec<&ModelInfo> {
        match category {
            Some(cat) => self.models.iter().filter(|m| m.category == cat).collect(),
            None => self.models.iter().collect(),
        }
    }

    /// Download a model by its ID
    pub async fn download_model(
        &self,
        id: &str,
        progress: &dyn ProgressReporter,
    ) -> Result<PathBuf> {
        let model = self
            .get(id)
            .ok_or_else(|| OpenMediaError::ModelNotFound(id.to_string()))?;

        let url = match id {
            "clip-vit-b32-text" => "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model.onnx",
            "clip-vit-b32-vision" => "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model.onnx",
            "aesthetic-predictor" => "https://huggingface.co/greentext/aesthetic-predictor/resolve/main/aesthetic-predictor.onnx",
            _ => return Err(OpenMediaError::ModelNotFound(id.to_string())),
        };

        let dest_path = &model.path;
        match dest_path.parent() {
            Some(parent) => std::fs::create_dir_all(parent)?,
            None => {}
        }

        let tmp_path = dest_path.with_extension("download");

        let client = crate::core::http::default_http_client();
        let mut response =
            client
                .get(url)
                .send()
                .await
                .map_err(|e| OpenMediaError::ModelLoadFailed {
                    model: id.to_string(),
                    reason: e.to_string(),
                })?;

        match response.status().is_success() {
            true => {}
            false => {
                return Err(OpenMediaError::ModelLoadFailed {
                    model: id.to_string(),
                    reason: format!("Server returned error code: {}", response.status()),
                });
            }
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut file = std::fs::File::create(&tmp_path)?;

        while let Some(chunk) =
            response
                .chunk()
                .await
                .map_err(|e| OpenMediaError::ModelLoadFailed {
                    model: id.to_string(),
                    reason: e.to_string(),
                })?
        {
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            progress.report(
                downloaded,
                total_size,
                &format!("Downloading {}...", model.name),
            );
        }

        file.flush()?;
        drop(file);

        // Atomically rename downloaded file to target model file
        std::fs::rename(&tmp_path, dest_path)?;
        progress.complete(&format!("Successfully downloaded {}", model.name));

        Ok(dest_path.clone())
    }
}
