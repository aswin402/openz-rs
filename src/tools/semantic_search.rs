use crate::tools::Tool;
use anyhow::{anyhow, Result};

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub struct SemanticSearchTool;

#[derive(Clone)]
struct ChunkRef {
    file_path: String,
    text: String,
    index: usize,
    embedding: Vec<f32>,
}

fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut start = 0;
    while start < chars.len() {
        let end = std::cmp::min(start + chunk_size, chars.len());
        let chunk: String = chars[start..end].iter().collect();
        chunks.push(chunk);
        if end == chars.len() {
            break;
        }
        start += chunk_size - overlap;
    }
    chunks
}

fn get_files_recursively(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_file() {
        files.push(path.to_path_buf());
    } else if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            // Skip common build/vcs directories to be efficient
            let path_str = entry_path.to_string_lossy();
            if path_str.contains("/.git")
                || path_str.contains("/target")
                || path_str.contains("/node_modules")
                || path_str.contains("/.fastembed_cache")
            {
                continue;
            }

            get_files_recursively(&entry_path, files)?;
        }
    }
    Ok(())
}

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for i in 0..v1.len() {
        dot_product += v1[i] * v2[i];
        norm_a += v1[i] * v1[i];
        norm_b += v2[i] * v2[i];
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a.sqrt() * norm_b.sqrt())
    }
}

