pub mod db;
pub mod embeddings;
pub mod cognitive;
pub mod research;
pub mod interaction;
pub mod consolidation;
pub mod tests;

pub use db::{
    get_db_mutex,
    get_shared_client,
    get_sqlite_db_path,
    get_sqlite_connection,
    get_current_workspace,
};

pub use embeddings::{get_global_model, get_embedding, cosine_similarity};

pub use cognitive::{
    CognitiveMemoryEntry, prune_decayed_memories, StoreMemoryTool, RecallMemoryTool,
    ClearMemoryTool, DeleteMemoryTool, UpdateMemoryTool,
};


