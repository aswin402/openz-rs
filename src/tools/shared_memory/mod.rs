pub mod auto_capture;
pub mod cognitive;
pub mod consolidation;
pub mod db;
pub mod embeddings;
pub mod interaction;
pub mod knowledge;
pub mod research;
pub mod research_bundle;
pub mod web_cache;
pub mod workflows;

#[cfg(test)]
pub mod tests;

pub use db::{get_current_workspace, get_db_mutex, get_shared_client, get_sqlite_db_path, with_db};

pub use embeddings::{cosine_similarity, get_embedding, start_model_eviction, with_model};

pub use auto_capture::{auto_capture_research_memory, AutoCaptureSummary};

pub use cognitive::{
    prune_decayed_memories, ClearMemoryTool, CognitiveMemoryEntry, DeleteMemoryTool,
    RecallMemoryTool, StoreMemoryTool, UpdateMemoryTool,
};

pub use research::{
    archive_research_entries, archive_research_entry, archive_research_entry_with_meta,
    chunk_content_by_headings, search_research_entries, ArchiveResearchTool, SearchResearchTool,
};
pub use research_bundle::{
    clear_indexed_research, delete_indexed_source, export_research_bundle,
    export_research_bundle_filtered, import_research_bundle, list_indexed_sources,
    research_index_stats, search_research_hybrid, searchxyz_repos_dir, ResearchIndexStats,
    ResearchSearchMode,
};
pub use web_cache::{
    compute_expires_at, compute_expires_at_with_default_ttl, header_to_string, http_cache_stats,
    is_cache_fresh, load_http_cache, now_utc, refresh_http_cache_validators, save_http_cache,
    touch_http_cache, CachedHttpEntry, HttpCacheStats, WebCacheMode,
};

pub use consolidation::consolidate_shared_memory;
pub use interaction::{get_recent_interactions, log_interaction, update_interaction_errors};

pub use knowledge::{
    add_source_bookmark, delete_research_brief, delete_source, display_source_label,
    mark_source_checked, repair_research_brief_topics, save_research_brief, search_research_briefs,
    search_source_bookmarks, KnowledgeSourceTool, ResearchBrief, ResearchBriefTool, SourceBookmark,
};
pub use workflows::{
    add_workflow_card, delete_workflow, get_workflow_by_name, record_workflow_run,
    search_workflow_cards, set_workflow_status, WorkflowCard, WorkflowMemoryTool,
};

pub(crate) fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}
