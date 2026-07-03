pub mod config;
pub mod error;
pub mod hardware;
pub mod models;
pub mod progress;
pub mod types;

// Re-export key items at the crate root for convenience
pub use config::{Config, ComputeBackend};
pub use error::{OpenMediaError, Result};
pub use hardware::{HardwareInfo, CpuInfo, CpuFeatures, GpuInfo, GpuVendor, GpuApiSupport, RamInfo};
pub use models::{ModelInfo, ModelCategory, ModelFormat, ModelRegistry};
pub use progress::{ProgressReporter, McpProgressReporter, NullProgressReporter, StderrProgressReporter, ProgressUpdate};
pub use types::{ImageOutput, VideoSpec, SvgOutput, AnimatedSvgOutput, QualityScore};
