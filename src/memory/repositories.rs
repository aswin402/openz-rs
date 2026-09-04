//! Database boundaries for the existing memory stores.

use crate::memory::model::MemoryDatabase;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryRepositories;

impl MemoryRepositories {
    pub fn path(&self, database: MemoryDatabase) -> PathBuf {
        match database {
            MemoryDatabase::Shared => crate::tools::shared_memory::get_sqlite_db_path(),
            MemoryDatabase::Graph | MemoryDatabase::EmbeddingCache => {
                crate::config::loader::runtime_db_path(database.filename())
            }
        }
    }

    pub fn with_shared_db<F, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T>,
    {
        crate::tools::shared_memory::with_db(operation)
            .context("shared memory database operation failed")
    }

    pub fn with_graph_db<F, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        crate::tools::graph_memory::with_db(operation)
            .context("graph memory database operation failed")
    }
}

pub(crate) fn with_shared_db<F, T>(operation: F) -> Result<T>
where
    F: FnOnce(&mut Connection) -> Result<T>,
{
    MemoryRepositories.with_shared_db(operation)
}

pub(crate) fn with_graph_db<F, T>(operation: F) -> Result<T>
where
    F: FnOnce(&Connection) -> Result<T>,
{
    MemoryRepositories.with_graph_db(operation)
}
