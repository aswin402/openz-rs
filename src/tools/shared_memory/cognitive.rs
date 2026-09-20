use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::memory::with_shared_db as with_db;
use super::db::{get_current_workspace, get_db_mutex};
use super::embeddings::{cosine_similarity, get_embedding};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CognitiveMemoryEntry {
    pub id: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub timestamp: String,
    pub workspace: String,
    pub tags: Vec<String>,
    pub importance: f32,
    pub last_accessed: String,
    pub access_count: i64,
    pub decay_rate: f32,
}

pub fn prune_decayed_memories(conn: &Connection) -> Result<usize> {
    let mut stmt =
        conn.prepare("SELECT id, importance, last_accessed, decay_rate FROM cognitive_memory")?;
    let now = chrono::Utc::now();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, f64>(3)?,
        ))
    })?;

    let mut to_delete = Vec::new();
    for r in rows.flatten() {
        let (id, importance, last_acc_str, decay_rate) = r;
        let last_acc_date = chrono::DateTime::parse_from_rfc3339(&last_acc_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        let duration = now.signed_duration_since(last_acc_date);
        let days_elapsed = duration.num_seconds() as f64 / 86400.0;

        let decay_factor = (-decay_rate * days_elapsed).exp();
        let decayed_importance = importance * decay_factor;

        if decayed_importance < 0.15 {
            to_delete.push(id);
        }
    }

    let count = to_delete.len();
    for id in to_delete {
        let _ = conn.execute("DELETE FROM cognitive_memory WHERE id = ?", [id]);
    }
    Ok(count)
}

// ─── Extraction Helpers ─────────────────────────────────────────

