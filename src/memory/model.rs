//! Shared memory-domain types and the compatibility map for existing stores.
//!
//! These types describe the current storage contract. They do not create or
//! migrate tables; the existing memory and graph modules remain authoritative
//! until individual consumers are moved behind `MemoryService`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryDatabase {
    Shared,
    Graph,
    EmbeddingCache,
}

impl MemoryDatabase {
    pub const fn filename(self) -> &'static str {
        match self {
            Self::Shared => "memory.db",
            Self::Graph => "graph_memory.db",
            Self::EmbeddingCache => "embeddings_cache.db",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLayer {
    Working,
    Cognitive,
    Semantic,
    Episodic,
    Reflective,
    Procedural,
    Graph,
    Shared,
    Research,
    Interaction,
    Knowledge,
    Codebase,
    Cache,
    SessionMetadata,
    Skills,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeModel {
    Global,
    Workspace,
    SessionKey,
    UserSessionAgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryTable {
    pub database: MemoryDatabase,
    pub name: &'static str,
    pub layer: MemoryLayer,
    pub scope: ScopeModel,
    pub uses_fts5: bool,
    pub stores_embeddings: bool,
}

/// Current table ownership and scope contract across OpenZ memory stores.
pub const MEMORY_TABLES: &[MemoryTable] = &[
    MemoryTable {
        database: MemoryDatabase::Shared,
        name: "cognitive_memory",
        layer: MemoryLayer::Cognitive,
        scope: ScopeModel::Workspace,
        uses_fts5: false,
        stores_embeddings: true,
    },
    MemoryTable {
        database: MemoryDatabase::Shared,
        name: "research_archive",
        layer: MemoryLayer::Research,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: true,
    },
    MemoryTable {
        database: MemoryDatabase::Shared,
        name: "interaction_history",
        layer: MemoryLayer::Interaction,
        scope: ScopeModel::SessionKey,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Shared,
        name: "source_bookmarks",
        layer: MemoryLayer::Knowledge,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Shared,
        name: "research_briefs",
        layer: MemoryLayer::Knowledge,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Shared,
        name: "web_fetch_cache",
        layer: MemoryLayer::Cache,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Shared,
        name: "workflow_cards",
        layer: MemoryLayer::Procedural,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Shared,
        name: "workflow_runs",
        layer: MemoryLayer::Procedural,
        scope: ScopeModel::SessionKey,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Shared,
        name: "skills",
        layer: MemoryLayer::Skills,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "graph_nodes",
        layer: MemoryLayer::Graph,
        scope: ScopeModel::UserSessionAgent,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "graph_edges",
        layer: MemoryLayer::Graph,
        scope: ScopeModel::UserSessionAgent,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "episodic_logs",
        layer: MemoryLayer::Episodic,
        scope: ScopeModel::UserSessionAgent,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "reflection_memory",
        layer: MemoryLayer::Reflective,
        scope: ScopeModel::UserSessionAgent,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "tool_performance",
        layer: MemoryLayer::Procedural,
        scope: ScopeModel::UserSessionAgent,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "shared_agent_memory",
        layer: MemoryLayer::Shared,
        scope: ScopeModel::UserSessionAgent,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "semantic_metadata",
        layer: MemoryLayer::Semantic,
        scope: ScopeModel::UserSessionAgent,
        uses_fts5: true,
        stores_embeddings: true,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "code_elements",
        layer: MemoryLayer::Codebase,
        scope: ScopeModel::UserSessionAgent,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "code_calls",
        layer: MemoryLayer::Codebase,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "repo_evolution",
        layer: MemoryLayer::Codebase,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::Graph,
        name: "semantic_fts",
        layer: MemoryLayer::Semantic,
        scope: ScopeModel::Global,
        uses_fts5: true,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::EmbeddingCache,
        name: "file_cache",
        layer: MemoryLayer::Cache,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: false,
    },
    MemoryTable {
        database: MemoryDatabase::EmbeddingCache,
        name: "chunk_cache",
        layer: MemoryLayer::Cache,
        scope: ScopeModel::Global,
        uses_fts5: false,
        stores_embeddings: true,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingBackend {
    LocalFastEmbed,
    CloudPreferred,
    CloudOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryScope {
    pub workspace: String,
    pub user_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub session_key: Option<String>,
}

impl MemoryScope {
    pub fn new(
        user_id: impl Into<String>,
        session_id: impl Into<String>,
        agent_id: impl Into<String>,
    ) -> Self {
        Self::current().with_identity(user_id, session_id, agent_id)
    }

    pub fn session(session_id: impl Into<String>) -> Self {
        Self::new("*", session_id, "*")
    }

    pub fn current() -> Self {
        Self {
            workspace: crate::tools::shared_memory::get_current_workspace(),
            user_id: "*".to_string(),
            session_id: "*".to_string(),
            agent_id: "*".to_string(),
            session_key: None,
        }
    }

    pub fn global() -> Self {
        Self::current()
    }

    pub fn from_tool_args(args: &Value) -> Self {
        let mut scope = Self::current();
        scope.user_id = scoped_value(args, "userId", "user_id");
        scope.session_id = scoped_value(args, "sessionId", "session_id");
        scope.agent_id = scoped_value(args, "agentId", "agent_id");
        scope.session_key = args
            .get("sessionKey")
            .or_else(|| args.get("session_key"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(workspace) = args
            .get("workspace")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            scope.workspace = workspace.to_string();
        }
        scope
    }

    pub fn with_workspace(mut self, workspace: impl Into<String>) -> Self {
        self.workspace = workspace.into();
        self
    }

    pub fn with_identity(
        mut self,
        user_id: impl Into<String>,
        session_id: impl Into<String>,
        agent_id: impl Into<String>,
    ) -> Self {
        self.user_id = user_id.into();
        self.session_id = session_id.into();
        self.agent_id = agent_id.into();
        self
    }

    pub fn with_session_key(mut self, session_key: impl Into<String>) -> Self {
        self.session_key = Some(session_key.into());
        self
    }
}

fn scoped_value(args: &Value, camel: &str, snake: &str) -> String {
    args.get(camel)
        .or_else(|| args.get(snake))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("*")
        .to_string()
}

pub(crate) fn scope_from_args(args: &Value) -> (String, String, String) {
    let scope = MemoryScope::from_tool_args(args);
    (scope.user_id, scope.session_id, scope.agent_id)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryQuery {
    pub text: String,
    pub layers: Vec<MemoryLayer>,
    pub scope: MemoryScope,
    pub limit: usize,
    pub min_score: Option<f32>,
}

impl MemoryQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            layers: Vec::new(),
            scope: MemoryScope::current(),
            limit: 10,
            min_score: None,
        }
    }

    pub fn with_scope(mut self, scope: MemoryScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_layers(mut self, layers: impl IntoIterator<Item = MemoryLayer>) -> Self {
        self.layers = layers.into_iter().collect();
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_min_score(mut self, min_score: f32) -> Self {
        self.min_score = Some(min_score);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecallItem {
    pub id: String,
    pub text: String,
    pub layer: MemoryLayer,
    pub score: f64,
    pub raw: Value,
}
