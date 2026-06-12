use crate::tools::Tool;
use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub struct GrepSearchTool;

impl GrepSearchTool {
    fn should_skip(path: &Path) -> bool {
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name.starts_with('.') && name != "." && name != ".." {
                return true;
            }
            let skipped_dirs = ["target", "node_modules", "build", "dist", "vendor", "bin", "obj"];
            if skipped_dirs.contains(&name) {
                return true;
            }
        }
        false
    }

    fn is_binary_file(path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let binary_extensions = [
                "png", "jpg", "jpeg", "gif", "ico", "pdf", "zip", "tar", "gz", "7z", "rar",
                "exe", "dll", "so", "dylib", "db", "sqlite", "wasm", "woff", "woff2", "ttf",
                "eot", "mp4", "mp3", "wav", "avi", "mov", "bin", "out", "o", "a",
            ];
            if binary_extensions.contains(&ext.to_lowercase().as_str()) {
                return true;
            }
        }
        false
    }

    fn walk_and_search(
        dir: &Path,
        query: &str,
        is_regex: bool,
        results: &mut Vec<Value>,
        limit: usize,
    ) -> Result<()> {
        if results.len() >= limit {
            return Ok(());
        }

        if Self::should_skip(dir) {
            return Ok(());
        }

        if dir.is_file() {
            if Self::is_binary_file(dir) {
                return Ok(());
            }

            if let Ok(content) = fs::read_to_string(dir) {
                let lines: Vec<&str> = content.lines().collect();
                let re = if is_regex {
                    match Regex::new(query) {
                        Ok(r) => Some(r),
                        Err(_) => None,
                    }
                } else {
                    None
                };

                for (idx, line) in lines.iter().enumerate() {
                    let is_match = if let Some(ref r) = re {
                        r.is_match(line)
                    } else {
                        line.contains(query)
                    };

                    if is_match {
                        results.push(json!({
                            "file": dir.to_string_lossy(),
                            "line": idx + 1,
                            "content": line.trim()
                        }));

                        if results.len() >= limit {
                            break;
                        }
                    }
                }
            }
            return Ok(());
        }

        if dir.is_dir() {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    Self::walk_and_search(&path, query, is_regex, results, limit)?;
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn run_ripgrep(
        &self,
        dir: &Path,
        query: &str,
        is_regex: bool,
        limit: usize,
    ) -> Result<Value> {
        let mut cmd = tokio::process::Command::new("rg");
        cmd.arg("--json");
        
        cmd.arg("--glob").arg("!target");
        cmd.arg("--glob").arg("!node_modules");
        cmd.arg("--glob").arg("!build");
        cmd.arg("--glob").arg("!dist");
        cmd.arg("--glob").arg("!.git");
        
        if !is_regex {
            cmd.arg("-F");
        }

        cmd.arg("--");
        cmd.arg(query);
        cmd.arg(dir);

        let output = cmd.output().await?;
        if !output.status.success() && output.status.code() != Some(1) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("ripgrep failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            if let Ok(val) = serde_json::from_str::<Value>(line) {
                if val.get("type").and_then(|t| t.as_str()) == Some("match") {
                    if let Some(data) = val.get("data") {
                        let file = data.get("path").and_then(|p| p.get("text")).and_then(|t| t.as_str()).unwrap_or_default().to_string();
                        let line_num = data.get("line_number").and_then(|l| l.as_u64()).unwrap_or(0);
                        let content = data.get("lines").and_then(|l| l.get("text")).and_then(|t| t.as_str()).unwrap_or_default().trim_end_matches('\n').trim().to_string();
                        
                        results.push(json!({
                            "file": file,
                            "line": line_num,
                            "content": content
                        }));

                        if results.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }

        Ok(json!({
            "status": "success",
            "results": results,
            "capped": results.len() >= limit
        }))
    }
}

#[async_trait::async_trait]
impl Tool for GrepSearchTool {
    fn name(&self) -> &str {
        "grep_search"
    }

    fn description(&self) -> &str {
        "Perform a high-speed text search across the codebase. Returns matching files, line numbers, and matching line contents."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search term or regex pattern to look for inside files."
                },
                "is_regex": {
                    "type": "boolean",
                    "description": "Whether to treat the query as a regular expression. Defaults to false."
                },
                "dir": {
                    "type": "string",
                    "description": "The directory path to search in (defaults to current directory '.')."
                }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments.get("query").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;
        
        let is_regex = arguments.get("is_regex").and_then(|v| v.as_bool()).unwrap_or(false);
        
        let search_dir_str = arguments.get("dir").and_then(|v| v.as_str()).unwrap_or(".");
        let search_dir = PathBuf::from(search_dir_str);

        if !search_dir.exists() {
            return Err(anyhow!("Directory '{}' does not exist", search_dir_str));
        }

        let limit = 50;

        // Try using Ripgrep first since it is installed and extremely fast
        if let Ok(output) = self.run_ripgrep(&search_dir, query, is_regex, limit).await {
            return Ok(output);
        }

        // Fallback to manual recursive search if Ripgrep fails
        let mut results = Vec::new();
        Self::walk_and_search(&search_dir, query, is_regex, &mut results, limit)?;

        Ok(json!({
            "status": "success",
            "results": results,
            "capped": results.len() >= limit
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_grep_search() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("openz_grep_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        
        let file_path = temp_dir.join("test.txt");
        std::fs::write(&file_path, "Hello world!\nThis is a grep test.\nHave a nice day!")?;

        let tool = GrepSearchTool;
        let args = json!({
            "query": "grep",
            "dir": temp_dir.to_str().unwrap()
        });

        let res = tool.call(&args).await?;
        assert_eq!(res["status"], "success");
        let results = res["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["line"], 2);
        assert_eq!(results[0]["content"], "This is a grep test.");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
