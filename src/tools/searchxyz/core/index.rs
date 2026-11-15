use rusqlite::params;

use crate::memory::with_shared_db;
use crate::tools::searchxyz::core::config::IndexConfig;
use crate::tools::searchxyz::core::error::SearchXyzError;
use crate::tools::searchxyz::core::extractor::ExtractedContent;
use crate::tools::shared_memory::research_bundle::{
    encode_embedding_blob, sanitize_fts5_query, search_research_hybrid, ResearchSearchMode,
};
use crate::tools::shared_memory::{get_db_mutex, get_embedding};

/// Lightweight native adapter over OpenZ's unified SQLite `memory.db`
/// (`research_archive` + `research_archive_fts` + shared idle-evicting embeddings).
#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    scope_tag: Option<String>,
}

/// A result from querying the local index.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexSearchResult {
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub source: String,
    pub score: f32,
}

/// An entry representing a document's source metadata.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceEntry {
    pub url: String,
    pub title: String,
    pub source: String,
    pub indexed_at: String,
}

impl SearchIndex {
    /// Open a zero-RAM handle backed by OpenZ's shared SQLite `memory.db`.
    pub fn open(config: &IndexConfig) -> Result<Self, SearchXyzError> {
        with_shared_db(|_| Ok(()))
            .map_err(|e| SearchXyzError::IndexError(format!("Failed to initialize memory.db: {e}")))?;

        let custom_test_scope = cfg!(test)
            && !config.path.as_os_str().is_empty()
            && !config.path.ends_with("memory.db");
        let scope_tag = custom_test_scope.then(|| config.path.to_string_lossy().to_string());

        Ok(Self { scope_tag })
    }

    fn scoped_source(&self, source: &str) -> String {
        match &self.scope_tag {
            Some(tag) => format!("{tag}::{source}"),
            None => source.to_string(),
        }
    }

