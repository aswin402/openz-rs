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
    text.len().div_ceil(4)
}

pub(crate) fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

pub(crate) fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

pub fn is_binary_file(path: &std::path::Path) -> bool {
    match std::fs::read(path) {
        Ok(content) => content.iter().take(4096).any(|&b| b == 0x00),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests;
