pub mod db;
pub mod graph;
pub mod branch;

#[cfg(test)]
pub mod tests;

pub(crate) use db::{with_db, scope_from_args};

pub use graph::{
    CreateEntitiesTool, CreateRelationsTool, AddObservationsTool,
    DeleteEntitiesTool, DeleteObservationsTool, DeleteRelationsTool,
    ReadGraphTool, SearchNodesTool, OpenNodesTool,
};

pub use branch::{
    CreateDatabaseBranchTool, CommitDatabaseBranchTool, RollbackDatabaseBranchTool,
};

#[cfg(test)]
pub(crate) fn test_lock() -> &'static tokio::sync::Mutex<()> {
    static TEST_MUTEX: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    TEST_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}
