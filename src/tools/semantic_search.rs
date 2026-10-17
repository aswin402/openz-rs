use crate::memory::{MemoryDatabase, MemoryService};
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
    let chars: Vec<char> = text.chars().collect();
    let step = chunk_size.saturating_sub(overlap).max(1);
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = std::cmp::min(start + chunk_size, chars.len());
        let chunk: String = chars[start..end].iter().collect();
        chunks.push(chunk);
        match end == chars.len() {
            true => break,
            false => start += step,
        }
    }
    chunks
}

fn should_skip_index_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    ["/.git", "/target", "/node_modules", "/.fastembed_cache"]
        .iter()
        .any(|needle| path_str.contains(needle))
}

fn get_files_recursively(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    match (path.is_file(), path.is_dir()) {
        (true, _) => files.push(path.to_path_buf()),
        (false, true) => {
            for entry in fs::read_dir(path)? {
                let entry_path = entry?.path();
                match should_skip_index_path(&entry_path) {
                    true => {}
                    false => get_files_recursively(&entry_path, files)?,
                }
            }
        }
        (false, false) => {}
    }
    Ok(())
}

fn get_db_conn() -> Result<rusqlite::Connection> {
    let db_path = MemoryService::current().database_path(MemoryDatabase::EmbeddingCache);
    match db_path.parent() {
        Some(parent) => {
            let _ = fs::create_dir_all(parent);
        }
        None => {}
    }
    let conn = rusqlite::Connection::open(&db_path)?;
    crate::core::sqlite::apply_standard_pragmas_with_extra(&conn, "PRAGMA foreign_keys=ON;")?;
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

    let to_delete: Vec<String> = rows
        .flatten()
        .filter(|path_str| !Path::new(path_str).exists())
        .collect();

    match to_delete.is_empty() {
        true => {}
        false => {
            let mut del_stmt = conn.prepare("DELETE FROM file_cache WHERE file_path = ?1")?;
            for path_str in to_delete {
                let _ = del_stmt.execute([path_str]);
            }
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
        let memory = MemoryService::current();
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
        for p_str in paths_val.iter().filter_map(Value::as_str) {
            let resolved = crate::config::resolve_path(p_str);
            let _ = get_files_recursively(&resolved, &mut files);
        }

        match files.is_empty() {
            true => return Err(anyhow!("No files found to index in the specified paths.")),
            false => {}
        }

        let mut dirty_files = Vec::new();
        let mut final_chunks = Vec::new();

        // 1. Separate files into cached and dirty (missing or changed mtime)
        // Scope the database connection and statements so they are dropped before the await point
        {
            let conn = get_db_conn()?;
            let _ = prune_deleted_files(&conn);

            let mut mtime_stmt =
                conn.prepare("SELECT mtime_secs FROM file_cache WHERE file_path = ?1")?;
            let mut chunks_stmt = conn.prepare(
                "SELECT chunk_index, chunk_text, embedding FROM chunk_cache WHERE file_path = ?1",
            )?;

            for file_path in &files {
                let path_str = file_path.to_string_lossy().to_string();
                let mtime_secs = match fs::metadata(file_path).and_then(|m| m.modified()) {
                    Ok(time) => time
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    Err(_) => 0,
                };

                let is_cached = mtime_stmt
                    .query([&path_str])
                    .ok()
                    .and_then(|mut rows| {
                        rows.next()
                            .ok()
                            .flatten()
                            .and_then(|row| row.get::<_, u64>(0).ok())
                    })
                    .is_some_and(|cached_mtime| cached_mtime == mtime_secs);

                match is_cached {
                    true => {
                        match chunks_stmt.query_map([&path_str], |row| {
                            let idx: usize = row.get(0)?;
                            let text: String = row.get(1)?;
                            let bytes: Vec<u8> = row.get(2)?;

                            let embedding: Vec<f32> = bytes
                                .chunks_exact(4)
                                .map(|chunk| {
                                    let array: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                                    f32::from_ne_bytes(array)
                                })
                                .collect();
                            Ok(ChunkRef {
                                file_path: path_str.clone(),
                                text,
                                index: idx,
                                embedding,
                            })
                        }) {
                            Ok(rows) => {
                                for chunk in rows.flatten() {
                                    final_chunks.push(chunk);
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    false => {
                        match fs::read_to_string(file_path)
                            .ok()
                            .filter(|text| !text.trim().is_empty() && text.len() <= 1024 * 1024)
                            .map(|text| chunk_text(&text, 600, 100))
                            .filter(|file_chunks| !file_chunks.is_empty())
                        {
                            Some(file_chunks) => {
                                dirty_files.push((path_str, mtime_secs, file_chunks));
                            }
                            None => {}
                        }
                    }
                }
            }
        }

        // 2. Spawn blocking task to generate embeddings for query and new chunks
        let query_owned = query.to_string();
        let query_prefixed = format!("query: {}", query_owned);

        let dirty_texts: Vec<String> = dirty_files
            .iter()
            .flat_map(|(_, _, chunks)| chunks.iter().map(|text| format!("passage: {}", text)))
            .collect();

        let embedding_service = memory.clone();
        let (query_vec, new_embeds) =
            tokio::task::spawn_blocking(move || -> Result<(Vec<f32>, Vec<Vec<f32>>)> {
                embedding_service.with_local_model(|model| {
                    let query_embeds = model.embed(vec![&query_prefixed], None)?;
                    let q_vec = query_embeds[0].clone();

                    let p_embeds = match dirty_texts.is_empty() {
                        true => Vec::new(),
                        false => {
                            let refs: Vec<&str> = dirty_texts.iter().map(|s| s.as_str()).collect();
                            model.embed(refs, None)?
                        }
                    };

                    Ok((q_vec, p_embeds))
                })
            })
            .await??;

        // 3. Merge new embeddings back into cache and final list
        match dirty_files.is_empty() {
            true => {}
            false => {
                let mut tx_conn = get_db_conn()?;
                let tx = tx_conn.transaction()?;

                let mut embed_idx = 0;
                for (path_str, mtime_secs, chunks) in dirty_files {
                    tx.execute("DELETE FROM file_cache WHERE file_path = ?1", [&path_str])?;
                    tx.execute(
                        "INSERT INTO file_cache (file_path, mtime_secs) VALUES (?1, ?2)",
                        rusqlite::params![&path_str, mtime_secs],
                    )?;

                    for (idx, text) in chunks.into_iter().enumerate() {
                        let embedding = new_embeds[embed_idx].clone();
                        embed_idx += 1;

                        let bytes: Vec<u8> = embedding
                            .iter()
                            .flat_map(|val| val.to_ne_bytes())
                            .collect();

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
        }

        match final_chunks.is_empty() {
            true => {
                return Err(anyhow!(
                    "Could not extract any readable text chunks from files."
                ));
            }
            false => {}
        }

        // 4. Calculate similarity scores and sort
        let mut results: Vec<(f32, &ChunkRef)> = final_chunks
            .iter()
            .map(|chunk| (memory.cosine_similarity(&query_vec, &chunk.embedding), chunk))
            .collect();

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let matches: Vec<Value> = results
            .into_iter()
            .take(top_k)
            .map(|(sim, chunk)| {
                json!({
                    "file_path": chunk.file_path,
                    "chunk_index": chunk.index,
                    "score": sim,
                    "text": chunk.text
                })
            })
            .collect();

        Ok(json!({
            "status": "success",
            "matches": matches
        }))
    }
}

#[cfg(test)]
#[path = "semantic_search_tests.rs"]
mod tests;

