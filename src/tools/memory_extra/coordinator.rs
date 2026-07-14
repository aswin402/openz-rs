use crate::tools::graph_memory::with_db;
use crate::tools::memory_extra::facts::{forget_session_metadata_memories, forget_skills};
use crate::tools::memory_extra::search::query_fts5;
use crate::tools::memory_extra::working::{
    active_working_memory_count, semantic_embedding_for_text, semantic_embedding_from_blob,
    store_semantic_fact,
};
use crate::tools::shared_memory::cosine_similarity;
use anyhow::Result;
use rusqlite::params;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryLayer {
    Semantic,
    Graph,
    Shared,
    Cognitive,
    Research,
    SessionMetadata,
    Skills,
}

#[derive(Debug, Clone)]
pub struct MemoryScope {
    pub user_id: String,
    pub session_id: String,
    pub agent_id: String,
}

impl MemoryScope {
    pub fn new(
        user_id: impl Into<String>,
        session_id: impl Into<String>,
        agent_id: impl Into<String>,
    ) -> Self {
        Self {
            user_id: user_id.into(),
            session_id: session_id.into(),
            agent_id: agent_id.into(),
        }
    }

    pub fn session(session_id: impl Into<String>) -> Self {
        Self::new("*", session_id, "*")
    }
}

#[derive(Debug, Clone)]
pub struct MemoryWriteResult {
    pub id: String,
    pub layer: MemoryLayer,
}

#[derive(Debug, Clone)]
pub struct GraphWriteResult {
    pub layer: MemoryLayer,
    pub created: bool,
    pub conflicts_resolved: i64,
}