fn extract_string_field(val: &Value, keys: &[&str]) -> Option<String> {
    if let Some(s) = val.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    } else if val.is_number() || val.is_boolean() {
        let s = val.to_string();
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    for key in keys {
        if let Some(v) = val.get(*key) {
            if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            } else if v.is_number() || v.is_boolean() {
                let s = v.to_string();
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn extract_string_list_field(val: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(v) = val.get(*key) {
            if let Some(arr) = v.as_array() {
                let list: Vec<String> = arr
                    .iter()
                    .filter_map(|item| {
                        if let Some(s) = item.as_str() {
                            let trimmed = s.trim();
                            if !trimmed.is_empty() {
                                Some(trimmed.to_string())
                            } else {
                                None
                            }
                        } else if item.is_number() || item.is_boolean() {
                            Some(item.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !list.is_empty() {
                    return list;
                }
            } else if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    if trimmed.contains(',') {
                        return trimmed
                            .split(',')
                            .map(|part| part.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect();
                    } else {
                        return vec![trimmed.to_string()];
                    }
                }
            }
        }
    }
    Vec::new()
}

fn extract_f32_arg(val: &Value, keys: &[&str]) -> Option<f32> {
    for key in keys {
        if let Some(v) = val.get(*key) {
            if let Some(f) = v.as_f64() {
                return Some(f as f32);
            }
            if let Some(i) = v.as_i64() {
                return Some(i as f32);
            }
            if let Some(s) = v.as_str() {
                if let Ok(f) = s.trim().parse::<f32>() {
                    return Some(f);
                }
            }
        }
    }
    None
}

fn extract_usize_arg(val: &Value, keys: &[&str]) -> Option<usize> {
    for key in keys {
        if let Some(v) = val.get(*key) {
            if let Some(u) = v.as_u64() {
                return Some(u as usize);
            }
            if let Some(i) = v.as_i64() {
                if i >= 0 {
                    return Some(i as usize);
                }
            }
            if let Some(s) = v.as_str() {
                if let Ok(u) = s.trim().parse::<usize>() {
                    return Some(u);
                }
            }
        }
    }
    None
}

// 1. StoreMemoryTool
pub struct StoreMemoryTool;

#[async_trait::async_trait]
impl Tool for StoreMemoryTool {
    fn name(&self) -> &str {
        "store_memory"
    }

    fn description(&self) -> &str {
        "Store a fact, guideline, key decision, or code pattern in the cognitive memory bus with support for importance weighting and temporal decay."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "The description of the fact, key solution, API usage, or decision to remember (aliases: content, memory, fact)."
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional category tags or comma-separated string (e.g. ['auth', 'cargo'] or 'auth, cargo')."
                },
                "importance": {
                    "type": "number",
                    "description": "Optional importance score from 0.0 (low) to 1.0 (high) (default 0.8, aliases: priority, weight, score)."
                },
                "decay_rate": {
                    "type": "number",
                    "description": "Optional decay rate for time-based forgetting (default 0.05, alias: decayRate)."
                }
            },
            "required": ["text"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let text = extract_string_field(
            arguments,
            &["text", "content", "memory", "fact", "data", "body", "value"],
        )
        .ok_or_else(|| anyhow!("Missing 'text' parameter"))?;

        let tags = extract_string_list_field(
            arguments,
            &["tags", "tag", "categories", "category"],
        );

        let importance = extract_f32_arg(arguments, &["importance", "priority", "weight", "score"])
            .unwrap_or(0.8)
            .clamp(0.0, 1.0);
        let decay_rate = extract_f32_arg(arguments, &["decay_rate", "decayRate", "decay"])
            .unwrap_or(0.05)
            .clamp(0.0, 1.0);

        let embedding = get_embedding(&text, false).await?;
        let workspace = get_current_workspace();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        let embedding_json = serde_json::to_string(&embedding)?;
        let tags_json = serde_json::to_string(&tags)?;

        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO cognitive_memory (id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9)",
                params![id, text, embedding_json, timestamp, workspace, tags_json, importance, timestamp, decay_rate],
            )?;

            let _ = prune_decayed_memories(&tx);
            tx.commit()?;
            Ok(())
        })?;

        Ok(json!({
            "status": "success",
            "message": "Fact stored successfully in the cognitive memory bus."
        }))
    }
}

// 2. RecallMemoryTool
pub struct RecallMemoryTool;

#[async_trait::async_trait]
impl Tool for RecallMemoryTool {
    fn name(&self) -> &str {
        "recall_memory"
    }

    fn description(&self) -> &str {
        "Recall relevant facts, guidelines, or code patterns from the cognitive memory bus using semantic similarity combined with temporal memory decay."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query to search for in memory (e.g. 'cargo check workflow issue', aliases: q, search, term)."
                },
                "top_k": {
                    "type": "integer",
                    "description": "Optional number of top matches to return (default 5, aliases: topK, limit, count)."
                },
                "scope": {
                    "type": "string",
                    "enum": ["workspace", "global"],
                    "description": "Optional search scope. 'workspace' limits results to the current active project/directory. 'global' searches all workspaces (default 'workspace')."
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional list of tags (or comma-separated string) to filter by. Matches entries containing at least one tag."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = extract_string_field(
            arguments,
            &["query", "q", "search", "text", "prompt", "term", "filter"],
        )
        .unwrap_or_else(|| "*".to_string());

        let top_k = extract_usize_arg(arguments, &["top_k", "topK", "limit", "count", "n"])
            .unwrap_or(5);
        let raw_scope = extract_string_field(arguments, &["scope", "domain", "workspace_scope"])
            .unwrap_or_else(|| "workspace".to_string());
        let scope = if raw_scope.eq_ignore_ascii_case("global") || raw_scope.eq_ignore_ascii_case("all") {
            "global"
        } else {
            "workspace"
        };

        let filter_tags: Vec<String> = extract_string_list_field(
            arguments,
            &["tags", "tag", "category", "categories"],
        );

        let is_wildcard = query == "*"
            || query.eq_ignore_ascii_case("all")
            || query.eq_ignore_ascii_case("recent")
            || query.trim().is_empty();
        let query_embed = if is_wildcard {
            Vec::new()
        } else {
            get_embedding(&query, true).await?
        };
        let current_ws = get_current_workspace();

        let _lock = get_db_mutex().lock().await;
        let entries = with_db(|conn| {
            let mut stmt = conn.prepare("SELECT id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate FROM cognitive_memory LIMIT 1000")?;
            let mapped = stmt.query_map([], |row| {
                let embedding_str: String = row.get(2)?;
                let tags_str: String = row.get(5)?;
                let embedding: Vec<f32> = serde_json::from_str(&embedding_str).unwrap_or_default();
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

                Ok(CognitiveMemoryEntry {
                    id: row.get(0)?,
                    text: row.get(1)?,
                    embedding,
                    timestamp: row.get(3)?,
                    workspace: row.get(4)?,
                    tags,
                    importance: row.get(6)?,
                    last_accessed: row.get(7)?,
                    access_count: row.get(8)?,
                    decay_rate: row.get(9)?,
                })
            })?;

            let mut collected = Vec::new();
            for item in mapped {
                collected.push(item?);
            }
            Ok(collected)
        })?;

        let now = chrono::Utc::now();
        let mut scored_results = Vec::new();

        for entry in entries {
            // Scope filter
            if scope == "workspace" && entry.workspace != current_ws {
                continue;
            }

            // Tag filter
            if !filter_tags.is_empty() {
                let matches_tag = entry.tags.iter().any(|t| filter_tags.contains(t));
                if !matches_tag {
                    continue;
                }
            }

            // Calculate temporal decay factor
            let last_acc_date = chrono::DateTime::parse_from_rfc3339(&entry.last_accessed)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or(now);
            let duration = now.signed_duration_since(last_acc_date);
            let days_elapsed = duration.num_seconds() as f32 / 86400.0;

            let decay_factor = (-entry.decay_rate * days_elapsed).exp();
            let decayed_importance = entry.importance * decay_factor;

            let cognitive_score = if is_wildcard {
                decayed_importance
            } else {
                let sim = cosine_similarity(&query_embed, &entry.embedding);
                let query_lower = query.to_lowercase();
                let keyword_match = query_lower.split_whitespace().any(|w| {
                    w.len() >= 3
                        && (entry.text.to_lowercase().contains(w)
                            || entry.tags.iter().any(|t| t.to_lowercase().contains(w)))
                });
                let keyword_boost = if keyword_match { 0.2 } else { 0.0 };
                (sim * 0.7 + decayed_importance * 0.3 + keyword_boost).min(1.0)
            };

            scored_results.push((cognitive_score, entry));
        }

        // Sort descending by cognitive score
        scored_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let selected_matches: Vec<(f32, CognitiveMemoryEntry)> =
            scored_results.into_iter().take(top_k).collect();

        // Update last_accessed and access_count for returned matches
        let now_str = now.to_rfc3339();
        let _ = with_db(|conn| {
            for (_, entry) in &selected_matches {
                let _ = conn.execute(
                    "UPDATE cognitive_memory SET last_accessed = ?1, access_count = access_count + 1 WHERE id = ?2",
                    params![now_str, entry.id],
                );
            }
            Ok(())
        });

        let matches_val: Vec<Value> = selected_matches
            .into_iter()
            .map(|(score, entry)| {
                json!({
                    "id": entry.id,
                    "text": entry.text,
                    "score": score,
                    "timestamp": entry.timestamp,
                    "workspace": entry.workspace,
                    "tags": entry.tags,
                })
            })
            .collect();

        Ok(json!({
            "status": "success",
            "matches": matches_val
        }))
    }
}

