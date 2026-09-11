pub mod cache;
pub mod compress;
pub mod policy;
pub mod scoping;
pub mod stats;
pub mod syntax;

// Re-exports
pub use cache::{
    CacheAlignTool, CacheStatsTool, ClearCacheTool, ExportCacheTool, ImportCacheTool,
    SearchCacheTool,
};
pub use compress::{
    CompressContentTool, CompressDiffTool, CompressDirectoryTool, CompressFileTool,
    CompressSchemaTool, CompressUrlTool, RetrieveOriginalTool, RunAndCompressTool,
};
pub use scoping::{ScopeContextTool, SummarizeCodebaseTool};
pub use stats::{CountTokensTool, HeadroomStatsTool, HeadroomUsageTool, PingTool, ServerInfoTool};

// Shared Constants and Helpers
pub const MAX_INPUT_SIZE: usize = 512_000; // 500KB max input
pub const CACHE_CAPACITY: usize = 1000;

pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.len().div_ceil(4)
}

#[cfg(test)]
mod tests;
