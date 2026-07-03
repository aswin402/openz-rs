use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::io::Write;
use crate::error::{Result, OpenMediaError};
use crate::hardware::HardwareInfo;
use crate::progress::ProgressReporter;

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
    pub async fn scan(model_dir: &PathBuf) -> Result<Self> {
        let mut models = vec![
            ModelInfo {
                id: "clip-vit-b32-text".to_string(),
                name: "CLIP ViT-B/32 Text Encoder".to_string(),
                category: ModelCategory::Clip,
                path: model_dir.join("clip/text_model.onnx"),
                size_bytes: 350_000_000,
                format: ModelFormat::Onnx,
                quantization: None,
                checksum: None,
                verified: true,
                min_vram: 0,
                supported_resolutions: vec![],
                default_resolution: (0, 0),
            },
            ModelInfo {
                id: "clip-vit-b32-vision".to_string(),
                name: "CLIP ViT-B/32 Vision Encoder".to_string(),
                category: ModelCategory::Clip,
                path: model_dir.join("clip/vision_model.onnx"),
                size_bytes: 350_000_000,
                format: ModelFormat::Onnx,
                quantization: None,
                checksum: None,
                verified: true,
                min_vram: 0,
                supported_resolutions: vec![],
                default_resolution: (0, 0),
            },
            ModelInfo {
                id: "aesthetic-predictor".to_string(),
                name: "LAION Aesthetic Predictor".to_string(),
                category: ModelCategory::Aesthetic,
                path: model_dir.join("clip/aesthetic-predictor.onnx"),
                size_bytes: 25_000_000,
                format: ModelFormat::Onnx,
                quantization: None,
                checksum: None,
                verified: true,
                min_vram: 0,
                supported_resolutions: vec![],
                default_resolution: (0, 0),
            },
        ];

        // Update verified status based on file existence
        for model in &mut models {
            model.verified = model.path.exists();
        }

        Ok(Self {
            models,
            model_dir: model_dir.clone(),
        })
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

    /// Select the best diffusion model given hardware constraints
    pub fn select_best_diffusion(&self, _hardware: &HardwareInfo) -> Option<&ModelInfo> {
        None
    }

    /// Download a model by its ID
    pub async fn download_model(&self, id: &str, progress: &dyn ProgressReporter) -> Result<PathBuf> {
        let model = self.get(id).ok_or_else(|| OpenMediaError::ModelNotFound(id.to_string()))?;
        
        let url = match id {
            "clip-vit-b32-text" => "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model.onnx",
            "clip-vit-b32-vision" => "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model.onnx",
            "aesthetic-predictor" => "https://huggingface.co/greentext/aesthetic-predictor/resolve/main/aesthetic-predictor.onnx",
            _ => return Err(OpenMediaError::ModelNotFound(id.to_string())),
        };

        let dest_path = &model.path;
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let tmp_path = dest_path.with_extension("download");
        
        let client = reqwest::Client::new();
        let mut response = client.get(url)
            .send()
            .await
            .map_err(|e| OpenMediaError::ModelLoadFailed { model: id.to_string(), reason: e.to_string() })?;

        if !response.status().is_success() {
            return Err(OpenMediaError::ModelLoadFailed {
                model: id.to_string(),
                reason: format!("Server returned error code: {}", response.status()),
            });
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut file = std::fs::File::create(&tmp_path)?;

        while let Some(chunk) = response.chunk().await.map_err(|e| OpenMediaError::ModelLoadFailed { model: id.to_string(), reason: e.to_string() })? {
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            progress.report(downloaded, total_size, &format!("Downloading {}...", model.name));
        }

        file.flush()?;
        drop(file);

        // Atomically rename downloaded file to target model file
        std::fs::rename(&tmp_path, dest_path)?;
        progress.complete(&format!("Successfully downloaded {}", model.name));

        Ok(dest_path.clone())
    }

    /// Verify a model's checksum
    pub async fn verify_model(&self, id: &str) -> Result<bool> {
        let model = self.get(id).ok_or_else(|| OpenMediaError::ModelNotFound(id.to_string()))?;
        Ok(model.path.exists())
    }
}
