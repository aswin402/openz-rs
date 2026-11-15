use crate::memory::{with_graph_db, with_shared_db as with_db};
use anyhow::{anyhow, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

use super::db::get_db_mutex;
use super::embeddings::{cosine_similarity, get_embedding};

const RRF_K: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchSearchMode {
    Hybrid,
    Semantic,
    Keyword,
}

impl ResearchSearchMode {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("semantic" | "vector") => Self::Semantic,
            Some("keyword" | "fts" | "bm25" | "lexical") => Self::Keyword,
            _ => Self::Hybrid,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchIndexStats {
    pub chunk_count: usize,
    pub distinct_sources: usize,
    pub distinct_urls: usize,
    pub fts_rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableResearchChunk {
    pub id: String,
    pub query: String,
    pub title: String,
    pub content: String,
    pub url: String,
    pub source: String,
    pub timestamp: String,
    #[serde(default)]
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableGraphRelation {
    pub from: String,
    pub to: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableResearchBundle {
    pub version: u32,
    pub exported_at: String,
    pub chunks: Vec<PortableResearchChunk>,
    #[serde(default)]
    pub relations: Vec<PortableGraphRelation>,
}

pub fn encode_embedding_blob(embedding: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(embedding.len() * 4);
    for val in embedding {
        out.extend_from_slice(&val.to_le_bytes());
    }
    out
}

pub fn decode_embedding_blob(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
        .collect()
}

pub fn decode_embedding_columns(blob: Option<&[u8]>, legacy_json: &str) -> Vec<f32> {
    match blob.filter(|bytes| !bytes.is_empty() && bytes.len() % 4 == 0) {
        Some(bytes) => decode_embedding_blob(bytes),
        None => serde_json::from_str::<Vec<f32>>(legacy_json).unwrap_or_default(),
    }
}

pub fn sanitize_fts5_query(raw: &str) -> Option<String> {
    let tokens: Vec<String> = raw
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .map(str::trim)
        .filter(|tok| tok.chars().count() >= 2)
        .take(16)
        .map(|tok| format!("\"{}\"*", tok.to_ascii_lowercase()))
        .collect();
    (!tokens.is_empty()).then(|| tokens.join(" OR "))
}

#[derive(Debug, Clone)]
struct RawArchiveRow {
    id: String,
    query: String,
    title: String,
    content: String,
    url: String,
    source: String,
    timestamp: String,
    embedding: Vec<f32>,
}

fn matches_filters(
    row: &RawArchiveRow,
    source_filter: Option<&str>,
    url_prefix: Option<&str>,
) -> bool {
    let source_ok = match source_filter.map(str::trim).filter(|s| !s.is_empty()) {
        Some(filter) => {
            let fl = filter.to_ascii_lowercase();
            row.source.to_ascii_lowercase().contains(&fl)
        }
        None => true,
    };
    let url_ok = match url_prefix.map(str::trim).filter(|s| !s.is_empty()) {
        Some(prefix) => {
            row.url.starts_with(prefix)
                || row.source.contains(prefix)
                || row.query.starts_with(prefix)
        }
        None => true,
    };
    source_ok && url_ok
}

pub async fn search_research_hybrid(
    query: &str,
    top_k: usize,
    mode: ResearchSearchMode,
    source_filter: Option<&str>,
    url_prefix: Option<&str>,
) -> Result<Vec<Value>> {
    let limit = top_k.clamp(1, 50);
    let use_vector = matches!(mode, ResearchSearchMode::Hybrid | ResearchSearchMode::Semantic);
    let use_keyword = matches!(mode, ResearchSearchMode::Hybrid | ResearchSearchMode::Keyword);

    let query_embed = match use_vector && !query.trim().is_empty() {
        true => get_embedding(query, true).await.unwrap_or_default(),
        false => Vec::new(),
    };
    let fts_query = match use_keyword {
        true => sanitize_fts5_query(query),
        false => None,
    };

    let sf_owned = source_filter.map(ToString::to_string);
    let up_owned = url_prefix.map(ToString::to_string);

    let _lock = get_db_mutex().lock().await;
    let (rows_by_id, fts_ranks) = with_db(|conn| {
        let mut fts_ranks: HashMap<String, usize> = HashMap::new();
        match fts_query.as_deref() {
            Some(fts_expr) => {
                let stmt_res = conn.prepare(
                    "SELECT entry_id
                     FROM research_archive_fts
                     WHERE research_archive_fts MATCH ?1
                     ORDER BY bm25(research_archive_fts, 0.0, 3.0, 2.5, 1.0, 1.5, 0.5)
                     LIMIT 200",
                );
                match stmt_res {
                    Ok(mut stmt) => {
                        let iter_res = stmt.query_map(params![fts_expr], |r| r.get::<_, String>(0));
                        match iter_res {
                            Ok(mapped) => {
                                for (rank_idx, id_res) in mapped.flatten().enumerate() {
                                    fts_ranks.entry(id_res).or_insert(rank_idx + 1);
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    Err(_) => {}
                }
            }
            None => {}
        }

        let mut stmt = conn.prepare(
            "SELECT id, query, COALESCE(title, ''), content, COALESCE(url, ''), source, timestamp, embedding, embedding_blob
             FROM research_archive
             ORDER BY timestamp DESC
             LIMIT 2000",
        )?;
        let mapped = stmt.query_map([], |row| {
            let legacy_json: String = row.get(7)?;
            let blob: Option<Vec<u8>> = row.get(8)?;
            let embedding = decode_embedding_columns(blob.as_deref(), &legacy_json);
            Ok(RawArchiveRow {
                id: row.get(0)?,
                query: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                url: row.get(4)?,
                source: row.get(5)?,
                timestamp: row.get(6)?,
                embedding,
            })
        })?;

        let mut map = HashMap::new();
        for item in mapped {
            let row = item?;
            match matches_filters(&row, sf_owned.as_deref(), up_owned.as_deref()) {
                true => {
                    map.insert(row.id.clone(), row);
                }
                false => {}
            }
        }
        Ok((map, fts_ranks))
    })?;

    let mut semantic_ranked: Vec<(String, f32)> = match use_vector && !query_embed.is_empty() {
        true => {
            let mut sims: Vec<(String, f32)> = rows_by_id
                .values()
                .filter(|row| !row.embedding.is_empty())
                .map(|row| (row.id.clone(), cosine_similarity(&query_embed, &row.embedding)))
                .filter(|(_, sim)| *sim > 0.12)
                .collect();
            sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            sims
        }
        false => Vec::new(),
    };
    semantic_ranked.truncate(200);
    let semantic_ranks: HashMap<String, (usize, f32)> = semantic_ranked
        .into_iter()
        .enumerate()
        .map(|(idx, (id, sim))| (id, (idx + 1, sim)))
        .collect();

    let query_lower = query.trim().to_ascii_lowercase();
    let mut fused: Vec<(f64, Value)> = Vec::new();
    for (id, row) in &rows_by_id {
        let fts_rank = fts_ranks.get(id).copied();
        let sem_info = semantic_ranks.get(id).copied();
        let substr_hit = !query_lower.is_empty()
            && (row.query.to_ascii_lowercase().contains(&query_lower)
                || row.title.to_ascii_lowercase().contains(&query_lower)
                || row.content.to_ascii_lowercase().contains(&query_lower));

        let rrf_fts = fts_rank
            .map(|rank| 1.0 / (RRF_K + rank as f64))
            .unwrap_or(0.0);
        let rrf_sem = sem_info
            .map(|(rank, sim)| (1.0 / (RRF_K + rank as f64)) + f64::from(sim) * 0.35)
            .unwrap_or(0.0);
        let substr_bonus = match substr_hit && use_keyword {
            true => 0.12,
            false => 0.0,
        };
        let score = rrf_fts + rrf_sem + substr_bonus;
        let matched = match mode {
            ResearchSearchMode::Semantic => sem_info.is_some(),
            ResearchSearchMode::Keyword => fts_rank.is_some() || substr_hit,
            ResearchSearchMode::Hybrid => fts_rank.is_some() || sem_info.is_some() || substr_hit,
        };

        match matched && score > 0.0 {
            true => fused.push((
                score,
                json!({
                    "id": row.id,
                    "query": row.query,
                    "title": row.title,
                    "url": row.url,
                    "content": row.content,
                    "source": row.source,
                    "timestamp": row.timestamp,
                    "score": score,
                }),
            )),
            false => {}
        }
    }

    fused.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(fused
        .into_iter()
        .take(limit)
        .map(|(_, val)| val)
        .collect())
}

pub async fn list_indexed_sources(
    source_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<Value>> {
    let cap = limit.clamp(1, 200);
    let sf = source_filter.map(|s| s.trim().to_ascii_lowercase());
    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(NULLIF(url, ''), source) AS key_uri,
                    MAX(COALESCE(NULLIF(title, ''), query)) AS display_title,
                    source,
                    COUNT(*) AS chunks,
                    MAX(timestamp) AS latest_ts
             FROM research_archive
             GROUP BY key_uri, source
             ORDER BY latest_ts DESC
             LIMIT 500",
        )?;
        let mapped = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for item in mapped {
            let (uri, title, src, chunks, ts) = item?;
            let keep = match sf.as_deref().filter(|s| !s.is_empty()) {
                Some(filter) => {
                    src.to_ascii_lowercase().contains(filter)
                        || uri.to_ascii_lowercase().contains(filter)
                }
                None => true,
            };
            match keep {
                true => out.push(json!({
                    "url": uri,
                    "title": title,
                    "source": src,
                    "chunk_count": chunks,
                    "indexed_at": ts,
                })),
                false => {}
            }
            match out.len() >= cap {
                true => break,
                false => {}
            }
        }
        Ok(out)
    })
}

pub fn searchxyz_repos_dir() -> PathBuf {
    crate::config::loader::runtime_data_dir()
        .join("searchxyz")
        .join("repos")
}

fn cleanup_github_clone_for_prefix(url_or_prefix: &str) -> bool {
    let trimmed = url_or_prefix
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git");
    let slug_opt = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("github.com/"));
    let parts: Vec<&str> = match slug_opt {
        Some(rest) => rest.split('/').filter(|p| !p.is_empty()).take(2).collect(),
        None => Vec::new(),
    };
    match parts.len() == 2 {
        true => {
            let sanitize = |seg: &str| -> String {
                seg.chars()
                    .filter(|c| c.is_ascii_alphanumeric() || matches!(*c, '-' | '_' | '.'))
                    .collect()
            };
            let prefix = format!("{}__{}", sanitize(parts[0]), sanitize(parts[1]));
            let repos_root = searchxyz_repos_dir();
            let mut removed_any = false;
            match std::fs::read_dir(&repos_root) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let file_name = entry.file_name().to_string_lossy().to_string();
                        let matches_repo = file_name == prefix
                            || file_name.starts_with(&format!("{prefix}__"));
                        let path = entry.path();
                        match matches_repo && path.starts_with(&repos_root) {
                            true => {
                                removed_any = std::fs::remove_dir_all(&path).is_ok() || removed_any;
                            }
                            false => {}
                        }
                    }
                }
                Err(_) => {}
            }
            removed_any
        }
        false => false,
    }
}

pub async fn delete_indexed_source(url_or_prefix: &str, confirm: bool) -> Result<Value> {
    confirm.then_some(()).ok_or_else(|| {
        anyhow!("Refusing destructive source deletion without explicit confirm=true")
    })?;
    let needle = url_or_prefix.trim();
    (!needle.is_empty())
        .then_some(())
        .ok_or_else(|| anyhow!("url_or_prefix must not be empty"))?;
    let prefix_like = format!("{needle}%");
    let source_like = format!("%{needle}%");

    let _lock = get_db_mutex().lock().await;
    let deleted_chunks = with_db(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM research_archive_fts
             WHERE entry_id IN (
                 SELECT id FROM research_archive
                 WHERE url = ?1 OR url LIKE ?2 OR source = ?1 OR source LIKE ?3 OR query = ?1
             )",
            params![needle, prefix_like, source_like],
        )?;
        let removed = tx.execute(
            "DELETE FROM research_archive
             WHERE url = ?1 OR url LIKE ?2 OR source = ?1 OR source LIKE ?3 OR query = ?1",
            params![needle, prefix_like, source_like],
        )?;
        let _ = tx.execute(
            "DELETE FROM source_bookmarks WHERE uri = ?1 OR uri LIKE ?2",
            params![needle, prefix_like],
        );
        tx.commit()?;
        Ok(removed)
    })?;

    let repo_removed = cleanup_github_clone_for_prefix(needle);
    Ok(json!({
        "status": "success",
        "deleted_chunks": deleted_chunks,
        "repo_clone_removed": repo_removed,
        "target": needle,
    }))
}

pub async fn clear_indexed_research(confirm: bool) -> Result<Value> {
    confirm.then_some(()).ok_or_else(|| {
        anyhow!("Refusing to clear local research index without explicit confirm=true")
    })?;

    let _lock = get_db_mutex().lock().await;
    let deleted_chunks = with_db(|conn| {
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM research_archive_fts", [])?;
        let removed = tx.execute("DELETE FROM research_archive", [])?;
        tx.commit()?;
        Ok(removed)
    })?;

    let repos_dir = searchxyz_repos_dir();
    let repos_cleared = match repos_dir.exists() {
        true => std::fs::remove_dir_all(&repos_dir).is_ok(),
        false => false,
    };

    Ok(json!({
        "status": "success",
        "deleted_chunks": deleted_chunks,
        "repos_cleared": repos_cleared,
    }))
}

pub async fn export_research_bundle(file_path: &str) -> Result<Value> {
    export_research_bundle_filtered(file_path, None, None).await
}

pub async fn export_research_bundle_filtered(
    file_path: &str,
    source_filter: Option<&str>,
    url_prefix: Option<&str>,
) -> Result<Value> {
    let validated = crate::tools::opendoc::security::validate_path(file_path)
        .map_err(|err| anyhow!("Invalid export path '{file_path}': {err}"))?;
    match validated.parent() {
        Some(parent) => std::fs::create_dir_all(parent)?,
        None => {}
    }

    let sf_lc = source_filter
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());
    let up_lc = url_prefix
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());

    let _lock = get_db_mutex().lock().await;
    let chunks = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, query, COALESCE(title, ''), content, COALESCE(url, ''), source, timestamp, embedding, embedding_blob
             FROM research_archive
             ORDER BY timestamp ASC",
        )?;
        let mapped = stmt.query_map([], |row| {
            let legacy_json: String = row.get(7)?;
            let blob: Option<Vec<u8>> = row.get(8)?;
            Ok(PortableResearchChunk {
                id: row.get(0)?,
                query: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                url: row.get(4)?,
                source: row.get(5)?,
                timestamp: row.get(6)?,
                embedding: decode_embedding_columns(blob.as_deref(), &legacy_json),
            })
        })?;
        let mut list = Vec::new();
        for item in mapped {
            let chunk = item?;
            let source_ok = match sf_lc.as_deref() {
                None => true,
                Some(needle) => {
                    chunk.source.to_ascii_lowercase().contains(needle)
                        || chunk.url.to_ascii_lowercase().contains(needle)
                        || chunk.query.to_ascii_lowercase().contains(needle)
                }
            };
            let url_ok = match up_lc.as_deref() {
                None => true,
                Some(prefix) => chunk.url.to_ascii_lowercase().starts_with(prefix),
            };
            match source_ok && url_ok {
                true => list.push(chunk),
                false => {}
            }
        }
        Ok(list)
    })?;

    let has_filter = sf_lc.is_some() || up_lc.is_some();
    let exported_urls: std::collections::HashSet<String> = chunks
        .iter()
        .filter(|c| !c.url.is_empty())
        .map(|c| c.url.to_ascii_lowercase())
        .collect();

    let relations = with_graph_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT from_name, to_name, relation_type
             FROM graph_edges
             WHERE valid_until IS NULL
             LIMIT 5000",
        )?;
        let mapped = stmt.query_map([], |row| {
            Ok(PortableGraphRelation {
                from: row.get(0)?,
                to: row.get(1)?,
                relation_type: row.get(2)?,
            })
        })?;
        let mut list = Vec::new();
        for item in mapped {
            let rel = item?;
            let keep = match has_filter {
                false => true,
                true => {
                    let f_lc = rel.from.to_ascii_lowercase();
                    let t_lc = rel.to.to_ascii_lowercase();
                    exported_urls.contains(&f_lc)
                        || exported_urls.contains(&t_lc)
                        || sf_lc
                            .as_deref()
                            .is_some_and(|n| f_lc.contains(n) || t_lc.contains(n))
                }
            };
            match keep {
                true => list.push(rel),
                false => {}
            }
        }
        Ok(list)
    })
    .unwrap_or_default();

    let bundle = PortableResearchBundle {
        version: 1,
        exported_at: chrono::Utc::now().to_rfc3339(),
        chunks,
        relations,
    };
    let serialized = serde_json::to_string_pretty(&bundle)?;
    std::fs::write(&validated, &serialized)?;

    Ok(json!({
        "status": "success",
        "file_path": validated.to_string_lossy(),
        "source_filter": sf_lc,
        "url_prefix": up_lc,
        "exported_chunks": bundle.chunks.len(),
        "exported_relations": bundle.relations.len(),
        "bytes_written": serialized.len(),
    }))
}

