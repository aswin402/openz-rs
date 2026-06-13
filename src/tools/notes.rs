use crate::tools::Tool;
use crate::tools::shared_memory::{get_db_mutex, get_db_path, get_current_workspace, get_embedding, MemoryEntry};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub struct IndexNotesTool;

fn walk_notes(dir: &Path, md_files: &mut Vec<PathBuf>) -> Result<()> {
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
        "Scan local notes (e.g. Obsidian or local markdown files) and index their sections into semantic memory for search."
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
                        
                        entries_to_add.push(MemoryEntry {
                            id,
                            text: formatted_text,
                            embedding,
                            timestamp,
                            workspace,
                            tags: vec!["notes".to_string(), "second-brain".to_string()],
                        });
                        count += 1;
                    }
                }
            }
        }

        if !entries_to_add.is_empty() {
            let db_path = get_db_path();
            let _lock = get_db_mutex().lock().await;

            let mut entries: Vec<MemoryEntry> = if db_path.exists() {
                let data = fs::read_to_string(&db_path)?;
                serde_json::from_str(&data).unwrap_or_default()
            } else {
                Vec::new()
            };

            entries.extend(entries_to_add);

            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let serialized = serde_json::to_string_pretty(&entries)?;
            fs::write(db_path, serialized)?;
        }

        Ok(json!({
            "status": "success",
            "message": format!("Successfully indexed {} notes sections into semantic memory.", count)
        }))
    }
}