    fn unscoped_source<'a>(&self, stored: &'a str) -> Option<&'a str> {
        match &self.scope_tag {
            Some(tag) => stored
                .strip_prefix(tag.as_str())
                .and_then(|rest| rest.strip_prefix("::")),
            None => Some(stored),
        }
    }

    fn scope_sql_prefix(&self) -> Option<String> {
        self.scope_tag.as_ref().map(|tag| format!("{tag}::%"))
    }

    /// Index a piece of extracted content into `research_archive` + `research_archive_fts`.
    pub async fn add_document(
        &self,
        content: &ExtractedContent,
        source: &str,
    ) -> Result<(), SearchXyzError> {
        let raw_chunks = chunk_content(&content.content_markdown, 1500, 200);
        let chunks = match raw_chunks.is_empty() {
            true => vec![content.content_markdown.clone()],
            false => raw_chunks,
        };

        let multi_chunk = chunks.len() > 1;
        let stored_source = self.scoped_source(source);
        let mut prepared = Vec::with_capacity(chunks.len());

        for (idx, chunk) in chunks.into_iter().enumerate() {
            let chunk_url = match multi_chunk {
                true => format!("{}#chunk-{idx}", content.url),
                false => content.url.clone(),
            };
            let embed_input: String = format!("{}\n\n{}", content.title, chunk)
                .chars()
                .take(1200)
                .collect();
            let embedding = get_embedding(&embed_input, false).await.unwrap_or_default();
            let blob = encode_embedding_blob(&embedding);
            let legacy_json = serde_json::to_string(&embedding)?;
            prepared.push((chunk_url, chunk, legacy_json, blob));
        }

        let base_url = content.url.clone();
        let chunk_like = format!("{base_url}#%");
        let title = content.title.clone();
        let scope_like = self.scope_sql_prefix();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let _lock = get_db_mutex().lock().await;
        with_shared_db(|conn| {
            let tx = conn.transaction()?;
            delete_url_rows_tx(&tx, &base_url, &chunk_like, scope_like.as_deref())?;

            for (chunk_url, chunk_text, legacy_json, blob) in &prepared {
                let id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT OR REPLACE INTO research_archive (id, query, title, content, url, source, timestamp, embedding, embedding_blob)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        id,
                        title,
                        title,
                        chunk_text,
                        chunk_url,
                        stored_source,
                        timestamp,
                        legacy_json,
                        blob,
                    ],
                )?;
                tx.execute(
                    "INSERT INTO research_archive_fts (entry_id, query, title, content, url, source)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, title, title, chunk_text, chunk_url, stored_source],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .map_err(|e| SearchXyzError::IndexError(format!("Failed to persist document: {e}")))?;

        tracing::debug!(url = %content.url, "Indexed document into shared memory.db");
        Ok(())
    }

    /// Hybrid FTS5 + cosine vector search via OpenZ's shared research index.
    pub async fn search_hybrid(
        &self,
        query_str: &str,
        max_results: usize,
        mode: ResearchSearchMode,
    ) -> Result<Vec<IndexSearchResult>, SearchXyzError> {
        let filter = self.scope_tag.as_deref();
        let values = search_research_hybrid(query_str, max_results, mode, filter, None)
            .await
            .map_err(|e| SearchXyzError::IndexError(format!("Hybrid search failed: {e}")))?;

        let results = values
            .into_iter()
            .filter_map(|val| {
                let raw_source = val.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let clean_source = self.unscoped_source(raw_source)?.to_string();
                let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let snippet = build_snippet(content);
                Some(IndexSearchResult {
                    url: val
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    title: val
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet,
                    source: clean_source,
                    score: val.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                })
            })
            .collect();
        Ok(results)
    }

    /// Semantic vector search across indexed content.
    pub async fn search_semantic(
        &self,
        query_str: &str,
        max_results: usize,
    ) -> Result<Vec<IndexSearchResult>, SearchXyzError> {
        self.search_hybrid(query_str, max_results, ResearchSearchMode::Semantic)
            .await
    }

    /// Full-text keyword search across indexed content via SQLite FTS5.
    pub fn search(
        &self,
        query_str: &str,
        max_results: usize,
    ) -> Result<Vec<IndexSearchResult>, SearchXyzError> {
        let limit = max_results.clamp(1, 100);
        let fts_expr = match sanitize_fts5_query(query_str) {
            Some(expr) => expr,
            None => return Ok(Vec::new()),
        };

        with_shared_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT r.url,
                        COALESCE(NULLIF(r.title, ''), r.query),
                        r.content,
                        r.source,
                        -bm25(research_archive_fts, 0.0, 3.0, 2.5, 1.0, 1.5, 0.5) AS rank_score
                 FROM research_archive_fts
                 JOIN research_archive r ON r.id = research_archive_fts.entry_id
                 WHERE research_archive_fts MATCH ?1
                 ORDER BY bm25(research_archive_fts, 0.0, 3.0, 2.5, 1.0, 1.5, 0.5)
                 LIMIT 250",
            )?;
            let mapped = stmt.query_map(params![fts_expr], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4).unwrap_or(0.0) as f32,
                ))
            })?;

            let mut out = Vec::new();
            for item in mapped {
                let (url, title, content, raw_source, score) = item?;
                match self.unscoped_source(&raw_source) {
                    Some(clean_source) => {
                        out.push(IndexSearchResult {
                            url,
                            title,
                            snippet: build_snippet(&content),
                            source: clean_source.to_string(),
                            score,
                        });
                    }
                    None => {}
                }
                match out.len() >= limit {
                    true => break,
                    false => {}
                }
            }
            Ok(out)
        })
        .map_err(|e| SearchXyzError::IndexError(format!("FTS5 search failed: {e}")))
    }

    /// Retrieve metadata of indexed documents with optional source filtering and pagination.
    pub fn list_documents(
        &self,
        source_filter: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<SourceEntry>, usize), SearchXyzError> {
        let exact_source = source_filter.map(|src| self.scoped_source(src));
        let scope_like = self.scope_sql_prefix();

        with_shared_db(|conn| {
            let mut stmt = conn.prepare(
                "SELECT url,
                        COALESCE(NULLIF(title, ''), query),
                        source,
                        timestamp
                 FROM research_archive
                 WHERE (?1 IS NULL OR source = ?1)
                   AND (?2 IS NULL OR source LIKE ?2)
                 ORDER BY timestamp DESC
                 LIMIT 10000",
            )?;
            let mapped = stmt.query_map(params![exact_source, scope_like], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;

            let mut all_entries = Vec::new();
            for item in mapped {
                let (url, title, raw_source, indexed_at) = item?;
                match self.unscoped_source(&raw_source) {
                    Some(clean_source) => {
                        all_entries.push(SourceEntry {
                            url,
                            title,
                            source: clean_source.to_string(),
                            indexed_at,
                        });
                    }
                    None => {}
                }
            }

            let total_count = all_entries.len();
            let paginated = all_entries.into_iter().skip(offset).take(limit).collect();
            Ok((paginated, total_count))
        })
        .map_err(|e| SearchXyzError::IndexError(format!("Failed to list indexed documents: {e}")))
    }

    /// SQLite WAL makes commits immediately visible; retained as a zero-cost compatibility hook.
    pub fn reload(&self) -> Result<(), SearchXyzError> {
        Ok(())
    }

    /// Delete a document by URL and any of its associated `#chunk-*` entries.
    pub async fn delete_document(&self, url: &str) -> Result<(), SearchXyzError> {
        let base_url = url.trim().to_string();
        let chunk_like = format!("{base_url}#%");
        let scope_like = self.scope_sql_prefix();

        let _lock = get_db_mutex().lock().await;
        with_shared_db(|conn| {
            let tx = conn.transaction()?;
            delete_url_rows_tx(&tx, &base_url, &chunk_like, scope_like.as_deref())?;
            tx.commit()?;
            Ok(())
        })
        .map_err(|e| SearchXyzError::IndexError(format!("Failed to delete document `{url}`: {e}")))
    }
}

