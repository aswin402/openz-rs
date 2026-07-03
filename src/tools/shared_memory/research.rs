use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use rusqlite::params;

use super::db::{get_db_mutex, with_db};
use super::embeddings::{get_embedding, get_cloud_embeddings_batch, get_global_model, cosine_similarity};

pub fn chunk_content_by_headings(query: &str, content: &str) -> Vec<(String, String)> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    let mut current_heading = String::new();
    let mut current_chunk = Vec::new();
    let mut current_len = 0;
    
    for line in lines {
        let is_heading = line.trim_start().starts_with('#') 
            || line.trim_start().starts_with("--- Sheet:")
            || line.trim_start().starts_with("Title:");
        
        if is_heading && !current_chunk.is_empty() {
            let chunk_text = current_chunk.join("\n");
            let chunk_query = if current_heading.is_empty() {
                query.to_string()
            } else {
                format!("{} - {}", query, current_heading)
            };
            chunks.push((chunk_query, chunk_text));
            current_chunk.clear();
            current_len = 0;
        }
        
        if is_heading {
            current_heading = line.trim().to_string();
        }
        
        current_chunk.push(line);
        current_len += line.len();
        
        if current_len > 2500 {
            let chunk_text = current_chunk.join("\n");
            let chunk_query = if current_heading.is_empty() {
                query.to_string()
            } else {
                format!("{} - {}", query, current_heading)
            };
            chunks.push((chunk_query, chunk_text));
            current_chunk.clear();
            current_len = 0;
        }
    }
    
    if !current_chunk.is_empty() {
        let chunk_text = current_chunk.join("\n");
        let chunk_query = if current_heading.is_empty() {
            query.to_string()
        } else {
            format!("{} - {}", query, current_heading)
        };
        chunks.push((chunk_query, chunk_text));
    }
    
    if chunks.is_empty() {
        chunks.push((query.to_string(), content.to_string()));
    }
    
    chunks
}

pub async fn archive_research_entry(query: &str, content: &str, source: &str) -> Result<()> {
    let chunks = chunk_content_by_headings(query, content);
    for (chunk_query, chunk_content) in chunks {
        let embedding = get_embedding(&chunk_query, false).await.unwrap_or_else(|e| {
            eprintln!("Failed to generate embedding for research archive chunk: {:?}", e);
            Vec::new()
        });

        if embedding.is_empty() {
            eprintln!("Skipping research archive chunk (empty embedding): {}", chunk_query);
            continue;
        }

        let timestamp = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        let embedding_json = serde_json::to_string(&embedding)?;

        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO research_archive (id, query, content, source, timestamp, embedding)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, chunk_query, chunk_content, source, timestamp, embedding_json],
            )?;
            Ok(())
        })?;
    }
    Ok(())
}

pub async fn archive_research_entries(entries: Vec<(String, String, String)>) -> Result<()> {
    let mut all_chunks = Vec::new();
    for (query, content, source) in entries {
        let chunks = chunk_content_by_headings(&query, &content);
        for (chunk_query, chunk_content) in chunks {
            all_chunks.push((chunk_query, chunk_content, source.clone()));
        }
    }

    if all_chunks.is_empty() {
        return Ok(());
    }

    let config = crate::config::loader::load_config().ok();
    let mode = config.as_ref()
        .and_then(|c| c.embeddings.as_ref())
        .map(|e| e.mode.as_str())
        .unwrap_or("local");

    let mut all_embeddings = Vec::new();
    for chunk_group in all_chunks.chunks(128) {
        let queries_to_embed: Vec<String> = chunk_group.iter().map(|(q, _, _)| q.clone()).collect();
        
        let mut embeds = None;
        if mode != "local" {
            match get_cloud_embeddings_batch(queries_to_embed.clone(), false).await {
                Ok(res) => {
                    embeds = Some(res);
                }
                Err(e) => {
                    if mode == "cloud_only" {
                        return Err(anyhow::anyhow!("Cloud batch embedding failed and local model fallback is disabled: {:?}", e));
                    }
                    tracing::warn!("Cloud batch embedding failed: {:?}. Falling back to local fastembed.", e);
                }
            }
        }

        let embeds = match embeds {
            Some(res) => res,
            None => {
                tokio::task::spawn_blocking(move || -> Result<Vec<Vec<f32>>> {
                    let model_mutex = get_global_model()?;
                    let mut model = model_mutex.lock().map_err(|e| anyhow!("Failed to lock model Mutex: {:?}", e))?;
                    
                    let refs: Vec<&str> = queries_to_embed.iter().map(|s| s.as_str()).collect();
                    let formatted_refs: Vec<String> = refs.iter().map(|s| format!("passage: {}", s)).collect();
                    let formatted_slices: Vec<&str> = formatted_refs.iter().map(|s| s.as_str()).collect();
                    
                    let embeds = model.embed(formatted_slices, None)?;
                    Ok(embeds)
                }).await??
            }
        };
        
        all_embeddings.extend(embeds);
    }

    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        let tx = conn.transaction()?;
        let timestamp = chrono::Utc::now().to_rfc3339();

        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO research_archive (id, query, content, source, timestamp, embedding)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            )?;

            for (idx, (chunk_query, chunk_content, source)) in all_chunks.into_iter().enumerate() {
                let embedding = &all_embeddings[idx];
                let id = uuid::Uuid::new_v4().to_string();
                let embedding_json = serde_json::to_string(embedding)?;

                stmt.execute(params![
                    id,
                    chunk_query,
                    chunk_content,
                    source,
                    timestamp,
                    embedding_json
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    })?;
    Ok(())
}

