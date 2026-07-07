pub mod config;
pub mod error;
pub mod hardware;
pub mod models;
pub mod progress;
pub mod types;

// Re-export key items at the crate root for convenience
pub use config::{ComputeBackend, Config};
pub use error::{OpenMediaError, Result};
pub use hardware::{
    CpuFeatures, CpuInfo, GpuApiSupport, GpuInfo, GpuVendor, HardwareInfo, RamInfo,
};
pub use models::{ModelCategory, ModelFormat, ModelInfo, ModelRegistry};
pub use progress::{
    McpProgressReporter, NullProgressReporter, ProgressReporter, ProgressUpdate,
    StderrProgressReporter,
};
pub use types::{AnimatedSvgOutput, ImageOutput, QualityScore, SvgOutput, VideoSpec};