// 3. ClearMemoryTool
pub struct ClearMemoryTool;

#[async_trait::async_trait]
impl Tool for ClearMemoryTool {
    fn name(&self) -> &str {
        "clear_memory"
    }

    fn description(&self) -> &str {
        "Clear stored memories or force-compact the cognitive memory database."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["workspace", "all", "prune"],
                    "description": "The scope of memory deletion. 'workspace' deletes memories for the current project. 'all' deletes all stored memories. 'prune' deletes deeply decayed memories (default 'workspace')."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let raw_scope = extract_string_field(arguments, &["scope", "target", "mode", "domain"])
            .unwrap_or_else(|| "workspace".to_string());
        let scope = if raw_scope.eq_ignore_ascii_case("all") || raw_scope.eq_ignore_ascii_case("global") {
            "all"
        } else if raw_scope.eq_ignore_ascii_case("prune") || raw_scope.eq_ignore_ascii_case("decayed") {
            "prune"
        } else {
            "workspace"
        };

        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            if scope == "all" {
                conn.execute("DELETE FROM cognitive_memory", [])?;
                Ok(json!({
                    "status": "success",
                    "message": "All memories cleared successfully."
                }))
            } else if scope == "prune" {
                let count = prune_decayed_memories(conn)?;
                Ok(json!({
                    "status": "success",
                    "message": format!("Pruned {} deeply decayed memories from the cognitive database.", count)
                }))
            } else {
                let current_ws = get_current_workspace();
                let count = conn.execute(
                    "DELETE FROM cognitive_memory WHERE workspace = ?",
                    params![current_ws],
                )?;
                Ok(json!({
                    "status": "success",
                    "message": format!("Cleared {} memories associated with the current workspace.", count)
                }))
            }
        })
    }
}