fn delete_url_rows_tx(
    tx: &rusqlite::Transaction<'_>,
    base_url: &str,
    chunk_like: &str,
    scope_like: Option<&str>,
) -> anyhow::Result<()> {
    tx.execute(
        "DELETE FROM research_archive_fts
         WHERE entry_id IN (
             SELECT id FROM research_archive
             WHERE (url = ?1 OR url LIKE ?2)
               AND (?3 IS NULL OR source LIKE ?3)
         )",
        params![base_url, chunk_like, scope_like],
    )?;
    tx.execute(
        "DELETE FROM research_archive
         WHERE (url = ?1 OR url LIKE ?2)
           AND (?3 IS NULL OR source LIKE ?3)",
        params![base_url, chunk_like, scope_like],
    )?;
    Ok(())
}

fn build_snippet(content: &str) -> String {
    content
        .chars()
        .take(250)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn chunk_content(content: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current_section = String::new();
    let mut has_headers = false;

    for line in content.lines() {
        let is_header =
            line.starts_with("# ") || line.starts_with("## ") || line.starts_with("### ");
        match is_header {
            true => {
                has_headers = true;
                match current_section.trim().is_empty() {
                    true => {}
                    false => sections.push(current_section.trim().to_string()),
                }
                current_section = line.to_string();
                current_section.push('\n');
            }
            false => {
                current_section.push_str(line);
                current_section.push('\n');
            }
        }
    }
    match current_section.trim().is_empty() {
        true => {}
        false => sections.push(current_section.trim().to_string()),
    }

    match has_headers {
        false => return sliding_window_chunk(content, chunk_size, overlap),
        true => {}
    }

    let mut chunks = Vec::new();
    for section in sections {
        match section.chars().count() > chunk_size {
            true => chunks.extend(sliding_window_chunk(&section, chunk_size, overlap)),
            false => chunks.push(section),
        }
    }
    chunks
}

fn sliding_window_chunk(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();

    match chars.is_empty() {
        true => return chunks,
        false => {}
    }

    let mut start = 0;
    while start < chars.len() {
        let end = std::cmp::min(start + chunk_size, chars.len());
        let chunk: String = chars[start..end].iter().collect();
        let chunk_trimmed = chunk.trim().to_string();
        match chunk_trimmed.is_empty() {
            true => {}
            false => chunks.push(chunk_trimmed),
        }

        match end == chars.len() {
            true => break,
            false => {}
        }

        let step = match chunk_size > overlap {
            true => chunk_size - overlap,
            false => 1,
        };
        start += step;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::searchxyz::core::config::IndexConfig;

    #[tokio::test]
    async fn test_semantic_and_keyword_search() {
        let test_dir =
            std::env::temp_dir().join(format!("searchxyz_test_{}", rand::random::<u64>()));
        let config = IndexConfig {
            path: test_dir,
            writer_heap_bytes: 0,
            embedding: Default::default(),
        };

        let index = SearchIndex::open(&config).unwrap();

        let doc1 = ExtractedContent {
            url: "https://example.com/quantum".to_string(),
            title: "Quantum Computing Foundations".to_string(),
            description: String::new(),
            content_markdown: "Quantum mechanics describes the physical properties of nature at the scale of atoms and subatomic particles.".to_string(),
            links: vec![],
        };

        let doc2 = ExtractedContent {
            url: "https://example.com/baking".to_string(),
            title: "How to Bake Bread".to_string(),
            description: String::new(),
            content_markdown: "To bake delicious bread at home, you need flour, water, yeast, and salt. Kneading the dough is crucial.".to_string(),
            links: vec![],
        };

        index.add_document(&doc1, "test").await.unwrap();
        index.add_document(&doc2, "test").await.unwrap();

        let kw_results = index.search("bread", 5).unwrap();
        assert!(!kw_results.is_empty());
        assert_eq!(kw_results[0].title, "How to Bake Bread");

        let hyb_results = index
            .search_hybrid("Quantum subatomic particles", 5, ResearchSearchMode::Hybrid)
            .await
            .unwrap();
        assert!(!hyb_results.is_empty());
        assert_eq!(hyb_results[0].title, "Quantum Computing Foundations");
    }

    #[tokio::test]
    async fn test_list_documents() {
        let test_dir =
            std::env::temp_dir().join(format!("searchxyz_test_list_{}", rand::random::<u64>()));
        let config = IndexConfig {
            path: test_dir,
            writer_heap_bytes: 0,
            embedding: Default::default(),
        };

        let index = SearchIndex::open(&config).unwrap();

        let doc1 = ExtractedContent {
            url: "https://example.com/a".to_string(),
            title: "Doc A".to_string(),
            description: String::new(),
            content_markdown: "Some content".to_string(),
            links: vec![],
        };

        let doc2 = ExtractedContent {
            url: "https://example.com/b".to_string(),
            title: "Doc B".to_string(),
            description: String::new(),
            content_markdown: "Some other content".to_string(),
            links: vec![],
        };

        index.add_document(&doc1, "sourcea").await.unwrap();
        index.add_document(&doc2, "sourceb").await.unwrap();

        let (all_docs, count) = index.list_documents(None, 5, 0).unwrap();
        assert_eq!(count, 2);
        assert_eq!(all_docs.len(), 2);

        let (filtered_docs, filtered_count) = index.list_documents(Some("sourcea"), 5, 0).unwrap();
        assert_eq!(filtered_count, 1);
        assert_eq!(filtered_docs[0].title, "Doc A");
    }

    #[tokio::test]
    async fn test_markdown_chunking_logic() {
        let content =
            "Paragraph 1\n\n# Header 1\nSection 1 text here.\n\n## Header 2\nSection 2 text here.";
        let chunks = chunk_content(content, 1500, 200);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].contains("Paragraph 1"));
        assert!(chunks[1].contains("# Header 1"));
        assert!(chunks[2].contains("## Header 2"));
    }

    #[tokio::test]
    async fn test_prefix_deletion() {
        let test_dir = std::env::temp_dir().join(format!(
            "searchxyz_test_prefix_del_{}",
            rand::random::<u64>()
        ));
        let config = IndexConfig {
            path: test_dir,
            writer_heap_bytes: 0,
            embedding: Default::default(),
        };
        let index = SearchIndex::open(&config).unwrap();

        let doc = ExtractedContent {
            url: "https://example.com/prefix-del".to_string(),
            title: "Prefix Del".to_string(),
            description: String::new(),
            content_markdown: "Doc chunk 1\n\n# Header\nDoc chunk 2".to_string(),
            links: vec![],
        };
        let sibling = ExtractedContent {
            url: "https://example.com/prefix-del-keep".to_string(),
            title: "Prefix Keep".to_string(),
            description: String::new(),
            content_markdown: "Sibling content remains intact.".to_string(),
            links: vec![],
        };
        index.add_document(&doc, "test").await.unwrap();
        index.add_document(&sibling, "test").await.unwrap();

        index
            .delete_document("https://example.com/prefix-del")
            .await
            .unwrap();

        let results = index.search("chunk", 10).unwrap();
        assert_eq!(results.len(), 0);
        let kept = index.search("Sibling", 10).unwrap();
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn test_sliding_window_fallback() {
        let content: String = (0..3500).map(|i| (b'a' + (i % 26) as u8) as char).collect();
        let chunks = chunk_content(&content, 1500, 200);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].chars().count(), 1500);
        assert_eq!(chunks[1].chars().count(), 1500);
        assert_eq!(chunks[2].chars().count(), 900);

        let chars0: Vec<char> = chunks[0].chars().collect();
        let chars1: Vec<char> = chunks[1].chars().collect();
        let chars2: Vec<char> = chunks[2].chars().collect();

        assert_eq!(&chars0[1300..1500], &chars1[0..200]);
        assert_eq!(&chars1[1300..1500], &chars2[0..200]);
    }

    #[test]
    fn test_section_splitting_large() {
        let section_text = "b".repeat(2000);
        let content = format!("# Header\n{}", section_text);
        let chunks = chunk_content(&content, 1500, 200);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chars().count(), 1500);
        assert_eq!(chunks[1].chars().count(), 709);
        assert!(chunks[0].starts_with("# Header"));

        let chars0: Vec<char> = chunks[0].chars().collect();
        let chars1: Vec<char> = chunks[1].chars().collect();
        assert_eq!(&chars0[1300..1500], &chars1[0..200]);
    }
}
