use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

pub struct SemanticSearchTool;

#[derive(Clone)]
struct Chunk {
    file_path: String,
    text: String,
    index: usize,
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
            if path_str.contains("/.git") || 
               path_str.contains("/target") || 
               path_str.contains("/node_modules") || 
               path_str.contains("/.fastembed_cache") {
                continue;
            }

            get_files_recursively(&entry_path, files)?;
        }
    }
    Ok(())
}

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
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
        let query = arguments.get("query").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;
        
        let paths_val = arguments.get("paths").and_then(|v| v.as_array())
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

        // Chunk files
        let mut chunks = Vec::new();
        for file_path in &files {
            if let Ok(text) = fs::read_to_string(file_path) {
                // Ignore empty or extremely large files
                if text.trim().is_empty() || text.len() > 1024 * 1024 {
                    continue;
                }
                let file_chunks = chunk_text(&text, 600, 100);
                for (idx, ch_text) in file_chunks.into_iter().enumerate() {
                    chunks.push(Chunk {
                        file_path: file_path.to_string_lossy().to_string(),
                        text: ch_text,
                        index: idx,
                    });
                }
            }
        }

        if chunks.is_empty() {
            return Err(anyhow!("Could not extract any readable text chunks from files."));
        }

        // Run embeddings generation in spawn_blocking to keep Tokio responsive
        let query_owned = query.to_string();
        let res = tokio::task::spawn_blocking(move || -> Result<Value> {
            // Initialize fastembed model (BGE-small by default)
            let mut model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2))?;
            
            // Format inputs with prefix if model recommends
            let query_prefixed = format!("query: {}", query_owned);
            let query_embeds = model.embed(vec![&query_prefixed], None)?;
            let query_vec = &query_embeds[0];

            let passage_texts: Vec<String> = chunks.iter()
                .map(|c| format!("passage: {}", c.text))
                .collect();
            
            let passage_refs: Vec<&str> = passage_texts.iter()
                .map(|s| s.as_str())
                .collect();

            let embeds = model.embed(passage_refs, None)?;

            let mut results = Vec::new();
            for (idx, embed) in embeds.iter().enumerate() {
                let similarity = cosine_similarity(query_vec, embed);
                results.push((similarity, &chunks[idx]));
            }

            // Sort descending by similarity
            results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

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
        }).await??;

        Ok(res)
    }
}
