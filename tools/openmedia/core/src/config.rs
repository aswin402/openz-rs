use crate::error::{OpenMediaError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Root configuration for the OpenMedia server.
/// Loaded from config.toml, environment variables, and CLI defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server identification and metadata
    pub server: ServerConfig,

    /// File system paths for models, outputs, and data
    pub paths: PathConfig,

    /// Hardware and compute backend preferences
    pub compute: ComputeConfig,

    /// Image generation defaults
    pub image: ImageConfig,

    /// Video generation defaults
    pub video: VideoConfig,

    /// SVG generation defaults
    pub svg: SvgConfig,

    /// Image processing defaults
    pub processing: ProcessingConfig,

    /// Self-improvement system configuration
    pub improve: ImproveConfig,

    /// Resource limits and safety bounds
    pub limits: LimitsConfig,

    /// Logging and diagnostics
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server name reported via MCP
    pub name: String,
    /// Server version
    pub version: String,
    /// Whether to enable progress notifications
    pub progress_notifications: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    /// Root directory for model storage
    /// Default: ~/.openmedia/models/
    pub model_dir: PathBuf,

    /// Root directory for generated outputs
    /// Default: ~/.openmedia/output/
    pub output_dir: PathBuf,

    /// SQLite database path for generation history
    /// Default: ~/.openmedia/history.db
    pub history_db: PathBuf,

    /// Subdirectory for image outputs
    /// Default: images/
    pub image_subdir: String,

    /// Subdirectory for video outputs
    /// Default: videos/
    pub video_subdir: String,

    /// Subdirectory for SVG outputs
    /// Default: svgs/
    pub svg_subdir: String,

    /// Model checksum file
    /// Default: ~/.openmedia/models/checksums.sha256
    pub checksum_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeConfig {
    /// Preferred compute backend
    pub preferred_backend: ComputeBackend,

    /// Maximum CPU threads for inference
    /// Default: num_cpus / 2
    pub max_cpu_threads: usize,

    /// Maximum GPU memory to use (bytes)
    /// Default: 0 (auto-detect)
    pub max_gpu_memory: u64,

    /// Enable GPU acceleration for image processing
    pub gpu_processing: bool,

    /// CUDA device index (for multi-GPU systems)
    pub cuda_device: u32,

    /// Enable memory-mapped model loading
    pub mmap_models: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComputeBackend {
    /// Automatic selection based on hardware
    Auto,
    /// Candle framework (CPU/CUDA/Metal)
    Candle,
    /// diffusion_rs for GGUF quantized models
    DiffusionRs,
    /// ONNX Runtime
    Ort,
    /// CPU-only (force no GPU)
    CpuOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    /// Default model for txt2img when "auto"
    pub default_model: String,
    /// Default image width
    pub default_width: u32,
    /// Default image height
    pub default_height: u32,
    /// Default number of denoising steps
    pub default_steps: u32,
    /// Default classifier-free guidance scale
    pub default_cfg_scale: f32,
    /// Default noise scheduler
    pub default_scheduler: String,
    /// Default output format
    pub default_format: String,
    /// Default output quality (JPEG/WebP)
    pub default_quality: u8,
    /// Default CLIP skip layers
    pub default_clip_skip: u32,
    /// Enable auto-refine by default
    pub auto_refine: bool,
    /// Default max refinement rounds
    pub max_refine_rounds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoConfig {
    /// Default FPS for video output
    pub default_fps: u32,
    /// Default video codec
    pub default_codec: String,
    /// Default encoding quality preset
    pub default_quality: String,
    /// Default renderer selection
    pub default_renderer: String,
    /// Path to FFmpeg binary
    pub ffmpeg_path: Option<PathBuf>,
    /// Path to Chrome/Chromium binary
    pub chrome_path: Option<PathBuf>,
    /// Number of parallel render threads
    pub render_threads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgConfig {
    /// Default SVG width
    pub default_width: u32,
    /// Default SVG height
    pub default_height: u32,
    /// Enable SVG optimization by default
    pub optimize_by_default: bool,
    /// Default coordinate precision (decimal places)
    pub default_precision: u8,
    /// Default chart theme
    pub default_chart_theme: String,
    /// Default diagram direction
    pub default_diagram_direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Prefer GPU for image processing
    pub prefer_gpu: bool,
    /// Default resize method
    pub default_resize_method: String,
    /// Default output format
    pub default_format: String,
    /// Default JPEG/WebP quality
    pub default_quality: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImproveConfig {
    /// Enable generation history logging
    pub enable_history: bool,
    /// Enable CLIP scoring on image generations
    pub enable_clip_scoring: bool,
    /// Enable aesthetic scoring
    pub enable_aesthetic_scoring: bool,
    /// CLIP score threshold for refinement suggestions
    pub clip_threshold: f32,
    /// Aesthetic score threshold for refinement suggestions
    pub aesthetic_threshold: f32,
    /// Maximum database size in bytes
    pub max_db_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    /// Maximum image dimension (width or height)
    pub max_image_dimension: u32,
    /// Maximum video duration in seconds
    pub max_video_duration: f64,
    /// Maximum video resolution width
    pub max_video_width: u32,
    /// Maximum video resolution height
    pub max_video_height: u32,
    /// Maximum batch size for image generation
    pub max_batch_size: u32,
    /// Maximum concurrent operations
    pub max_concurrent_ops: usize,
    /// Maximum output file size in bytes
    pub max_output_file_size: u64,
    /// Maximum input file size in bytes
    pub max_input_file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level: trace, debug, info, warn, error
    pub level: String,
    /// Log format: json, pretty, compact
    pub format: String,
    /// Log to file (path) in addition to stderr
    pub file: Option<PathBuf>,
}

impl Config {
    /// Load configuration with priority: env vars > config file > defaults
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // Check if config file exists at ~/.openmedia/config.toml
        let base_dirs = directories::BaseDirs::new().ok_or_else(|| {
            OpenMediaError::ConfigError("Could not resolve home directory".into())
        })?;
        let config_path = base_dirs.home_dir().join(".openmedia/config.toml");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let file_config: Config = toml::from_str(&content).map_err(|e| {
                OpenMediaError::ConfigError(format!("Failed to parse config file: {}", e))
            })?;
            config = file_config;
        }

        // Apply environment overrides
        if let Ok(val) = std::env::var("OPENMEDIA_MODEL_DIR") {
            config.paths.model_dir = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("OPENMEDIA_OUTPUT_DIR") {
            config.paths.output_dir = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("OPENMEDIA_HISTORY_DB") {
            config.paths.history_db = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("OPENMEDIA_GPU_PREFERENCE") {
            match val.to_lowercase().as_str() {
                "candle" => config.compute.preferred_backend = ComputeBackend::Candle,
                "diffusion_rs" | "diffusionrs" => {
                    config.compute.preferred_backend = ComputeBackend::DiffusionRs
                }
                "ort" | "onnx" => config.compute.preferred_backend = ComputeBackend::Ort,
                "cpuonly" | "cpu" => config.compute.preferred_backend = ComputeBackend::CpuOnly,
                _ => {}
            }
        }

        Ok(config)
    }

    /// Resolve the full output path for a given category and filename
    pub fn output_path(&self, category: &str, filename: &str) -> PathBuf {
        self.paths.output_dir.join(category).join(filename)
    }

    /// Resolve the model path for a given model ID
    pub fn model_path(&self, category: &str, filename: &str) -> PathBuf {
        self.paths.model_dir.join(category).join(filename)
    }
}

impl Default for Config {
    fn default() -> Self {
        let home = directories::BaseDirs::new()
            .map(|b| b.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            server: ServerConfig {
                name: "openmedia".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                progress_notifications: true,
            },
            paths: PathConfig {
                model_dir: home.join(".openmedia/models"),
                output_dir: home.join(".openmedia/output"),
                history_db: home.join(".openmedia/history.db"),
                image_subdir: "images".into(),
                video_subdir: "videos".into(),
                svg_subdir: "svgs".into(),
                checksum_file: home.join(".openmedia/models/checksums.sha256"),
            },
            compute: ComputeConfig {
                preferred_backend: ComputeBackend::Auto,
                max_cpu_threads: std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
                    / 2,
                max_gpu_memory: 0,
                gpu_processing: true,
                cuda_device: 0,
                mmap_models: true,
            },
            image: ImageConfig {
                default_model: "auto".into(),
                default_width: 512,
                default_height: 512,
                default_steps: 20,
                default_cfg_scale: 7.5,
                default_scheduler: "dpm++".into(),
                default_format: "png".into(),
                default_quality: 95,
                default_clip_skip: 1,
                auto_refine: false,
                max_refine_rounds: 3,
            },
            video: VideoConfig {
                default_fps: 30,
                default_codec: "h264".into(),
                default_quality: "balanced".into(),
                default_renderer: "auto".into(),
                ffmpeg_path: None,
                chrome_path: None,
                render_threads: std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
                    .min(8),
            },
            svg: SvgConfig {
                default_width: 800,
                default_height: 600,
                optimize_by_default: true,
                default_precision: 2,
                default_chart_theme: "dark".into(),
                default_diagram_direction: "TB".into(),
            },
            processing: ProcessingConfig {
                prefer_gpu: true,
                default_resize_method: "lanczos3".into(),
                default_format: "png".into(),
                default_quality: 95,
            },
            improve: ImproveConfig {
                enable_history: true,
                enable_clip_scoring: true,
                enable_aesthetic_scoring: true,
                clip_threshold: 0.25,
                aesthetic_threshold: 4.5,
                max_db_size: 1_073_741_824, // 1 GB
            },
            limits: LimitsConfig {
                max_image_dimension: 4096,
                max_video_duration: 600.0,
                max_video_width: 3840,
                max_video_height: 2160,
                max_batch_size: 4,
                max_concurrent_ops: 2,
                max_output_file_size: 2_147_483_648, // 2 GB
                max_input_file_size: 536_870_912,    // 512 MB
            },
            logging: LoggingConfig {
                level: "info".into(),
                format: "compact".into(),
                file: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.name, "openmedia");
        assert_eq!(config.image.default_width, 512);
        assert_eq!(config.video.default_fps, 30);
    }

    #[test]
    fn test_env_overrides() {
        std::env::set_var("OPENMEDIA_MODEL_DIR", "/tmp/models");
        std::env::set_var("OPENMEDIA_OUTPUT_DIR", "/tmp/output");
        std::env::set_var("OPENMEDIA_GPU_PREFERENCE", "ort");

        let config = Config::load().unwrap();
        assert_eq!(config.paths.model_dir, PathBuf::from("/tmp/models"));
        assert_eq!(config.paths.output_dir, PathBuf::from("/tmp/output"));
        assert_eq!(config.compute.preferred_backend, ComputeBackend::Ort);

        std::env::remove_var("OPENMEDIA_MODEL_DIR");
        std::env::remove_var("OPENMEDIA_OUTPUT_DIR");
        std::env::remove_var("OPENMEDIA_GPU_PREFERENCE");
    }
}
