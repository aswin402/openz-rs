use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub struct SemanticSearchTool;

#[derive(Serialize, Deserialize, Clone)]
struct CachedChunk {
    index: usize,
    text: String,
    embedding: Vec<f32>,
}

#[derive(Serialize, Deserialize, Clone)]
struct CachedFile {
    mtime_secs: u64,
    chunks: Vec<CachedChunk>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct EmbeddingsCache {
    files: std::collections::HashMap<String, CachedFile>,
}

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

fn load_cache() -> EmbeddingsCache {
    let cache_path = crate::config::resolve_path("~/.openz/embeddings_cache.json");
    if cache_path.exists() {
        if let Ok(content) = fs::read_to_string(&cache_path) {
            if let Ok(cache) = serde_json::from_str::<EmbeddingsCache>(&content) {
                return cache;
            }
        }
    }
    EmbeddingsCache::default()
}

fn save_cache(cache: &EmbeddingsCache) -> Result<()> {
    let cache_path = crate::config::resolve_path("~/.openz/embeddings_cache.json");
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string(cache)?;
    fs::write(cache_path, content)?;
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

        let mut cache = load_cache();
        let mut cache_updated = false;

        // Prune deleted files from cache to prevent unbounded growth
        let initial_cache_len = cache.files.len();
        cache
            .files
            .retain(|path_str, _| Path::new(path_str).exists());
        if cache.files.len() != initial_cache_len {
            cache_updated = true;
        }

        let mut dirty_files = Vec::new();
        let mut final_chunks = Vec::new();

        // 1. Separate files into cached and dirty (missing or changed mtime)
        for file_path in &files {
            let path_str = file_path.to_string_lossy().to_string();
            let mtime_secs = match fs::metadata(file_path).and_then(|m| m.modified()) {
                Ok(time) => time
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                Err(_) => 0,
            };

            let is_cached = if let Some(cached_file) = cache.files.get(&path_str) {
                cached_file.mtime_secs == mtime_secs
            } else {
                false
            };

            if is_cached {
                if let Some(cached_file) = cache.files.get(&path_str) {
                    for chunk in &cached_file.chunks {
                        final_chunks.push(ChunkRef {
                            file_path: path_str.clone(),
                            text: chunk.text.clone(),
                            index: chunk.index,
                            embedding: chunk.embedding.clone(),
                        });
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
            let mut embed_idx = 0;
            for (path_str, mtime_secs, chunks) in dirty_files {
                let mut cached_chunks = Vec::new();
                for (idx, text) in chunks.into_iter().enumerate() {
                    let embedding = new_embeds[embed_idx].clone();
                    embed_idx += 1;

                    cached_chunks.push(CachedChunk {
                        index: idx,
                        text: text.clone(),
                        embedding: embedding.clone(),
                    });

                    final_chunks.push(ChunkRef {
                        file_path: path_str.clone(),
                        text,
                        index: idx,
                        embedding,
                    });
                }

                cache.files.insert(
                    path_str,
                    CachedFile {
                        mtime_secs,
                        chunks: cached_chunks,
                    },
                );
                cache_updated = true;
            }
        }

        if cache_updated {
            let _ = save_cache(&cache);
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

        // Sort descending by similarity — handle NaN by treating it as lowest value
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
