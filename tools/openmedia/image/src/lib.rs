use std::sync::Arc;
use serde::{Deserialize, Serialize};
use openmedia_core::{Result, ImageOutput, ModelInfo, ProgressReporter};

/// Parameters for text-to-image generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Txt2ImgRequest {
    pub prompt: String,
    pub negative_prompt: String,
    pub model: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub cfg_scale: f32,
    pub seed: Option<u64>,
    pub scheduler: SchedulerType,
    pub batch_size: u32,
    pub output_format: OutputFormat,
    pub output_quality: u8,
    pub clip_skip: u32,
    pub auto_refine: bool,
    pub max_refine_rounds: u32,
}

/// Parameters for image-to-image transformation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Img2ImgRequest {
    pub input_image: ImageInput,
    pub prompt: String,
    pub negative_prompt: String,
    pub strength: f32,
    pub model: String,
    pub steps: u32,
    pub cfg_scale: f32,
    pub seed: Option<u64>,
    pub scheduler: SchedulerType,
    pub output_format: OutputFormat,
    pub output_quality: u8,
}

/// Parameters for inpainting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InpaintRequest {
    pub input_image: ImageInput,
    pub mask_image: ImageInput,
    pub prompt: String,
    pub negative_prompt: String,
    pub mask_blur: u32,
    pub inpaint_full: bool,
    pub model: String,
    pub steps: u32,
    pub cfg_scale: f32,
    pub seed: Option<u64>,
    pub scheduler: SchedulerType,
    pub output_format: OutputFormat,
    pub output_quality: u8,
}

/// Input image source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageInput {
    /// File path on disk
    Path(std::path::PathBuf),
    /// Base64-encoded image data
    Base64 { data: String, format: String },
}

/// Supported output image formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Png,
    Jpeg,
    Webp,
}

impl OutputFormat {
    pub fn extension(&self) -> &str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
        }
    }

    pub fn supports_alpha(&self) -> bool {
        matches!(self, Self::Png | Self::Webp)
    }
}

/// Type of noise scheduler for diffusion inference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulerType {
    Ddim,
    DpmPlusPlus,
    Euler,
    EulerAncestral,
    Lcm,
}

/// Core trait for diffusion model inference backends.
/// Each backend (Candle, diffusion_rs, ORT) implements this trait.
#[async_trait::async_trait]
pub trait DiffusionPipeline: Send + Sync {
    /// Generate an image from a text prompt
    async fn txt2img(
        &self,
        request: &Txt2ImgRequest,
        progress: Arc<dyn ProgressReporter>,
    ) -> Result<ImageOutput>;

    /// Transform an existing image guided by a text prompt
    async fn img2img(
        &self,
        request: &Img2ImgRequest,
        progress: Arc<dyn ProgressReporter>,
    ) -> Result<ImageOutput>;

    /// Fill masked regions of an image guided by a text prompt
    async fn inpaint(
        &self,
        request: &InpaintRequest,
        progress: Arc<dyn ProgressReporter>,
    ) -> Result<ImageOutput>;

    /// Get the name of this backend
    fn backend_name(&self) -> &str;

    /// Check if this backend supports a given model
    fn supports_model(&self, model: &ModelInfo) -> bool;

    /// Get estimated VRAM usage for a given request
    fn estimate_vram(&self, width: u32, height: u32, model: &ModelInfo) -> u64;

    /// Unload the current model from memory
    async fn unload(&mut self) -> Result<()>;

    /// Check if a model is currently loaded
    fn is_loaded(&self) -> bool;
}

pub struct DummyDiffusionPipeline;

impl DummyDiffusionPipeline {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl DiffusionPipeline for DummyDiffusionPipeline {
    async fn txt2img(
        &self,
        _request: &Txt2ImgRequest,
        _progress: Arc<dyn ProgressReporter>,
    ) -> Result<ImageOutput> {
        Err(openmedia_core::OpenMediaError::BackendUnavailable("Dummy backend".into()))
    }

    async fn img2img(
        &self,
        _request: &Img2ImgRequest,
        _progress: Arc<dyn ProgressReporter>,
    ) -> Result<ImageOutput> {
        Err(openmedia_core::OpenMediaError::BackendUnavailable("Dummy backend".into()))
    }

    async fn inpaint(
        &self,
        _request: &InpaintRequest,
        _progress: Arc<dyn ProgressReporter>,
    ) -> Result<ImageOutput> {
        Err(openmedia_core::OpenMediaError::BackendUnavailable("Dummy backend".into()))
    }

    fn backend_name(&self) -> &str {
        "dummy"
    }

    fn supports_model(&self, _model: &ModelInfo) -> bool {
        false
    }

    fn estimate_vram(&self, _width: u32, _height: u32, _model: &ModelInfo) -> u64 {
        0
    }

    async fn unload(&mut self) -> Result<()> {
        Ok(())
    }

    fn is_loaded(&self) -> bool {
        false
    }
}
