use crate::tools::Tool;
use crate::config::resolve_path;
use anyhow::{Result, anyhow, Context};
use std::fs;

pub struct ReadFileTool;

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read contents of a file. Supports reading specific line ranges (1-indexed)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file" },
                "start_line": { "type": "integer", "description": "Start line (1-indexed, inclusive)" },
                "end_line": { "type": "integer", "description": "End line (1-indexed, inclusive)" }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let path_str = arguments.get("path")
            .or(arguments.get("TargetFile"))
            .or(arguments.get("filepath"))
            .or(arguments.get("file"))
            .or(arguments.get("Path"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'path' argument"))?;
        let path = resolve_path(path_str);
        
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file at {:?}", path))?;
        
        let start_line = arguments.get("start_line").and_then(|v| v.as_u64()).map(|v| v as usize);
        let end_line = arguments.get("end_line").and_then(|v| v.as_u64()).map(|v| v as usize);
        
        if start_line.is_some() || end_line.is_some() {
            let lines: Vec<&str> = content.lines().collect();
            let start = start_line.unwrap_or(1).saturating_sub(1);
            let end = end_line.unwrap_or(lines.len()).min(lines.len());
            
            if start > lines.len() || start >= end {
                return Ok(serde_json::Value::String(String::new()));
            }
            
            let sliced = lines[start..end].join("\n");
            Ok(serde_json::Value::String(sliced))
        } else {
            Ok(serde_json::Value::String(content))
        }
    }
}

pub struct WriteFileTool;

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file, overwriting it if it exists."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file" },
                "content": { "type": "string", "description": "File content to write" }
            },
            "required": ["path", "content"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let path_str = arguments.get("path")
            .or(arguments.get("TargetFile"))
            .or(arguments.get("filepath"))
            .or(arguments.get("file"))
            .or(arguments.get("Path"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'path' argument"))?;
        let content = arguments.get("content")
            .or(arguments.get("code"))
            .or(arguments.get("text"))
            .or(arguments.get("content_str"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'content' argument"))?;
        
        let path = resolve_path(path_str);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        fs::write(&path, content)
            .with_context(|| format!("Failed to write to file at {:?}", path))?;
        
        Ok(serde_json::json!({ "status": "success", "path": path.to_string_lossy() }))
    }
}

pub struct ListDirTool;

#[async_trait::async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List the contents of a directory."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the directory" }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let path_str = arguments.get("path")
            .or(arguments.get("TargetFile"))
            .or(arguments.get("filepath"))
            .or(arguments.get("file"))
            .or(arguments.get("Path"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'path' argument"))?;
        let path = resolve_path(path_str);
        
        let mut entries = Vec::new();
        for entry in fs::read_dir(&path).with_context(|| format!("Failed to read directory at {:?}", path))? {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let metadata = entry.metadata()?;
            let is_dir = metadata.is_dir();
            let size = metadata.len();
            
            entries.push(serde_json::json!({
                "name": file_name,
                "is_dir": is_dir,
                "size_bytes": size
            }));
        }
        
        Ok(serde_json::Value::Array(entries))
    }
}