// 4. DeleteMemoryTool
pub struct DeleteMemoryTool;

#[async_trait::async_trait]
impl Tool for DeleteMemoryTool {
    fn name(&self) -> &str {
        "delete_memory"
    }

    fn description(&self) -> &str {
        "Delete a specific stored memory by its ID. Use recall_memory first to get the ID."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The ID of the memory to delete (aliases: memory_id, memoryId, key)."
                }
            },
            "required": ["id"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = extract_string_field(arguments, &["id", "memory_id", "memoryId", "key"])
            .ok_or_else(|| anyhow!("Missing 'id' parameter"))?;

        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            let deleted =
                conn.execute("DELETE FROM cognitive_memory WHERE id = ?1", params![id])?;

            if deleted == 0 {
                Ok(json!({
                    "status": "not_found",
                    "message": format!("No memory found with ID '{}'", id)
                }))
            } else {
                Ok(json!({
                    "status": "success",
                    "message": "Memory deleted successfully."
                }))
            }
        })
    }
}

// 5. UpdateMemoryTool
pub struct UpdateMemoryTool;

#[async_trait::async_trait]
impl Tool for UpdateMemoryTool {
    fn name(&self) -> &str {
        "update_memory"
    }

    fn description(&self) -> &str {
        "Update the text and/or importance of a specific stored memory by its ID. The embedding is automatically recomputed."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The ID of the memory to update (aliases: memory_id, memoryId, key)."
                },
                "text": {
                    "type": "string",
                    "description": "The new text content for the memory (aliases: content, memory, fact)."
                },
                "importance": {
                    "type": "number",
                    "description": "Optional new importance score (0.0 to 1.0, aliases: priority, weight, score)."
                }
            },
            "required": ["id", "text"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let id = extract_string_field(arguments, &["id", "memory_id", "memoryId", "key"])
            .ok_or_else(|| anyhow!("Missing 'id' parameter"))?;
        let text = extract_string_field(
            arguments,
            &["text", "content", "memory", "fact", "body", "value"],
        )
        .ok_or_else(|| anyhow!("Missing 'text' parameter"))?;
        let importance = extract_f32_arg(arguments, &["importance", "priority", "weight", "score"])
            .map(|v| v.clamp(0.0, 1.0));

        let new_embedding = get_embedding(&text, false).await?;
        let embedding_json = serde_json::to_string(&new_embedding)?;

        let _lock = get_db_mutex().lock().await;
        with_db(|conn| {
            if let Some(imp) = importance {
                let updated = conn.execute(
                    "UPDATE cognitive_memory SET text = ?1, embedding = ?2, importance = ?3 WHERE id = ?4",
                    params![text, embedding_json, imp, id],
                )?;
                if updated == 0 {
                    Ok(
                        json!({"status": "not_found", "message": format!("No memory found with ID '{}'", id)}),
                    )
                } else {
                    Ok(json!({"status": "success", "message": "Memory updated successfully."}))
                }
            } else {
                let updated = conn.execute(
                    "UPDATE cognitive_memory SET text = ?1, embedding = ?2 WHERE id = ?3",
                    params![text, embedding_json, id],
                )?;
                if updated == 0 {
                    Ok(
                        json!({"status": "not_found", "message": format!("No memory found with ID '{}'", id)}),
                    )
                } else {
                    Ok(json!({"status": "success", "message": "Memory updated successfully."}))
                }
            }
        })
    }
}
