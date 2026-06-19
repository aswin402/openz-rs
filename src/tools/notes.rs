use crate::tools::Tool;
use crate::tools::shared_memory::{get_db_mutex, get_sqlite_connection, get_current_workspace, get_embedding, CognitiveMemoryEntry};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use rusqlite::params;

pub struct IndexNotesTool;

fn walk_notes(dir: &Path, md_files: &mut Vec<PathBuf>) -> Result<()> {
    if let Ok(metadata) = dir.symlink_metadata() {
        if metadata.file_type().is_symlink() {
            return Ok(());
        }
    }
    if dir.is_dir() {
        if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
            if name == "target" || name == "node_modules" || name == ".git" {
                return Ok(());
            }
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                walk_notes(&entry.path(), md_files)?;
            }
        }
    } else if dir.is_file() {
        if let Some(ext) = dir.extension().and_then(|s| s.to_str()) {
            if ext == "md" {
                md_files.push(dir.to_path_buf());
            }
        }
    }
    Ok(())
}

fn parse_markdown_blocks(content: &str, file_name: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut current_header = "General".to_string();
    let mut current_content = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let clean_content = current_content.trim().to_string();
            if !clean_content.is_empty() {
                blocks.push((format!("{} > {}", file_name, current_header), clean_content));
            }
            current_header = trimmed.trim_start_matches('#').trim().to_string();
            current_content = String::new();
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    let clean_content = current_content.trim().to_string();
    if !clean_content.is_empty() {
        blocks.push((format!("{} > {}", file_name, current_header), clean_content));
    }

    blocks
}

#[async_trait::async_trait]
impl Tool for IndexNotesTool {
    fn name(&self) -> &str {
        "index_notes"
    }

    fn description(&self) -> &str {
        "Scan local notes (e.g. Obsidian or local markdown files) and index their sections into cognitive semantic memory for search."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the notes directory to index (defaults to active project directory)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let path_str = arguments.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let resolved_path = crate::config::resolve_path(path_str);

        if !resolved_path.exists() {
            return Err(anyhow!("Directory '{}' does not exist", path_str));
        }

        let mut md_files = Vec::new();
        walk_notes(&resolved_path, &mut md_files)?;

        let mut count = 0;
        let mut entries_to_add = Vec::new();

        for file in md_files {
            if let Ok(content) = fs::read_to_string(&file) {
                let file_name = file.file_name().and_then(|s| s.to_str()).unwrap_or("Note");
                let blocks = parse_markdown_blocks(&content, file_name);

                for (header, text) in blocks {
                    let formatted_text = format!("[Note Segment] {}: {}", header, text);
                    if let Ok(embedding) = get_embedding(&formatted_text, false).await {
                        let id = uuid::Uuid::new_v4().to_string();
                        let workspace = get_current_workspace();
                        let timestamp = chrono::Utc::now().to_rfc3339();
                        
                        entries_to_add.push(CognitiveMemoryEntry {
                            id,
                            text: formatted_text,
                            embedding,
                            timestamp: timestamp.clone(),
                            workspace,
                            tags: vec!["notes".to_string(), "second-brain".to_string()],
                            importance: 0.5,
                            last_accessed: timestamp,
                            access_count: 1,
                            decay_rate: 0.05,
                        });
                        count += 1;
                    }
                }
            }
        }

        if !entries_to_add.is_empty() {
            let _lock = get_db_mutex().lock().await;
            let conn = get_sqlite_connection()?;

            for entry in entries_to_add {
                let embedding_json = serde_json::to_string(&entry.embedding)?;
                let tags_json = serde_json::to_string(&entry.tags)?;
                conn.execute(
                    "INSERT OR REPLACE INTO cognitive_memory (id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![entry.id, entry.text, embedding_json, entry.timestamp, entry.workspace, tags_json, entry.importance, entry.last_accessed, entry.access_count, entry.decay_rate],
                )?;
            }
        }

        Ok(json!({
            "status": "success",
            "message": format!("Successfully indexed {} notes sections into cognitive semantic memory.", count)
        }))
    }
}