#[derive(Debug, Clone)]
pub struct MemoryRecallItem {
    pub id: String,
    pub text: String,
    pub layer: MemoryLayer,
    pub score: f64,
    pub raw: Value,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryForgetResult {
    pub semantic_facts_expired: i64,
    pub semantic_fts_rows_deleted: i64,
    pub graph_nodes_deleted: i64,
    pub graph_edges_expired: i64,
    pub shared_memories_deleted: i64,
    pub cognitive_memories_deleted: i64,
    pub research_entries_deleted: i64,
    pub session_memories_scrubbed: i64,
    pub skills_scrubbed: i64,
    pub raw: Value,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryStatsSnapshot {
    pub graph_nodes: i64,
    pub graph_edges: i64,
    pub shared_memories: i64,
    pub semantic_facts: i64,
    pub semantic_facts_with_embeddings: i64,
    pub cognitive_memories: i64,
    pub research_entries: i64,
    pub session_metadata_memories: i64,
    pub skills_memories: i64,
    pub working_memory: i64,
    pub total_active: i64,
}

#[derive(Debug, Default, Clone)]
pub struct MemoryCoordinator;

const EXCLUSIVE_RELATIONS: &[&str] = &[
    "lives_in",
    "current_job",
    "spouse",
    "has_status",
    "is_born_in",
    "located_in",
];

pub fn calculate_auto_importance(
    access_count: u32,
    edge_count: u32,
    age_hours: f64,
    was_reinforced: bool,
) -> f64 {
    let access_score = (access_count as f64 + 1.0).ln() / 10.0;
    let connection_score = (edge_count as f64 + 1.0).ln() / 5.0;
    let freshness = (-0.05 * age_hours).exp() / 3.0;
    let reinforcement = if was_reinforced { 0.3 } else { 0.0 };
    (access_score + connection_score + freshness + reinforcement).min(1.0)
}

fn is_exclusive_relation(relation_type: &str) -> bool {
    EXCLUSIVE_RELATIONS.contains(&relation_type)
}

fn count_session_metadata_memories() -> i64 {
    let sessions_dir = crate::config::loader::runtime_data_dir().join("sessions");
    let Ok(entries) = std::fs::read_dir(sessions_dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .filter_map(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .filter(|session| {
            session
                .get("metadata")
                .and_then(|metadata| metadata.get("memory"))
                .and_then(|memory| memory.as_str())
                .map(|memory| !memory.trim().is_empty())
                .unwrap_or(false)
        })
        .count() as i64
}

fn count_skills_memories() -> i64 {
    crate::agent::skills::get_connection()
        .and_then(|conn| {
            conn.query_row("SELECT COUNT(*) FROM skills", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(anyhow::Error::from)
        })
        .unwrap_or(0)
}

const SEMANTIC_EXCLUSIVE_PATTERNS: &[(&str, &str)] = &[
    (" current job is ", "current_job"),
    (" job is ", "current_job"),
    (" lives in ", "lives_in"),
    (" live in ", "lives_in"),
    (" works at ", "works_at"),
    (" works for ", "works_at"),
    (" located in ", "located_in"),
    (" born in ", "is_born_in"),
    (" spouse is ", "spouse"),
    (" status is ", "has_status"),
];

fn normalize_semantic_text(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    lower
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn semantic_exclusive_slot(text: &str) -> Option<(String, &'static str)> {
    let normalized = format!(" {} ", normalize_semantic_text(text));
    for (pattern, slot) in SEMANTIC_EXCLUSIVE_PATTERNS {
        if let Some(idx) = normalized.find(pattern) {
            let subject = normalized[..idx].trim();
            if !subject.is_empty() {
                return Some((subject.to_string(), *slot));
            }
        }
    }
    None
}

fn resolve_semantic_slot_conflicts(
    text: &str,
    importance: f64,
    scope: &MemoryScope,
) -> Result<i64> {
    let Some((subject, slot)) = semantic_exclusive_slot(text) else {
        return Ok(0);
    };

    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT node_id, raw_text, importance
             FROM semantic_metadata
             WHERE valid_until IS NULL
               AND user_id = ?1
               AND session_id = ?2
               AND agent_id = ?3",
        )?;
        let rows = stmt.query_map(
            params![scope.user_id, scope.session_id, scope.agent_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                ))
            },
        )?;

        let mut expired = 0_i64;
        for row in rows {
            let (node_id, existing_text, existing_importance) = row?;
            if existing_text == text || existing_importance > importance {
                continue;
            }
            let existing_slot = semantic_exclusive_slot(&existing_text);
            if !matches!(existing_slot.as_ref(), Some((existing_subject, existing_slot)) if existing_subject == &subject && existing_slot == &slot)
            {
                continue;
            }
            expired += conn.execute(
                "UPDATE semantic_metadata
                 SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE node_id = ?1
                   AND user_id = ?2
                   AND session_id = ?3
                   AND agent_id = ?4
                   AND valid_until IS NULL",
                params![node_id, scope.user_id, scope.session_id, scope.agent_id],
            )? as i64;
        }
        Ok(expired)
    })
}

fn resolve_semantic_similarity_conflicts(
    text: &str,
    importance: f64,
    scope: &MemoryScope,
) -> Result<i64> {
    let new_embedding = semantic_embedding_for_text(text);
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT node_id, raw_text, importance, embedding
             FROM semantic_metadata
             WHERE valid_until IS NULL
               AND embedding IS NOT NULL
               AND user_id = ?1
               AND session_id = ?2
               AND agent_id = ?3",
        )?;
        let rows = stmt.query_map(
            params![scope.user_id, scope.session_id, scope.agent_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            },
        )?;

        let mut expired = 0_i64;
        for row in rows {
            let (node_id, existing_text, existing_importance, blob) = row?;
            if existing_text == text || existing_importance > importance {
                continue;
            }
            let existing_embedding = semantic_embedding_from_blob(&blob);
            let similarity = cosine_similarity(&new_embedding, &existing_embedding) as f64;
            if similarity < 0.90 {
                continue;
            }
            expired += conn.execute(
                "UPDATE semantic_metadata
                 SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE node_id = ?1
                   AND user_id = ?2
                   AND session_id = ?3
                   AND agent_id = ?4
                   AND valid_until IS NULL",
                params![node_id, scope.user_id, scope.session_id, scope.agent_id],
            )? as i64;
        }
        Ok(expired)
    })
}

