pub mod cognitive;
pub mod consolidation;
pub mod db;
pub mod embeddings;
pub mod interaction;
pub mod research;

#[cfg(test)]
pub mod tests;

pub use db::{get_current_workspace, get_db_mutex, get_shared_client, get_sqlite_db_path, with_db};

pub use embeddings::{cosine_similarity, get_embedding, get_global_model};

pub use cognitive::{
    prune_decayed_memories, ClearMemoryTool, CognitiveMemoryEntry, DeleteMemoryTool,
    RecallMemoryTool, StoreMemoryTool, UpdateMemoryTool,
};

pub use research::{
    archive_research_entries, archive_research_entry, chunk_content_by_headings,
    search_research_entries, ArchiveResearchTool, SearchResearchTool,
};

pub use consolidation::consolidate_shared_memory;
pub use interaction::{get_recent_interactions, log_interaction, update_interaction_errors};
