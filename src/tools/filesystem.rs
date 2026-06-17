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

pub struct PatchFileTool;

#[async_trait::async_trait]
impl Tool for PatchFileTool {
    fn name(&self) -> &str {
        "patch_file"
    }

    fn description(&self) -> &str {
        "Apply a unified diff patch to a file. This is highly efficient for applying specific modifications to a file without rewriting it entirely."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file to modify" },
                "patch": { "type": "string", "description": "Unified diff patch content to apply (standard diff format)" }
            },
            "required": ["path", "patch"]
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
        
        let patch_str = arguments.get("patch")
            .or(arguments.get("content"))
            .or(arguments.get("diff"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'patch' argument"))?;

        let path = resolve_path(path_str);
        
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file at {:?}", path))?;

        let parsed_patch = diffy::Patch::from_str(patch_str)
            .map_err(|e| anyhow!("Failed to parse patch: {}", e))?;

        let patched_content = diffy::apply(&content, &parsed_patch)
            .map_err(|e| anyhow!("Failed to apply patch: {}", e))?;

        fs::write(&path, &patched_content)
            .with_context(|| format!("Failed to write patched content to file at {:?}", path))?;

        Ok(serde_json::json!({ "status": "success", "path": path.to_string_lossy() }))
    }
}

pub struct ReplaceLinesTool;

#[async_trait::async_trait]
impl Tool for ReplaceLinesTool {
    fn name(&self) -> &str {
        "replace_lines"
    }

    fn description(&self) -> &str {
        "Replace a specific range of lines (1-indexed, inclusive) in a file with new content."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file to edit" },
                "start_line": { "type": "integer", "description": "Start line number (1-indexed, inclusive)" },
                "end_line": { "type": "integer", "description": "End line number (1-indexed, inclusive)" },
                "replacement": { "type": "string", "description": "The new replacement text content" }
            },
            "required": ["path", "start_line", "end_line", "replacement"]
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
        
        let start_line = arguments.get("start_line").and_then(|v| v.as_u64()).map(|v| v as usize)
            .ok_or_else(|| anyhow!("Missing 'start_line' argument"))?;
        let end_line = arguments.get("end_line").and_then(|v| v.as_u64()).map(|v| v as usize)
            .ok_or_else(|| anyhow!("Missing 'end_line' argument"))?;
        let replacement = arguments.get("replacement").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'replacement' argument"))?;

        if start_line == 0 || end_line == 0 || start_line > end_line {
            return Err(anyhow!("Invalid line range: {} to {}", start_line, end_line));
        }

        let path = resolve_path(path_str);
        
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file at {:?}", path))?;
        
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        
        if start_line > lines.len() + 1 {
            return Err(anyhow!("start_line {} is beyond file line count {}", start_line, lines.len()));
        }

        let start_idx = start_line - 1;
        let end_idx = (end_line).min(lines.len());

        let mut new_lines = Vec::new();
        new_lines.extend(lines[..start_idx].iter().cloned());
        for repl_line in replacement.lines() {
            new_lines.push(repl_line.to_string());
        }
        if end_idx < lines.len() {
            new_lines.extend(lines[end_idx..].iter().cloned());
        }

        let new_content = new_lines.join("\n");
        fs::write(&path, &new_content)
            .with_context(|| format!("Failed to write to file at {:?}", path))?;

        Ok(serde_json::json!({
            "status": "success",
            "path": path.to_string_lossy(),
            "lines_modified": end_idx - start_idx,
            "new_line_count": new_lines.len()
        }))
    }
}

pub struct FindFilesTool;