impl MemoryCoordinator {
    pub async fn write_semantic(
        &self,
        text: &str,
        importance: f64,
        scope: &MemoryScope,
    ) -> Result<MemoryWriteResult> {
        let id = format!("coordinator-semantic-{}", uuid::Uuid::new_v4());
        self.write_semantic_with_id(&id, text, importance, scope)
            .await
    }

    pub async fn write_semantic_with_id(
        &self,
        id: &str,
        text: &str,
        importance: f64,
        scope: &MemoryScope,
    ) -> Result<MemoryWriteResult> {
        let importance = if importance < 0.0 {
            calculate_auto_importance(0, 0, 0.0, false)
        } else {
            importance.clamp(0.0, 1.0)
        };
        let _ = resolve_semantic_slot_conflicts(text, importance, scope)?;
        let _ = resolve_semantic_similarity_conflicts(text, importance, scope)?;
        store_semantic_fact(
            id,
            text,
            importance,
            &scope.user_id,
            &scope.session_id,
            &scope.agent_id,
        )?;
        Ok(MemoryWriteResult {
            id: id.to_string(),
            layer: MemoryLayer::Semantic,
        })
    }

    pub async fn write_graph_relation(
        &self,
        from: &str,
        relation_type: &str,
        to: &str,
        scope: &MemoryScope,
    ) -> Result<GraphWriteResult> {
        with_db(|conn| {
            for name in [from, to] {
                conn.execute(
                    "INSERT OR IGNORE INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id)
                     VALUES (?1, 'Concept', '[]', ?2, ?3, ?4)",
                    params![name, scope.user_id, scope.session_id, scope.agent_id],
                )?;
            }

            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_edges WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6 AND valid_until IS NULL)",
                params![from, to, relation_type, scope.user_id, scope.session_id, scope.agent_id],
                |row| row.get(0),
            )?;
            if exists {
                return Ok(GraphWriteResult {
                    layer: MemoryLayer::Graph,
                    created: false,
                    conflicts_resolved: 0,
                });
            }

            let conflicts_resolved = if is_exclusive_relation(relation_type) {
                conn.execute(
                    "UPDATE graph_edges
                     SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                     WHERE from_name = ?1
                       AND relation_type = ?2
                       AND to_name <> ?3
                       AND valid_until IS NULL
                       AND user_id = ?4
                       AND session_id = ?5
                       AND agent_id = ?6",
                    params![
                        from,
                        relation_type,
                        to,
                        scope.user_id,
                        scope.session_id,
                        scope.agent_id
                    ],
                )? as i64
            } else {
                0
            };

            conn.execute(
                "INSERT INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![from, to, relation_type, scope.user_id, scope.session_id, scope.agent_id],
            )?;
            Ok(GraphWriteResult {
                layer: MemoryLayer::Graph,
                created: true,
                conflicts_resolved,
            })
        })
    }

    pub async fn recall(
        &self,
        query: &str,
        limit: usize,
        scope: &MemoryScope,
    ) -> Result<Vec<MemoryRecallItem>> {
        let raw = self.recall_raw(query, limit, scope).await?;
        let items = raw
            .into_iter()
            .map(|item| {
                let id = item
                    .get("nodeId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let text = item
                    .get("rawText")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let score = item
                    .get("vectorSimilarity")
                    .and_then(|v| v.as_f64())
                    .or_else(|| item.get("importance").and_then(|v| v.as_f64()))
                    .unwrap_or(0.0);
                MemoryRecallItem {
                    id,
                    text,
                    layer: MemoryLayer::Semantic,
                    score,
                    raw: item,
                }
            })
            .collect();
        Ok(items)
    }

    pub async fn recall_raw(
        &self,
        query: &str,
        limit: usize,
        scope: &MemoryScope,
    ) -> Result<Vec<Value>> {
        let fts_results = with_db(|conn| {
            query_fts5(
                conn,
                query,
                limit * 2,
                &scope.user_id,
                &scope.session_id,
                &scope.agent_id,
            )
        })?;

        let query_embedding = semantic_embedding_for_text(query);
        let vector_results = with_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, raw_text, timestamp, importance, embedding
                 FROM semantic_metadata
                 WHERE valid_until IS NULL
                   AND embedding IS NOT NULL
                   AND (?1 IS NULL OR user_id = ?1 OR user_id = '*')
                   AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
                   AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')",
            )?;
            let mut rows = stmt.query(params![scope.user_id, scope.session_id, scope.agent_id])?;
            let mut scored = Vec::new();
            while let Some(row) = rows.next()? {
                let blob: Vec<u8> = row.get(4)?;
                let embedding = semantic_embedding_from_blob(&blob);
                let similarity = cosine_similarity(&query_embedding, &embedding) as f64;
                if similarity <= 0.0 {
                    continue;
                }
                let importance: f64 = row.get(3)?;
                let score = (similarity * 0.85) + (importance * 0.15);
                scored.push((
                    json!({
                        "nodeId": row.get::<_, String>(0)?,
                        "rawText": row.get::<_, String>(1)?,
                        "timestamp": row.get::<_, String>(2)?,
                        "importance": importance,
                        "vectorSimilarity": similarity,
                    }),
                    score,
                ));
            }
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit * 2);
            Ok(scored)
        })?;

        let mut doc_scores: HashMap<String, f64> = HashMap::new();
        let mut doc_map: HashMap<String, Value> = HashMap::new();

        for (i, fact) in fts_results.iter().enumerate() {
            let rank = (i + 1) as f64;
            let node_id = fact["nodeId"].as_str().unwrap_or("");
            *doc_scores.entry(node_id.to_string()).or_insert(0.0) += 1.0 / (60.0 + rank);
            doc_map
                .entry(node_id.to_string())
                .or_insert_with(|| fact.clone());
        }

        for (i, (fact, _)) in vector_results.iter().enumerate() {
            let rank = (i + 1) as f64;
            let node_id = fact["nodeId"].as_str().unwrap_or("");
            *doc_scores.entry(node_id.to_string()).or_insert(0.0) += 1.0 / (60.0 + rank);
            doc_map.insert(node_id.to_string(), fact.clone());
        }

        let mut ranked: Vec<(String, f64)> = doc_scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(limit);

        Ok(ranked
            .into_iter()
            .filter_map(|(node_id, _)| doc_map.remove(&node_id))
            .collect())
    }

    pub async fn forget(&self, query: &str, scope: &MemoryScope) -> Result<MemoryForgetResult> {
        let like = format!("%{}%", query);
        let (
            semantic_facts_expired,
            semantic_fts_rows_deleted,
            graph_nodes_deleted,
            graph_edges_expired,
            shared_memories_deleted,
            tombstones_written,
        ) = with_db(|conn| {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS memory_tombstones (
                    id TEXT PRIMARY KEY,
                    query TEXT NOT NULL,
                    source TEXT NOT NULL,
                    scope_user_id TEXT NOT NULL,
                    scope_session_id TEXT NOT NULL,
                    scope_agent_id TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
                )",
                [],
            )?;

            let mut semantic_ids = Vec::new();
            {
                let mut stmt = conn.prepare(
                    "SELECT node_id FROM semantic_metadata
                     WHERE raw_text LIKE ?1 AND valid_until IS NULL
                       AND (user_id = ?2 OR user_id = '*')
                       AND (session_id = ?3 OR session_id = '*')
                       AND (agent_id = ?4 OR agent_id = '*')",
                )?;
                let rows = stmt.query_map(
                    params![like, scope.user_id, scope.session_id, scope.agent_id],
                    |r| r.get::<_, String>(0),
                )?;
                for id in rows.flatten() {
                    semantic_ids.push(id);
                }
            }

            let semantic_expired = conn.execute(
                "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE raw_text LIKE ?1 AND valid_until IS NULL
                   AND (user_id = ?2 OR user_id = '*')
                   AND (session_id = ?3 OR session_id = '*')
                   AND (agent_id = ?4 OR agent_id = '*')",
                params![like, scope.user_id, scope.session_id, scope.agent_id],
            )? as i64;

            let mut fts_deleted = 0i64;
            for node_id in &semantic_ids {
                fts_deleted += conn.execute(
                    "DELETE FROM semantic_fts WHERE node_id = ?1",
                    params![node_id],
                )? as i64;
            }

            let graph_nodes_deleted = conn.execute(
                "DELETE FROM graph_nodes
                 WHERE (name LIKE ?1 OR entity_type LIKE ?1 OR observations LIKE ?1)
                   AND (user_id = ?2 OR user_id = '*')
                   AND (session_id = ?3 OR session_id = '*')
                   AND (agent_id = ?4 OR agent_id = '*')",
                params![like, scope.user_id, scope.session_id, scope.agent_id],
            )? as i64;

            let graph_edges_expired = conn.execute(
                "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE valid_until IS NULL
                   AND (from_name LIKE ?1 OR to_name LIKE ?1 OR relation_type LIKE ?1)
                   AND (user_id = ?2 OR user_id = '*')
                   AND (session_id = ?3 OR session_id = '*')
                   AND (agent_id = ?4 OR agent_id = '*')",
                params![like, scope.user_id, scope.session_id, scope.agent_id],
            )? as i64;

            let shared_deleted = conn.execute(
                "DELETE FROM shared_agent_memory
                 WHERE (memory_key LIKE ?1 OR memory_value LIKE ?1 OR source_agent LIKE ?1)
                   AND (user_id = ?2 OR user_id = '*')
                   AND (session_id = ?3 OR session_id = '*')
                   AND (agent_id = ?4 OR agent_id = '*')",
                params![like, scope.user_id, scope.session_id, scope.agent_id],
            )? as i64;

            let sources = [
                ("semantic", semantic_expired),
                ("graph_nodes", graph_nodes_deleted),
                ("graph_edges", graph_edges_expired),
                ("shared_agent_memory", shared_deleted),
            ];
            let mut tombstones = 0i64;
            for (source, count) in sources {
                if count > 0 {
                    conn.execute(
                        "INSERT INTO memory_tombstones (id, query, source, scope_user_id, scope_session_id, scope_agent_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            format!("tombstone-{}", uuid::Uuid::new_v4()),
                            query,
                            source,
                            scope.user_id,
                            scope.session_id,
                            scope.agent_id
                        ],
                    )?;
                    tombstones += 1;
                }
            }

            Ok((
                semantic_expired,
                fts_deleted,
                graph_nodes_deleted,
                graph_edges_expired,
                shared_deleted,
                tombstones,
            ))
        })?;

        let (cognitive_memories_deleted, research_entries_deleted) =
            crate::tools::shared_memory::with_db(|conn| {
                let cognitive_deleted = conn.execute(
                    "DELETE FROM cognitive_memory WHERE text LIKE ?1 OR tags LIKE ?1",
                    params![like],
                )? as i64;
                let research_deleted = conn.execute(
                    "DELETE FROM research_archive WHERE query LIKE ?1 OR content LIKE ?1 OR source LIKE ?1",
                    params![like],
                )? as i64;
                Ok((cognitive_deleted, research_deleted))
            })?;

        let session_memories_scrubbed = forget_session_metadata_memories(query)?;
        let skills_scrubbed = forget_skills(query)?;
        let raw = json!({
            "status": "forgotten",
            "query": query,
            "semanticFactsExpired": semantic_facts_expired,
            "semanticFtsRowsDeleted": semantic_fts_rows_deleted,
            "graphNodesDeleted": graph_nodes_deleted,
            "graphEdgesExpired": graph_edges_expired,
            "sharedMemoriesDeleted": shared_memories_deleted,
            "cognitiveMemoriesDeleted": cognitive_memories_deleted,
            "researchEntriesDeleted": research_entries_deleted,
            "sessionMemoriesScrubbed": session_memories_scrubbed,
            "skillsScrubbed": skills_scrubbed,
            "tombstonesWritten": tombstones_written,
            "unsupportedStores": []
        });

        Ok(MemoryForgetResult {
            semantic_facts_expired,
            semantic_fts_rows_deleted,
            graph_nodes_deleted,
            graph_edges_expired,
            shared_memories_deleted,
            cognitive_memories_deleted,
            research_entries_deleted,
            session_memories_scrubbed,
            skills_scrubbed,
            raw,
        })
    }

    pub async fn stats(&self, scope: &MemoryScope) -> Result<MemoryStatsSnapshot> {
        let (
            graph_nodes,
            graph_edges,
            shared_memories,
            semantic_facts,
            semantic_facts_with_embeddings,
        ) = with_db(|conn| {
            let graph_nodes = conn.query_row(
                    "SELECT COUNT(*) FROM graph_nodes WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*')",
                    rusqlite::params![scope.user_id, scope.session_id, scope.agent_id],
                    |r| r.get::<_, i64>(0),
                )?;
            let graph_edges = conn.query_row(
                    "SELECT COUNT(*) FROM graph_edges WHERE valid_until IS NULL AND (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*')",
                    rusqlite::params![scope.user_id, scope.session_id, scope.agent_id],
                    |r| r.get::<_, i64>(0),
                )?;
            let shared_memories = conn.query_row(
                    "SELECT COUNT(*) FROM shared_agent_memory WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*')",
                    rusqlite::params![scope.user_id, scope.session_id, scope.agent_id],
                    |r| r.get::<_, i64>(0),
                )?;
            let semantic_facts = conn.query_row(
                    "SELECT COUNT(*) FROM semantic_metadata WHERE valid_until IS NULL AND (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*')",
                    rusqlite::params![scope.user_id, scope.session_id, scope.agent_id],
                    |r| r.get::<_, i64>(0),
                )?;
            let semantic_facts_with_embeddings = conn.query_row(
                    "SELECT COUNT(*) FROM semantic_metadata WHERE valid_until IS NULL AND embedding IS NOT NULL AND length(embedding) > 0 AND (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*')",
                    rusqlite::params![scope.user_id, scope.session_id, scope.agent_id],
                    |r| r.get::<_, i64>(0),
                )?;
            Ok((
                graph_nodes,
                graph_edges,
                shared_memories,
                semantic_facts,
                semantic_facts_with_embeddings,
            ))
        })?;

        let (cognitive_memories, research_entries) =
            crate::tools::shared_memory::with_db(|conn| {
                let cognitive_memories =
                    conn.query_row("SELECT COUNT(*) FROM cognitive_memory", [], |r| {
                        r.get::<_, i64>(0)
                    })?;
                let research_entries =
                    conn.query_row("SELECT COUNT(*) FROM research_archive", [], |r| {
                        r.get::<_, i64>(0)
                    })?;
                Ok((cognitive_memories, research_entries))
            })?;

        let session_metadata_memories = count_session_metadata_memories();
        let skills_memories = count_skills_memories();
        let working_memory =
            active_working_memory_count(&scope.user_id, &scope.session_id, &scope.agent_id);

        let total_active = graph_nodes
            + graph_edges
            + shared_memories
            + semantic_facts
            + cognitive_memories
            + research_entries
            + session_metadata_memories
            + skills_memories
            + working_memory;

        Ok(MemoryStatsSnapshot {
            graph_nodes,
            graph_edges,
            shared_memories,
            semantic_facts,
            semantic_facts_with_embeddings,
            cognitive_memories,
            research_entries,
            session_metadata_memories,
            skills_memories,
            working_memory,
            total_active,
        })
    }
}
