use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Output from an image generation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageOutput {
    /// Path to the generated image file
    pub path: PathBuf,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// RNG seed used for generation
    pub seed: u64,
    /// Image format (png, jpeg, webp)
    pub format: String,
    /// File size in bytes
    pub file_size: u64,
    /// Generation UUID
    pub generation_id: String,
    /// CLIP alignment score (if scoring enabled)
    pub clip_score: Option<f32>,
    /// Aesthetic quality score (if scoring enabled)
    pub aesthetic_score: Option<f32>,
    /// Model used for generation
    pub model_used: String,
    /// Backend used (candle, diffusion_rs, ort)
    pub backend_used: String,
    /// Wall-clock generation time in seconds
    pub generation_time: f64,
}

/// Specification for a video output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSpec {
    /// Output file path
    pub path: PathBuf,
    /// Video width
    pub width: u32,
    /// Video height
    pub height: u32,
    /// Duration in seconds
    pub duration: f64,
    /// Frames per second
    pub fps: u32,
    /// Codec used
    pub codec: String,
    /// File size in bytes
    pub file_size: u64,
    /// Generation UUID
    pub generation_id: String,
    /// Renderer used (svg, native, browser)
    pub renderer_used: String,
    /// Total frames rendered
    pub total_frames: u32,
    /// Wall-clock generation time
    pub generation_time: f64,
}

/// Output from SVG generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgOutput {
    /// Path to the SVG file
    pub path: PathBuf,
    /// SVG width
    pub width: u32,
    /// SVG height
    pub height: u32,
    /// Raw SVG content (for inline use)
    pub content: Option<String>,
    /// File size in bytes
    pub file_size: u64,
    /// Generation UUID
    pub generation_id: String,
}

/// Output from animated SVG generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimatedSvgOutput {
    /// Path to the animated SVG file
    pub path: PathBuf,
    /// SVG width
    pub width: u32,
    /// SVG height
    pub height: u32,
    /// Total animation duration in seconds
    pub duration: f64,
    /// Number of animation elements
    pub animation_count: u32,
    /// File size in bytes
    pub file_size: u64,
    /// Generation UUID
    pub generation_id: String,
}

/// Quality scores for a generated image
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QualityScore {
    /// CLIP text-image alignment score (0.0–1.0)
    pub clip_score: Option<f32>,
    /// Aesthetic quality prediction (1.0–10.0)
    pub aesthetic_score: Option<f32>,
    /// Whether the scores suggest refinement would help
    pub needs_refinement: bool,
}
