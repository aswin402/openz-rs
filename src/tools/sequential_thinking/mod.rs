pub mod store;
pub mod engine;
pub mod tools;

#[cfg(test)]
pub mod tests;

pub use store::{
    ThoughtData, ToolResult, QualityReport, SessionInfo, ThoughtStore,
    MemoryThoughtStore, SqliteThoughtStore, get_db_path, get_db_mutex, get_store,
};

pub use engine::{detect_loop, generate_mermaid, analyze_quality, export_session_as_markdown};