pub async fn search_research_entries(query: &str, top_k: usize) -> Result<Value> {
    let query_embed = get_embedding(query, true).await.unwrap_or_default();
    let query_lower = query.to_lowercase();

    let _lock = get_db_mutex().lock().await;
    let entries = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT id, query, content, source, timestamp, embedding FROM research_archive LIMIT 1000")?;
        let mapped = stmt.query_map([], |row| {
            let embedding_str: String = row.get(5)?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_str).unwrap_or_default();
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "query": row.get::<_, String>(1)?,
                "content": row.get::<_, String>(2)?,
                "source": row.get::<_, String>(3)?,
                "timestamp": row.get::<_, String>(4)?,
                "embedding": embedding,
            }))
        })?;
        let mut collected = Vec::new();
        for item in mapped {
            collected.push(item?);
        }
        Ok(collected)
    })?;

    let mut scored_results = Vec::new();
    for entry in entries {
        let entry_query = entry["query"].as_str().unwrap_or_default();
        let entry_content = entry["content"].as_str().unwrap_or_default();
        let entry_embed: Vec<f32> = serde_json::from_value(entry["embedding"].clone()).unwrap_or_default();

        let sim = if !query_embed.is_empty() && !entry_embed.is_empty() {
            cosine_similarity(&query_embed, &entry_embed)
        } else {
            0.0
        };

        let query_lower_entry = entry_query.to_lowercase();
        let content_lower_entry = entry_content.to_lowercase();
        let keyword_match = query_lower_entry.contains(&query_lower) || content_lower_entry.contains(&query_lower) || query_lower.contains(&query_lower_entry);

        let final_score = if keyword_match {
            sim * 0.7 + 0.3
        } else {
            sim * 0.7
        };

        if final_score > 0.15 || keyword_match {
            scored_results.push((final_score, entry));
        }
    }

    scored_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let selected: Vec<Value> = scored_results.into_iter()
        .take(top_k)
        .map(|(score, mut entry)| {
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("score".to_string(), json!(score));
                obj.remove("embedding");
            }
            entry
        })
        .collect();

    Ok(Value::Array(selected))
}

// 4. ArchiveResearchTool
pub struct ArchiveResearchTool;

#[async_trait::async_trait]
impl Tool for ArchiveResearchTool {
    fn name(&self) -> &str {
        "archive_research"
    }

    fn description(&self) -> &str {
        "Archive successful research results (e.g., scrape content, web searches, codebase mappings) to the local persistent knowledge base cache."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query, search topic, or file path describing the research."
                },
                "content": {
                    "type": "string",
                    "description": "The full plain text content or findings to archive."
                },
                "source": {
                    "type": "string",
                    "description": "The source of the research content (e.g. 'web_fetch: URL', 'web_search', 'local_file')."
                }
            },
            "required": ["query", "content", "source"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments.get("query").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;
        let content = arguments.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'content' parameter"))?;
        let source = arguments.get("source").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'source' parameter"))?;

        archive_research_entry(query, content, source).await?;

        Ok(json!({
            "status": "success",
            "message": "Research content archived successfully to the local knowledge base."
        }))
    }
}

// 5. SearchResearchTool
pub struct SearchResearchTool;

#[async_trait::async_trait]
impl Tool for SearchResearchTool {
    fn name(&self) -> &str {
        "search_research"
    }

    fn description(&self) -> &str {
        "Search the local persistent research archive (knowledge base) using semantic similarity and keyword matching before attempting external web searches or page scraping."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query or search term to look up in the archive (e.g. error message, library docs, command usage)."
                },
                "top_k": {
                    "type": "integer",
                    "description": "Optional number of top matches to return (default 5)."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments.get("query").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;
        let top_k = arguments.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        let matches = search_research_entries(query, top_k).await?;

        Ok(json!({
            "status": "success",
            "matches": matches
        }))
    }
}
