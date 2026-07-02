use anyhow::Result;
use rusqlite::params;

use super::db::{get_db_mutex, get_sqlite_connection};
use super::embeddings::{get_embedding, cosine_similarity};
use super::cognitive::CognitiveMemoryEntry;

pub async fn consolidate_shared_memory(provider: &std::sync::Arc<dyn crate::providers::LLMProvider>) -> Result<()> {
    let _lock = get_db_mutex().lock().await;
    
    let mut entries: Vec<CognitiveMemoryEntry> = {
        let conn = get_sqlite_connection()?;
        let mut stmt = conn.prepare("SELECT id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate FROM cognitive_memory")?;
        let rows = stmt.query_map([], |row| {
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
        rows.flatten().collect()
    };

    if entries.len() < 5 {
        return Ok(());
    }

    let mut consolidated_count = 0;
    
    for _ in 0..3 {
        let n = entries.len();
        if n < 2 {
            break;
        }

        let mut max_sim = 0.0;
        let mut best_pair = None;

        for i in 0..n {
            for j in (i + 1)..n {
                let sim = cosine_similarity(&entries[i].embedding, &entries[j].embedding);
                if sim > max_sim {
                    max_sim = sim;
                    best_pair = Some((i, j));
                }
            }
        }

        if let Some((i, j)) = best_pair {
            if max_sim >= 0.82 {
                let entry_a = &entries[i];
                let entry_b = &entries[j];
                
                let merge_prompt = format!(
                    "Fact A: {}\nFact B: {}\n\nPlease consolidate these two facts into a single, concise, and complete statement. Do not lose any technical guidelines, details, or specific values. Return ONLY the consolidated statement, with no conversational filler.",
                    entry_a.text, entry_b.text
                );

                let system_prompt = "You are a Shared Memory Curator. Consolidate similar facts and remove redundancy, preserving all technical details.";
                let messages = vec![crate::session::Message {
                    role: "user".to_string(),
                    content: merge_prompt,
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    extra: serde_json::Map::new(),
                }];

                let settings = crate::providers::GenerationSettings {
                    temperature: 0.1,
                    max_tokens: 512,
                    reasoning_effort: None,
                };

                let resp = provider.chat(system_prompt, &messages, &[], &settings).await?;
                if let Some(merged_text) = resp.content {
                    let clean_text = merged_text.trim().to_string();
                    if !clean_text.is_empty() {
                        let new_embed = get_embedding(&clean_text, false).await?;
                        
                        let mut merged_tags = entry_a.tags.clone();
                        for t in &entry_b.tags {
                            if !merged_tags.contains(t) {
                                merged_tags.push(t.clone());
                            }
                        }

                        let workspace = entry_a.workspace.clone();
                        let new_id = uuid::Uuid::new_v4().to_string();
                        let now_str = chrono::Utc::now().to_rfc3339();
                        
                        // Calculate merged importance (average of two, scaled up slightly for consolidation reinforcement)
                        let new_importance = ((entry_a.importance + entry_b.importance) / 2.0 + 0.1).min(1.0);

                        let new_entry = CognitiveMemoryEntry {
                            id: new_id.clone(),
                            text: clean_text.clone(),
                            embedding: new_embed,
                            timestamp: now_str.clone(),
                            workspace,
                            tags: merged_tags.clone(),
                            importance: new_importance,
                            last_accessed: now_str.clone(),
                            access_count: entry_a.access_count + entry_b.access_count,
                            decay_rate: (entry_a.decay_rate + entry_b.decay_rate) / 2.0,
                        };

                        // Remove from database and insert new merged entry inside a short-lived block
                        {
                            let conn = get_sqlite_connection()?;
                            let _ = conn.execute("DELETE FROM cognitive_memory WHERE id IN (?1, ?2)", params![entry_a.id, entry_b.id]);
                            
                            let embedding_json = serde_json::to_string(&new_entry.embedding)?;
                            let tags_json = serde_json::to_string(&new_entry.tags)?;
                            let _ = conn.execute(
                                "INSERT INTO cognitive_memory (id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                                params![new_id, clean_text, embedding_json, now_str, new_entry.workspace, tags_json, new_importance, now_str, new_entry.access_count, new_entry.decay_rate],
                            );
                        }

                        entries.remove(j);
                        entries.remove(i);
                        entries.push(new_entry);
                        consolidated_count += 1;
                        continue;
                    }
                }
            }
        }
        break;
    }

    if consolidated_count > 0 {
        let aura_blue = "\x1b[38;2;96;165;250m";
        let color_reset = "\x1b[0m";
        crate::channels::cli::send_notification(&format!(
            "{}◇ [Memory-Curator] Consolidated {} duplicate/redundant shared memories.{}",
            aura_blue, consolidated_count, color_reset
        ));
    }

    Ok(())
}
