use serde::{Deserialize, Serialize};
use crate::config::ComputeBackend;

/// Comprehensive hardware information for backend selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpu: Option<GpuInfo>,
    pub ram: RamInfo,
    pub available_backends: Vec<ComputeBackend>,
    pub ffmpeg_available: bool,
    pub ffmpeg_version: Option<String>,
    pub chrome_available: bool,
    pub chrome_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU brand string (e.g., "Intel(R) Core(TM) i7-12700")
    pub brand: String,
    /// Number of physical cores
    pub physical_cores: usize,
    /// Number of logical cores (with hyperthreading)
    pub logical_cores: usize,
    /// CPU architecture
    pub arch: String,
    /// Supported instruction sets
    pub features: CpuFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuFeatures {
    pub avx: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub sse4_1: bool,
    pub sse4_2: bool,
    pub fma: bool,
    pub neon: bool,  // ARM
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU name (e.g., "NVIDIA GeForce RTX 3060")
    pub name: String,
    /// GPU vendor
    pub vendor: GpuVendor,
    /// Total VRAM in bytes
    pub vram_total: u64,
    /// Available VRAM in bytes (estimated)
    pub vram_available: u64,
    /// Supported graphics APIs
    pub api_support: GpuApiSupport,
    /// CUDA compute capability (NVIDIA only)
    pub cuda_compute: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuApiSupport {
    pub vulkan: bool,
    pub vulkan_version: Option<String>,
    pub metal: bool,
    pub dx12: bool,
    pub cuda: bool,
    pub cuda_version: Option<String>,
    pub opencl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RamInfo {
    /// Total system RAM in bytes
    pub total: u64,
    /// Available RAM in bytes
    pub available: u64,
}

impl HardwareInfo {
    /// Detect current hardware capabilities
    pub async fn detect() -> Self {
        // Detect CPU characteristics
        let logical_cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        let physical_cores = logical_cores / 2; // Simplistic approximation
        
        let arch = std::env::consts::ARCH.to_string();
        
        // Detect CPU instruction set features
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        let features = CpuFeatures {
            avx: std::is_x86_feature_detected!("avx"),
            avx2: std::is_x86_feature_detected!("avx2"),
            avx512f: std::is_x86_feature_detected!("avx512f"),
            sse4_1: std::is_x86_feature_detected!("sse4.1"),
            sse4_2: std::is_x86_feature_detected!("sse4.2"),
            fma: std::is_x86_feature_detected!("fma"),
            neon: false,
        };
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        let features = CpuFeatures {
            avx: false,
            avx2: false,
            avx512f: false,
            sse4_1: false,
            sse4_2: false,
            fma: false,
            neon: cfg!(target_arch = "aarch64") || cfg!(target_arch = "arm"),
        };

        let cpu = CpuInfo {
            brand: "Generic Processor".to_string(),
            physical_cores,
            logical_cores,
            arch,
            features,
        };

        // Detect RAM (simplified defaults, since querying OS ram is platform-specific and requires extra dependencies)
        let ram = RamInfo {
            total: 16 * 1024 * 1024 * 1024,      // 16 GB default
            available: 8 * 1024 * 1024 * 1024,  // 8 GB default
        };

        // Simple default backends (CPU fallback always available)
        let available_backends = vec![ComputeBackend::CpuOnly];

        // Check if FFmpeg is available
        let ffmpeg_available = std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_ok();

        // Check if Chrome is available
        let chrome_cmd = if cfg!(target_os = "windows") {
            "chrome.exe"
        } else {
            "google-chrome"
        };
        let chrome_available = std::process::Command::new(chrome_cmd)
            .arg("--version")
            .output()
            .is_ok();

        Self {
            cpu,
            gpu: None, // Simplified GPU detection in Phase 0
            ram,
            available_backends,
            ffmpeg_available,
            ffmpeg_version: None,
            chrome_available,
            chrome_version: None,
        }
    }

    /// Select the best compute backend for a given model and operation
    pub fn select_backend(
        &self,
        _model_format: &str,
        preferred: ComputeBackend,
    ) -> ComputeBackend {
        if preferred == ComputeBackend::Auto {
            ComputeBackend::CpuOnly
        } else {
            preferred
        }
    }

    /// Estimate max image resolution achievable with current hardware
    pub fn max_resolution_for_model(&self, _model_id: &str) -> (u32, u32) {
        (1024, 1024)
    }
}