pub async fn import_research_bundle(file_path: &str, merge: bool) -> Result<Value> {
    let validated = crate::tools::opendoc::security::validate_path(file_path)
        .map_err(|err| anyhow!("Invalid import path '{file_path}': {err}"))?;
    let raw = std::fs::read_to_string(&validated)?;
    let bundle: PortableResearchBundle = serde_json::from_str(&raw)?;

    let _lock = get_db_mutex().lock().await;
    let imported_chunks = with_db(|conn| {
        let tx = conn.transaction()?;
        match merge {
            false => {
                tx.execute("DELETE FROM research_archive_fts", [])?;
                tx.execute("DELETE FROM research_archive", [])?;
            }
            true => {}
        }
        let mut count = 0usize;
        for chunk in &bundle.chunks {
            let blob = encode_embedding_blob(&chunk.embedding);
            let legacy_json = serde_json::to_string(&chunk.embedding)?;
            tx.execute(
                "DELETE FROM research_archive_fts WHERE entry_id = ?1",
                params![chunk.id],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO research_archive (id, query, title, content, url, source, timestamp, embedding, embedding_blob)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    chunk.id,
                    chunk.query,
                    chunk.title,
                    chunk.content,
                    chunk.url,
                    chunk.source,
                    chunk.timestamp,
                    legacy_json,
                    blob,
                ],
            )?;
            tx.execute(
                "INSERT INTO research_archive_fts (entry_id, query, title, content, url, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    chunk.id,
                    chunk.query,
                    chunk.title,
                    chunk.content,
                    chunk.url,
                    chunk.source,
                ],
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    })?;

    let mut imported_relations = 0usize;
    let _ = with_graph_db(|conn| {
        let tx = conn.unchecked_transaction()?;
        for rel in &bundle.relations {
            tx.execute(
                "INSERT OR IGNORE INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id)
                 VALUES (?1, 'Entity', '[]', '*', '*', '*')",
                params![rel.from],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id)
                 VALUES (?1, 'Entity', '[]', '*', '*', '*')",
                params![rel.to],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id)
                 VALUES (?1, ?2, ?3, '*', '*', '*')",
                params![rel.from, rel.to, rel.relation_type],
            )?;
            imported_relations += 1;
        }
        tx.commit()?;
        Ok(())
    });

    Ok(json!({
        "status": "success",
        "file_path": validated.to_string_lossy(),
        "imported_chunks": imported_chunks,
        "imported_relations": imported_relations,
        "merged": merge,
    }))
}

pub fn research_index_stats() -> Result<ResearchIndexStats> {
    with_db(|conn| {
        let (chunk_count, distinct_sources, distinct_urls) = conn.query_row(
            "SELECT COUNT(*),
                    COUNT(DISTINCT source),
                    COUNT(DISTINCT CASE WHEN url != '' THEN url ELSE NULL END)
             FROM research_archive",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?.max(0) as usize,
                    row.get::<_, i64>(1)?.max(0) as usize,
                    row.get::<_, i64>(2)?.max(0) as usize,
                ))
            },
        )?;
        let fts_rows = conn
            .query_row("SELECT COUNT(*) FROM research_archive_fts", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or(0)
            .max(0) as usize;
        Ok(ResearchIndexStats {
            chunk_count,
            distinct_sources,
            distinct_urls,
            fts_rows,
        })
    })
}
