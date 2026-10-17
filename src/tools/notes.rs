use crate::memory::MemoryService;
use crate::tools::shared_memory::{get_db_mutex, CognitiveMemoryEntry};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::params;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub struct IndexNotesTool;

fn walk_notes(dir: &Path, md_files: &mut Vec<PathBuf>) -> Result<()> {
    let is_symlink = dir
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);
    match (is_symlink, dir.is_dir(), dir.is_file()) {
        (true, _, _) => Ok(()),
        (false, true, _) => {
            let skip_dir = dir
                .file_name()
                .and_then(|s| s.to_str())
                .map(|name| matches!(name, "target" | "node_modules" | ".git"))
                .unwrap_or(false);
            match (skip_dir, fs::read_dir(dir)) {
                (false, Ok(entries)) => {
                    for entry in entries.flatten() {
                        walk_notes(&entry.path(), md_files)?;
                    }
                    Ok(())
                }
                _ => Ok(()),
            }
        }
        (false, false, true) => {
            match dir.extension().and_then(|s| s.to_str()) {
                Some("md") => md_files.push(dir.to_path_buf()),
                _ => {}
            }
            Ok(())
        }
        (false, false, false) => Ok(()),
    }
}

fn push_non_empty_block(
    blocks: &mut Vec<(String, String)>,
    file_name: &str,
    header: &str,
    content: &str,
) {
    let clean_content = content.trim();
    match clean_content.is_empty() {
        true => {}
        false => blocks.push((
            format!("{} > {}", file_name, header),
            clean_content.to_string(),
        )),
    }
}

fn parse_markdown_blocks(content: &str, file_name: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut current_header = "General".to_string();
    let mut current_content = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        match trimmed.starts_with('#') {
            true => {
                push_non_empty_block(&mut blocks, file_name, &current_header, &current_content);
                current_header = trimmed.trim_start_matches('#').trim().to_string();
                current_content.clear();
            }
            false => {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }
    }

    push_non_empty_block(&mut blocks, file_name, &current_header, &current_content);
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
        let memory = MemoryService::current();
        let path_str = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let resolved_path = crate::config::resolve_path(path_str);

        match resolved_path.exists() {
            true => {}
            false => return Err(anyhow!("Directory '{}' does not exist", path_str)),
        }

        let mut md_files = Vec::new();
        walk_notes(&resolved_path, &mut md_files)?;

        let mut count = 0;
        let mut entries_to_add = Vec::new();

        for file in md_files {
            let content = match fs::read_to_string(&file) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let file_name = file.file_name().and_then(|s| s.to_str()).unwrap_or("Note");
            let blocks = parse_markdown_blocks(&content, file_name);

            for (header, text) in blocks {
                let formatted_text = format!("[Note Segment] {}: {}", header, text);
                match memory.embed(&formatted_text, false).await {
                    Ok(embedding) => {
                        let id = uuid::Uuid::new_v4().to_string();
                        let workspace = memory.scope().workspace.clone();
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
                    Err(_) => {}
                }
            }
        }

        match entries_to_add.is_empty() {
            true => {}
            false => {
                let _lock = get_db_mutex().lock().await;
                memory.with_shared_db(|conn| {
                    // Use a transaction for batch insert performance
                    conn.execute_batch("BEGIN TRANSACTION")?;
                    for entry in entries_to_add {
                        let embedding_json = serde_json::to_string(&entry.embedding)?;
                        let tags_json = serde_json::to_string(&entry.tags)?;
                        conn.execute(
                            "INSERT OR REPLACE INTO cognitive_memory (id, text, embedding, timestamp, workspace, tags, importance, last_accessed, access_count, decay_rate)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                            params![entry.id, entry.text, embedding_json, entry.timestamp, entry.workspace, tags_json, entry.importance, entry.last_accessed, entry.access_count, entry.decay_rate],
                        )?;
                    }
                    conn.execute_batch("COMMIT")?;
                    Ok(())
                })?;
            }
        }

        Ok(json!({
            "status": "success",
            "message": format!("Successfully indexed {} notes sections into cognitive semantic memory.", count)
        }))
    }
}
