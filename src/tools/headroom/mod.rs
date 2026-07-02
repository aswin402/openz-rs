pub mod scoping;
pub mod stats;
pub mod compress;
pub mod cache;

// Re-exports
pub use scoping::{ScopeContextTool, SummarizeCodebaseTool};
pub use stats::{CountTokensTool, PingTool, ServerInfoTool};
pub use compress::{
    CompressContentTool, RetrieveOriginalTool, CompressSchemaTool, CompressFileTool,
    CompressDiffTool, CompressUrlTool, RunAndCompressTool, CompressDirectoryTool,
};
pub use cache::{
    CacheStatsTool, ClearCacheTool, SearchCacheTool, CacheAlignTool, ExportCacheTool,
    ImportCacheTool,
};

// Shared Constants and Helpers
pub const MAX_INPUT_SIZE: usize = 512_000; // 500KB max input
pub const CACHE_CAPACITY: usize = 1000;

pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() { return 0; }
    (text.len() + 3) / 4
}

