use thiserror::Error;

/// Unified error type for all OpenMedia operations
#[derive(Debug, Error)]
pub enum OpenMediaError {
    // === Configuration Errors ===
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid configuration value for '{key}': {reason}")]
    ConfigValueError { key: String, reason: String },

    // === Model Errors ===
    #[error("Model not found: '{0}'. Run model download first.")]
    ModelNotFound(String),

    #[error("Failed to load model '{model}': {reason}")]
    ModelLoadFailed { model: String, reason: String },

    #[error("Model checksum mismatch for '{model}': expected {expected}, got {actual}")]
    ChecksumMismatch {
        model: String,
        expected: String,
        actual: String,
    },

    // === Inference Errors ===
    #[error("Inference error in {backend}: {message}")]
    InferenceError { backend: String, message: String },

    #[error("No suitable backend available for model '{0}'")]
    BackendUnavailable(String),

    #[error("Out of memory: operation requires {required} bytes, {available} available")]
    OutOfMemory { required: u64, available: u64 },

    // === Image Errors ===
    #[error("Failed to decode image: {0}")]
    ImageDecodeError(String),

    #[error("Failed to encode image to {format}: {reason}")]
    ImageEncodeError { format: String, reason: String },

    #[error("Invalid image dimensions: {width}x{height}. {reason}")]
    InvalidDimensions {
        width: u32,
        height: u32,
        reason: String,
    },

    // === Video Errors ===
    #[error("Video rendering error in frame {frame}: {message}")]
    RenderingError { frame: u32, message: String },

    #[error("Video encoding error: {0}")]
    EncodingError(String),

    #[error("FFmpeg not found. Install FFmpeg 6.0+ for video encoding.")]
    FfmpegNotFound,

    #[error("Chrome not found. Install Chrome/Chromium 120+ for Tier-3 rendering.")]
    ChromeNotFound,

    #[error("Invalid scene definition: {0}")]
    InvalidScene(String),

    // === SVG Errors ===
    #[error("Invalid SVG input: {0}")]
    InvalidSvgInput(String),

    #[error("Chart data error: {0}")]
    ChartDataError(String),

    #[error("Diagram layout error: {0}")]
    DiagramLayoutError(String),

    // === GPU Errors ===
    #[error("GPU pipeline error: {0}")]
    GpuError(String),

    #[error("WGSL shader compilation error in '{shader}': {message}")]
    ShaderError { shader: String, message: String },

    // === Scoring Errors ===
    #[error("Scoring model error: {0}")]
    ScoringError(String),

    // === Storage Errors ===
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Cannot write to output path '{path}': {reason}")]
    OutputPathError { path: String, reason: String },

    #[error("Input file not found: '{0}'")]
    InputFileNotFound(String),

    #[error("File too large: {size} bytes exceeds limit of {limit} bytes")]
    FileTooLarge { size: u64, limit: u64 },

    // === Parameter Validation ===
    #[error("Invalid parameter '{param}': {reason}")]
    InvalidParameter { param: String, reason: String },

    // === Generic ===
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl OpenMediaError {
    /// Convert to MCP error code
    pub fn mcp_error_code(&self) -> i32 {
        match self {
            Self::ModelNotFound(_) => 1001,
            Self::ModelLoadFailed { .. } | Self::ChecksumMismatch { .. } => 1002,
            Self::InferenceError { .. } => 1003,
            Self::BackendUnavailable(_) | Self::OutOfMemory { .. } => 1004,
            Self::RenderingError { .. } => 2001,
            Self::EncodingError(_) => 2002,
            Self::FfmpegNotFound => 2003,
            Self::ChromeNotFound => 2004,
            Self::InvalidSvgInput(_) => 3001,
            Self::ChartDataError(_) => 3002,
            Self::DiagramLayoutError(_) => 3003,
            Self::GpuError(_) | Self::ShaderError { .. } => 4001,
            Self::ImageDecodeError(_) => 4002,
            Self::ImageEncodeError { .. } => 4003,
            Self::ScoringError(_) => 5001,
            Self::DatabaseError(_) => 5002,
            Self::OutputPathError { .. } => 6001,
            Self::InputFileNotFound(_) => 6002,
            Self::FileTooLarge { .. } => 6003,
            Self::InvalidParameter { .. } => -32602,
            _ => -32603,
        }
    }
}

/// Result type alias for OpenMedia operations
pub type Result<T> = std::result::Result<T, OpenMediaError>;