impl FindFilesTool {
    async fn run_fd(&self, dir: &std::path::Path, pattern: &str) -> Result<Vec<String>> {
        let mut cmd = tokio::process::Command::new("fd");
        cmd.arg("-g"); // Treat pattern as a glob
        cmd.arg("-L"); // Follow symlinks
        cmd.arg("--hidden"); // Include hidden files
        cmd.arg("--exclude").arg("target");
        cmd.arg("--exclude").arg("node_modules");
        cmd.arg("--exclude").arg(".git");
        cmd.arg(pattern);
        cmd.arg(dir);

        let output = cmd.output().await?;
        if !output.status.success() && output.status.code() != Some(1) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("fd failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let results = stdout
            .lines()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        Ok(results)
    }

    fn walk_and_find(dir: &std::path::Path, re: &regex::Regex, results: &mut Vec<String>) -> Result<()> {
        if results.len() >= 1000 {
            return Ok(());
        }

        if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
            if name == "target" || name == "node_modules" || name == ".git" {
                return Ok(());
            }
        }

        if dir.is_file() {
            if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
                if re.is_match(name) {
                    results.push(dir.to_string_lossy().to_string());
                }
            }
            return Ok(());
        }

        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    Self::walk_and_find(&entry.path(), re, results)?;
                }
            }
        }

        Ok(())
    }

    fn glob_to_regex(&self, pattern: &str) -> Result<regex::Regex> {
        let mut regex_str = String::from("^");
        for c in pattern.chars() {
            match c {
                '*' => regex_str.push_str(".*"),
                '?' => regex_str.push_str("."),
                '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                    regex_str.push('\\');
                    regex_str.push(c);
                }
                _ => regex_str.push(c),
            }
        }
        regex_str.push('$');
        regex::Regex::new(&regex_str).map_err(|e| anyhow!("Invalid pattern: {}", e))
    }
}

#[async_trait::async_trait]
impl Tool for FindFilesTool {
    fn name(&self) -> &str {
        "find_files"
    }

    fn description(&self) -> &str {
        "Search for files inside a directory hierarchy matching a specific filename pattern/glob."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "The search pattern (e.g. '*.rs', 'Cargo.toml', 'index.*')" },
                "dir": { "type": "string", "description": "The root directory to search in (defaults to '.')" }
            },
            "required": ["pattern"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let pattern = arguments.get("pattern").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'pattern' argument"))?;
        
        let search_dir_str = arguments.get("dir").and_then(|v| v.as_str()).unwrap_or(".");
        let search_dir = resolve_path(search_dir_str);

        if !search_dir.exists() {
            return Err(anyhow!("Directory '{}' does not exist", search_dir_str));
        }

        // Try using fd first since it is installed and extremely fast
        if let Ok(results) = self.run_fd(&search_dir, pattern).await {
            return Ok(serde_json::json!({ "status": "success", "results": results }));
        }

        // Fallback to manual recursive search if fd fails or is not found
        let re = self.glob_to_regex(pattern)?;
        let mut results = Vec::new();
        Self::walk_and_find(&search_dir, &re, &mut results)?;

        Ok(serde_json::json!({ "status": "success", "results": results }))
    }
}

pub struct ZenflowEditTool {
    pub provider: std::sync::Arc<dyn crate::providers::LLMProvider>,
}

#[async_trait::async_trait]
impl Tool for ZenflowEditTool {
    fn name(&self) -> &str {
        "zenflow_edit"
    }