fn get_db_conn() -> Result<rusqlite::Connection> {
    let db_path = crate::config::loader::runtime_db_path("embeddings_cache.db");
    if let Some(parent) = db_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let conn = rusqlite::Connection::open(&db_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS file_cache (
            file_path TEXT PRIMARY KEY,
            mtime_secs INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS chunk_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            chunk_text TEXT NOT NULL,
            embedding BLOB NOT NULL,
            FOREIGN KEY(file_path) REFERENCES file_cache(file_path) ON DELETE CASCADE
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_chunk_file_path ON chunk_cache(file_path)",
        [],
    )?;
    Ok(conn)
}

fn prune_deleted_files(conn: &rusqlite::Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT file_path FROM file_cache")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    
    let mut to_delete = Vec::new();
    for row in rows {
        if let Ok(path_str) = row {
            if !Path::new(&path_str).exists() {
                to_delete.push(path_str);
            }
        }
    }

    if !to_delete.is_empty() {
        let mut del_stmt = conn.prepare("DELETE FROM file_cache WHERE file_path = ?1")?;
        for path_str in to_delete {
            let _ = del_stmt.execute([path_str]);
        }
    }
    Ok(())
}

#[async_trait::async_trait]
impl Tool for SemanticSearchTool {
    fn name(&self) -> &str {
        "semantic_search"
    }

    fn description(&self) -> &str {
        "Perform a local semantic vector search over a list of files or directories using local embeddings. Ideal for codebase Q&A."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query (e.g. 'how does authentication work?')."
                },
                "paths": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "List of files or directories to index and search."
                },
                "top_k": {
                    "type": "integer",
                    "description": "Optional number of top matching segments to return (default 5)."
                }
            },
            "required": ["query", "paths"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;

        let paths_val = arguments
            .get("paths")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("Missing 'paths' parameter"))?;

        let top_k = arguments.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        let mut files = Vec::new();
        for path_val in paths_val {
            if let Some(p_str) = path_val.as_str() {
                let resolved = crate::config::resolve_path(p_str);
                let _ = get_files_recursively(&resolved, &mut files);
            }
        }

        if files.is_empty() {
            return Err(anyhow!("No files found to index in the specified paths."));
        }

        let mut dirty_files = Vec::new();
        let mut final_chunks = Vec::new();

        // 1. Separate files into cached and dirty (missing or changed mtime)
        // Scope the database connection and statements so they are dropped before the await point
        {
            let conn = get_db_conn()?;
            let _ = prune_deleted_files(&conn);

            let mut mtime_stmt = conn.prepare("SELECT mtime_secs FROM file_cache WHERE file_path = ?1")?;
            let mut chunks_stmt = conn.prepare("SELECT chunk_index, chunk_text, embedding FROM chunk_cache WHERE file_path = ?1")?;

            for file_path in &files {
                let path_str = file_path.to_string_lossy().to_string();
                let mtime_secs = match fs::metadata(file_path).and_then(|m| m.modified()) {
                    Ok(time) => time
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    Err(_) => 0,
                };

                let mut is_cached = false;
                if let Ok(mut rows) = mtime_stmt.query([&path_str]) {
                    if let Ok(Some(row)) = rows.next() {
                        let cached_mtime: u64 = row.get(0)?;
                        if cached_mtime == mtime_secs {
                            is_cached = true;
                        }
                    }
                }

                if is_cached {
                    if let Ok(rows) = chunks_stmt.query_map([&path_str], |row| {
                        let idx: usize = row.get(0)?;
                        let text: String = row.get(1)?;
                        let bytes: Vec<u8> = row.get(2)?;
                        
                        let mut embedding = Vec::with_capacity(bytes.len() / 4);
                        for chunk in bytes.chunks_exact(4) {
                            let array: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                            embedding.push(f32::from_ne_bytes(array));
                        }
                        Ok(ChunkRef {
                            file_path: path_str.clone(),
                            text,
                            index: idx,
                            embedding,
                        })
                    }) {
                        for chunk_res in rows {
                            if let Ok(chunk) = chunk_res {
                                final_chunks.push(chunk);
                            }
                        }
                    }
                } else {
                    if let Ok(text) = fs::read_to_string(file_path) {
                        if !text.trim().is_empty() && text.len() <= 1024 * 1024 {
                            let file_chunks = chunk_text(&text, 600, 100);
                            if !file_chunks.is_empty() {
                                dirty_files.push((path_str, mtime_secs, file_chunks));
                            }
                        }
                    }
                }
            }
        }

        // 2. Spawn blocking task to generate embeddings for query and new chunks
        let query_owned = query.to_string();
        let query_prefixed = format!("query: {}", query_owned);

        let mut dirty_texts = Vec::new();
        for (_, _, chunks) in &dirty_files {
            for text in chunks {
                dirty_texts.push(format!("passage: {}", text));
            }
        }

        let (query_vec, new_embeds) =
            tokio::task::spawn_blocking(move || -> Result<(Vec<f32>, Vec<Vec<f32>>)> {
                let model_mutex = crate::tools::shared_memory::get_global_model()?;
                let mut model = model_mutex
                    .lock()
                    .map_err(|e| anyhow!("Failed to lock model Mutex: {:?}", e))?;

                // Embed Query
                let query_embeds = model.embed(vec![&query_prefixed], None)?;
                let q_vec = query_embeds[0].clone();

                // Embed New Passages if any
                let p_embeds = if !dirty_texts.is_empty() {
                    let refs: Vec<&str> = dirty_texts.iter().map(|s| s.as_str()).collect();
                    model.embed(refs, None)?
                } else {
                    Vec::new()
                };

                Ok((q_vec, p_embeds))
            })
            .await??;

        // 3. Merge new embeddings back into cache and final list
        if !dirty_files.is_empty() {
            let mut tx_conn = get_db_conn()?;
            let tx = tx_conn.transaction()?;

            let mut embed_idx = 0;
            for (path_str, mtime_secs, chunks) in dirty_files {
                tx.execute("DELETE FROM file_cache WHERE file_path = ?1", [&path_str])?;
                tx.execute("INSERT INTO file_cache (file_path, mtime_secs) VALUES (?1, ?2)", rusqlite::params![&path_str, mtime_secs])?;

                for (idx, text) in chunks.into_iter().enumerate() {
                    let embedding = new_embeds[embed_idx].clone();
                    embed_idx += 1;

                    let mut bytes = Vec::with_capacity(embedding.len() * 4);
                    for val in &embedding {
                        bytes.extend_from_slice(&val.to_ne_bytes());
                    }

                    tx.execute(
                        "INSERT INTO chunk_cache (file_path, chunk_index, chunk_text, embedding) VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![&path_str, idx, &text, bytes],
                    )?;

                    final_chunks.push(ChunkRef {
                        file_path: path_str.clone(),
                        text,
                        index: idx,
                        embedding,
                    });
                }
            }
            tx.commit()?;
        }

        if final_chunks.is_empty() {
            return Err(anyhow!(
                "Could not extract any readable text chunks from files."
            ));
        }

        // 4. Calculate similarity scores and sort
        let mut results = Vec::new();
        for chunk in &final_chunks {
            let similarity = cosine_similarity(&query_vec, &chunk.embedding);
            results.push((similarity, chunk));
        }

        // Sort descending by similarity
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut matches = Vec::new();
        for (sim, chunk) in results.into_iter().take(top_k) {
            matches.push(json!({
                "file_path": chunk.file_path,
                "chunk_index": chunk.index,
                "score": sim,
                "text": chunk.text
            }));
        }

        Ok(json!({
            "status": "success",
            "matches": matches
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_cache_storage_and_pruning() {
        let temp_dir = std::env::temp_dir().join(format!("openz_embed_cache_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let _db_path = temp_dir.join("embeddings_cache.db");
        
        let conn = crate::config::loader::CONFIG_DIR_OVERRIDE.scope(temp_dir.clone(), async {
            get_db_conn().unwrap()
        }).await;

        let fake_file = temp_dir.join("test_file.txt");
        std::fs::write(&fake_file, b"hello world").unwrap();
        let path_str = fake_file.to_string_lossy().to_string();

        conn.execute(
            "INSERT INTO file_cache (file_path, mtime_secs) VALUES (?1, ?2)",
            rusqlite::params![&path_str, 12345u64],
        ).unwrap();

        let dummy_emb = vec![0.1f32, 0.2f32, 0.3f32];
        let mut bytes = Vec::new();
        for val in &dummy_emb {
            bytes.extend_from_slice(&val.to_ne_bytes());
        }

        conn.execute(
            "INSERT INTO chunk_cache (file_path, chunk_index, chunk_text, embedding) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![&path_str, 0usize, "hello world", bytes],
        ).unwrap();

        let mut stmt = conn.prepare("SELECT chunk_index, chunk_text, embedding FROM chunk_cache WHERE file_path = ?1").unwrap();
        let mut rows = stmt.query_map([&path_str], |row: &rusqlite::Row<'_>| {
            let idx: usize = row.get(0)?;
            let text: String = row.get(1)?;
            let bytes: Vec<u8> = row.get(2)?;
            let mut embedding = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let array: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                embedding.push(f32::from_ne_bytes(array));
            }
            Ok((idx, text, embedding))
        }).unwrap();

        let (idx, text, emb) = rows.next().unwrap().unwrap();
        assert_eq!(idx, 0);
        assert_eq!(text, "hello world");
        assert_eq!(emb, dummy_emb);

        std::fs::remove_file(&fake_file).unwrap();

        prune_deleted_files(&conn).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM file_cache", [], |row: &rusqlite::Row<'_>| row.get(0)).unwrap();
        assert_eq!(count, 0);

        let chunk_count: i64 = conn.query_row("SELECT COUNT(*) FROM chunk_cache", [], |row: &rusqlite::Row<'_>| row.get(0)).unwrap();
        assert_eq!(chunk_count, 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
