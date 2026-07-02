pub mod scoping;
pub mod stats;
pub mod compress;
pub mod cache;

// Re-exports for Task 1
pub use scoping::{ScopeContextTool, SummarizeCodebaseTool};
pub use stats::{CountTokensTool, PingTool, ServerInfoTool};

// Shared Constants and Helpers
pub const MAX_INPUT_SIZE: usize = 512_000; // 500KB max input
pub const CACHE_CAPACITY: usize = 1000;

pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() { return 0; }
    (text.len() + 3) / 4
}