    fn description(&self) -> &str {
        "Edit a file transactionally. Takes a git snapshot before writing. If compilation/tests fail, it attempts to self-heal using the LLM. If healing fails, it automatically rolls back the changes."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to edit." },
                "content": { "type": "string", "description": "Complete new content to write to the file." },
                "compile_command": { "type": "string", "description": "Command to run to verify the build/test (e.g. 'cargo check', 'npm run build', 'pytest')." }
            },
            "required": ["path", "content", "compile_command"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let path_str = arguments.get("path")
            .or(arguments.get("TargetFile"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        let content = arguments.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'content' parameter"))?;
        let compile_cmd = arguments.get("compile_command").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'compile_command' parameter"))?;

        let path = resolve_path(path_str);
        
        let run_cmd = |cmd: &str| -> Result<(i32, String)> {
            let mut command = std::process::Command::new("sh");
            crate::config::loader::set_command_cwd(&mut command);
            command.arg("-c").arg(cmd);
            let output = command.output()?;
            let status = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Ok((status, format!("{}{}", stdout, stderr)))
        };

        let in_git = run_cmd("git rev-parse --is-inside-work-tree")
            .map(|(code, _)| code == 0)
            .unwrap_or(false);

        let mut committed = false;
        let mut original_content = None;

        if in_git {
            let _ = run_cmd("git add -A");
            if let Ok((code, _)) = run_cmd("git commit -m \"Zenflow pre-edit backup\" --no-verify") {
                if code == 0 {
                    committed = true;
                }
            }
        }

        if !committed {
            original_content = fs::read_to_string(&path).ok();
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;

        let (mut status, mut output_str) = run_cmd(compile_cmd)?;

        if status != 0 {
            let system_prompt = "You are a Self-Healing Code Assistant. Fix compile/test errors in the provided file.";
            let user_prompt = format!(
                "The following file edit was made at path '{}' but caused compile/test errors.\n\n\
                 Proposed Content:\n\
                 ```\n\
                 {}\n\
                 ```\n\n\
                 Compilation Error:\n\
                 ```\n\
                 {}\n\
                 ```\n\n\
                 Please analyze the compilation error and return the corrected, complete file content. Output ONLY the complete corrected content, no markdown wrappers like ```rust, no explanations.",
                 path.to_string_lossy(),
                 content,
                 output_str
            );

            let messages = vec![crate::session::Message {
                role: "user".to_string(),
                content: user_prompt,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            }];

            let settings = crate::providers::GenerationSettings {
                temperature: 0.1,
                max_tokens: 4096,
                reasoning_effort: None,
            };

            if let Ok(resp) = self.provider.chat(system_prompt, &messages, &[], &settings).await {
                if let Some(healed_content) = resp.content {
                    let cleaned = healed_content.trim().to_string();
                    if !cleaned.is_empty() {
                        fs::write(&path, &cleaned)?;
                        if let Ok((h_status, h_output)) = run_cmd(compile_cmd) {
                            status = h_status;
                            output_str = h_output;
                        }
                    }
                }
            }
        }

        if status == 0 {
            if committed {
                let _ = run_cmd("git reset --soft HEAD~1");
            }
            Ok(serde_json::json!({
                "status": "success",
                "message": "File written and verified successfully."
            }))
        } else {
            if committed {
                let _ = run_cmd("git reset --hard HEAD~1");
            } else if let Some(orig) = original_content {
                fs::write(&path, orig)?;
            } else {
                let _ = fs::remove_file(&path);
            }
            Ok(serde_json::json!({
                "status": "error",
                "error": format!("Compilation failed, self-healing failed. Rolled back changes. Error output:\n{}", output_str)
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_find_files() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("openz_find_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        
        let file_path = temp_dir.join("match_this.txt");
        std::fs::write(&file_path, "Hello world!")?;

        let tool = FindFilesTool;
        let args = serde_json::json!({
            "pattern": "*match*",
            "dir": temp_dir.to_str().unwrap()
        });

        let res = tool.call(&args).await?;
        assert_eq!(res["status"], "success");
        let results = res["results"].as_array().unwrap();
        assert!(results.len() >= 1);
        
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_replace_lines() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("openz_replace_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        
        let file_path = temp_dir.join("test.txt");
        std::fs::write(&file_path, "line 1\nline 2\nline 3")?;

        let tool = ReplaceLinesTool;
        let args = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "start_line": 2,
            "end_line": 2,
            "replacement": "replaced line 2"
        });

        let res = tool.call(&args).await?;
        assert_eq!(res["status"], "success");
        
        let updated = std::fs::read_to_string(&file_path)?;
        assert_eq!(updated, "line 1\nreplaced line 2\nline 3");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}

