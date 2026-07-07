pub mod branch;
pub mod db;
pub mod graph;

#[cfg(test)]
pub mod tests;

pub(crate) use db::{scope_from_args, with_db};

pub use graph::{
    AddObservationsTool, CreateEntitiesTool, CreateRelationsTool, DeleteEntitiesTool,
    DeleteObservationsTool, DeleteRelationsTool, OpenNodesTool, ReadGraphTool, SearchNodesTool,
};

pub use branch::{CommitDatabaseBranchTool, CreateDatabaseBranchTool, RollbackDatabaseBranchTool};

#[cfg(test)]
pub(crate) fn test_lock() -> &'static tokio::sync::Mutex<()> {
    static TEST_MUTEX: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    TEST_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}
