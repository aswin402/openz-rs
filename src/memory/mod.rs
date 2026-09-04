//! Unified memory boundary over OpenZ's existing SQLite stores and embedding paths.
//!
//! The facade is intentionally additive. It documents and exposes one domain
//! vocabulary while existing tools continue to own their current SQL and
//! response behavior. Consumers can migrate one layer at a time without a
//! database migration or a compatibility break.

pub mod model;
pub mod repositories;
pub mod service;

pub use model::{
    EmbeddingBackend, MemoryDatabase, MemoryLayer, MemoryQuery, MemoryRecallItem, MemoryScope,
    MemoryTable, ScopeModel, MEMORY_TABLES,
};
pub(crate) use model::scope_from_args;
pub use repositories::MemoryRepositories;
pub(crate) use repositories::{with_graph_db, with_shared_db};
pub use service::MemoryService;
