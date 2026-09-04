//! Memory service facade shared by memory tools and future repositories.

use crate::memory::model::{
    EmbeddingBackend, MemoryDatabase, MemoryLayer, MemoryQuery, MemoryScope, MemoryTable,
    MEMORY_TABLES,
};
use crate::memory::repositories::MemoryRepositories;
use anyhow::Result;
use rusqlite::Connection;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MemoryService {
    scope: MemoryScope,
    repositories: MemoryRepositories,
}

impl Default for MemoryService {
    fn default() -> Self {
        Self::current()
    }
}

impl MemoryService {
    pub fn current() -> Self {
        Self::new(MemoryScope::current())
    }

    pub fn from_tool_args(args: &Value) -> Self {
        Self::new(MemoryScope::from_tool_args(args))
    }

    pub fn new(scope: MemoryScope) -> Self {
        Self {
            scope,
            repositories: MemoryRepositories,
        }
    }

    pub fn with_identity(
        mut self,
        user_id: impl Into<String>,
        session_id: impl Into<String>,
        agent_id: impl Into<String>,
    ) -> Self {
        self.scope = self.scope.with_identity(user_id, session_id, agent_id);
        self
    }

    pub fn scope(&self) -> &MemoryScope {
        &self.scope
    }

    pub fn repositories(&self) -> MemoryRepositories {
        self.repositories
    }

    pub fn database_path(&self, database: MemoryDatabase) -> PathBuf {
        self.repositories.path(database)
    }

    pub fn table(&self, database: MemoryDatabase, name: &str) -> Option<&'static MemoryTable> {
        MEMORY_TABLES
            .iter()
            .find(|table| table.database == database && table.name == name)
    }

    pub fn database_for_layer(&self, layer: MemoryLayer) -> Option<MemoryDatabase> {
        MEMORY_TABLES
            .iter()
            .find(|table| table.layer == layer)
            .map(|table| table.database)
    }

    pub fn embedding_backend(&self) -> EmbeddingBackend {
        let mode = crate::config::loader::load_config()
            .ok()
            .and_then(|config| config.embeddings)
            .map(|config| config.mode)
            .unwrap_or_else(|| "local".to_string());
        match mode.trim().to_ascii_lowercase().as_str() {
            "cloud_only" => EmbeddingBackend::CloudOnly,
            "cloud" => EmbeddingBackend::CloudPreferred,
            _ => EmbeddingBackend::LocalFastEmbed,
        }
    }

    pub async fn embed(&self, text: &str, is_query: bool) -> Result<Vec<f32>> {
        crate::tools::shared_memory::get_embedding(text, is_query).await
    }

    pub fn with_local_model<F, R>(&self, operation: F) -> Result<R>
    where
        F: FnOnce(&mut fastembed::TextEmbedding) -> Result<R>,
    {
        crate::tools::shared_memory::with_model(operation)
    }

    pub fn hashed_semantic_embedding(&self, text: &str) -> Vec<f32> {
        crate::tools::memory_extra::working::semantic_embedding_for_text(text)
    }

    pub fn hashed_semantic_embedding_dimensions(&self) -> usize {
        crate::tools::memory_extra::working::SEMANTIC_EMBEDDING_DIMS
    }

    pub fn hashed_semantic_embedding_blob(&self, text: &str) -> Vec<u8> {
        crate::tools::memory_extra::working::semantic_embedding_blob(text)
    }

    pub fn hashed_semantic_embedding_from_blob(&self, blob: &[u8]) -> Vec<f32> {
        crate::tools::memory_extra::working::semantic_embedding_from_blob(blob)
    }

    pub fn cosine_similarity(&self, left: &[f32], right: &[f32]) -> f32 {
        crate::tools::shared_memory::cosine_similarity(left, right)
    }

    pub fn with_shared_db<F, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T>,
    {
        self.repositories.with_shared_db(operation)
    }

    pub fn with_graph_db<F, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        self.repositories.with_graph_db(operation)
    }

    pub fn query_for_scope(&self, query: impl Into<String>) -> MemoryQuery {
        MemoryQuery::new(query).with_scope(self.scope.clone())
    }
}
