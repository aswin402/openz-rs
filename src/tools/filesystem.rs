use crate::config::resolve_path;
use crate::tools::Tool;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::fs;

fn deserialize_flexible_opt_usize<'de, D>(deserializer: D) -> std::result::Result<Option<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    match opt {
        Some(serde_json::Value::Number(n)) => {
            if let Some(u) = n.as_u64() {
                Ok(Some(u as usize))
            } else if let Some(i) = n.as_i64() {
                if i >= 0 {
                    Ok(Some(i as usize))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        Some(serde_json::Value::String(s)) => {
            if let Ok(u) = s.trim().parse::<usize>() {
                Ok(Some(u))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

fn deserialize_flexible_usize<'de, D>(deserializer: D) -> std::result::Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Ok(u as usize)
            } else if let Some(i) = n.as_i64() {
                if i >= 0 {
                    Ok(i as usize)
                } else {
                    Err(serde::de::Error::custom("expected positive number"))
                }
            } else {
                Err(serde::de::Error::custom("expected integer"))
            }
        }
        serde_json::Value::String(s) => s
            .trim()
            .parse::<usize>()
            .map_err(|_| serde::de::Error::custom("expected integer string")),
        _ => Err(serde::de::Error::custom("expected number or string integer")),
    }
}

#[derive(Deserialize)]
struct PathArg {
    #[serde(
        default = "default_find_dir",
        alias = "file_path",
        alias = "filePath",
        alias = "dir",
        alias = "directory",
        alias = "target_dir",
        alias = "folder"
    )]
    path: String,
}

#[derive(Deserialize)]
struct ReadFileArgs {
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file",
        alias = "filename",
        alias = "uri"
    )]
    path: String,
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_opt_usize",
        alias = "startLine",
        alias = "start",
        alias = "from_line"
    )]
    start_line: Option<usize>,
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_opt_usize",
        alias = "endLine",
        alias = "end",
        alias = "to_line"
    )]
    end_line: Option<usize>,
}

#[derive(Deserialize)]
struct WriteFileArgs {
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file",
        alias = "filename",
        alias = "output_path",
        alias = "outputPath"
    )]
    path: String,
    #[serde(
        alias = "text",
        alias = "code",
        alias = "data",
        alias = "body"
    )]
    content: String,
}

#[derive(Deserialize)]
struct PatchFileArgs {
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file"
    )]
    path: String,
    #[serde(
        alias = "diff",
        alias = "unified_diff",
        alias = "unifiedDiff",
        alias = "content"
    )]
    patch: String,
}

#[derive(Deserialize)]
struct ReplaceLinesArgs {
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file"
    )]
    path: String,
    #[serde(
        deserialize_with = "deserialize_flexible_usize",
        alias = "startLine",
        alias = "start",
        alias = "from_line"
    )]
    start_line: usize,
    #[serde(
        deserialize_with = "deserialize_flexible_usize",
        alias = "endLine",
        alias = "end",
        alias = "to_line"
    )]
    end_line: usize,
    #[serde(
        alias = "content",
        alias = "new_content",
        alias = "newContent",
        alias = "text",
        alias = "code"
    )]
    replacement: String,
}

#[derive(Deserialize)]
struct FindFilesArgs {
    #[serde(
        default = "default_find_pattern",
        alias = "glob",
        alias = "query",
        alias = "search",
        alias = "name",
        alias = "filename_pattern"
    )]
    pattern: String,
    #[serde(
        default = "default_find_dir",
        alias = "directory",
        alias = "root",
        alias = "path",
        alias = "folder"
    )]
    dir: String,
}

fn default_find_pattern() -> String {
    "*".to_string()
}

fn default_find_dir() -> String {
    ".".to_string()
}

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
                "path": { "type": "string", "description": "Absolute or relative path to the file. Aliases accepted: file_path, filePath." },
                "start_line": { "type": "integer", "description": "Start line (1-indexed, inclusive). Alias accepted: startLine." },
                "end_line": { "type": "integer", "description": "End line (1-indexed, inclusive). Alias accepted: endLine." }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let args: ReadFileArgs = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid read_file arguments: {}", e))?;
        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;

        // Guard against reading excessively large files (>50MB) to prevent OOM
        let metadata = fs::metadata(&path)
            .with_context(|| format!("Failed to read file metadata at {:?}", path))?;
        const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB
        if metadata.len() > MAX_FILE_SIZE {
            return Err(anyhow!(
                "File too large to read ({} bytes, max {} bytes). Use start_line/end_line to read specific ranges.",
                metadata.len(),
                MAX_FILE_SIZE
            ));
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file at {:?}", path))?;

        let start_line = args.start_line;
        let end_line = args.end_line;

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
        "Write content to a file, overwriting it if it exists. Keep content payloads small. For large generated files (roughly over 8KB), write in smaller chunks with shell redirection/heredoc or patch_file to avoid malformed/truncated JSON tool calls."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative path to the file. Aliases accepted: file_path, filePath." },
                "content": { "type": "string", "description": "File content to write. Keep under roughly 8KB per tool call; for larger files, write in chunks or use a command-line heredoc approach." }
            },
            "required": ["path", "content"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let args: WriteFileArgs = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid write_file arguments: {}", e))?;

        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, &args.content)
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
                "path": { "type": "string", "description": "Absolute or relative path to the directory. Aliases accepted: file_path, filePath." }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let args: PathArg = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid list_dir arguments: {}", e))?;
        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;

        let mut entries = Vec::new();
        for entry in fs::read_dir(&path)
            .with_context(|| format!("Failed to read directory at {:?}", path))?
        {
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

        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        Ok(serde_json::json!({
            "path": path.to_string_lossy(),
            "canonical_path": canonical_path.to_string_lossy(),
            "entries": entries
        }))
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
                "path": { "type": "string", "description": "Absolute or relative path to the file to modify. Aliases accepted: file_path, filePath." },
                "patch": { "type": "string", "description": "Unified diff patch content to apply (standard diff format)" }
            },
            "required": ["path", "patch"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let args: PatchFileArgs = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid patch_file arguments: {}", e))?;

        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file at {:?}", path))?;

        let parsed_patch = diffy::Patch::from_str(&args.patch)
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
                "path": { "type": "string", "description": "Absolute or relative path to the file to edit. Aliases accepted: file_path, filePath." },
                "start_line": { "type": "integer", "description": "Start line number (1-indexed, inclusive). Alias accepted: startLine." },
                "end_line": { "type": "integer", "description": "End line number (1-indexed, inclusive). Alias accepted: endLine." },
                "replacement": { "type": "string", "description": "The new replacement text content. Alias accepted: content." }
            },
            "required": ["path", "start_line", "end_line", "replacement"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let args: ReplaceLinesArgs = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid replace_lines arguments: {}", e))?;
        let start_line = if args.start_line == 0 { 1 } else { args.start_line };
        let end_line = if args.end_line == 0 { 1 } else { args.end_line };
        let replacement = args.replacement;

        if start_line > end_line {
            return Err(anyhow!(
                "Invalid line range: {} to {} (start_line cannot be greater than end_line)",
                start_line,
                end_line
            ));
        }

        let path = resolve_path(&args.path);
        crate::config::loader::verify_safe_path(&path)?;

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file at {:?}", path))?;

        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        if start_line > lines.len() + 1 {
            return Err(anyhow!(
                "start_line {} is beyond file line count {}",
                start_line,
                lines.len()
            ));
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

        // Preserve trailing newline of the original file
        let mut new_content = new_lines.join("\n");
        if content.ends_with('\n') && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
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

    fn walk_and_find(
        dir: &std::path::Path,
        re: &regex::Regex,
        results: &mut Vec<String>,
    ) -> Result<()> {
        if let Ok(metadata) = dir.symlink_metadata() {
            if metadata.file_type().is_symlink() {
                return Ok(());
            }
        }
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
                '?' => regex_str.push('.'),
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
                "pattern": { "type": "string", "description": "The search pattern (e.g. '*.rs', 'Cargo.toml', 'index.*'). Alias accepted: glob." },
                "dir": { "type": "string", "description": "The root directory to search in (defaults to '.'). Aliases accepted: directory, root." }
            },
            "required": ["pattern"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let args: FindFilesArgs = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid find_files arguments: {}", e))?;

        let search_dir = resolve_path(&args.dir);
        crate::config::loader::verify_safe_path(&search_dir)?;

        if !search_dir.exists() {
            return Err(anyhow!("Directory '{}' does not exist", args.dir));
        }

        // Try using fd first since it is installed and extremely fast
        if let Ok(results) = self.run_fd(&search_dir, &args.pattern).await {
            return Ok(serde_json::json!({ "status": "success", "results": results }));
        }

        // Fallback to manual recursive search if fd fails or is not found
        let re = self.glob_to_regex(&args.pattern)?;
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
        let path_str = arguments
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'content' parameter"))?;
        let compile_cmd = arguments
            .get("compile_command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'compile_command' parameter"))?;

        let path = resolve_path(path_str);
        crate::config::loader::verify_safe_path(&path)?;

        crate::core::heal::run_transactional_heal_edit(
            self.provider.as_ref(),
            &path,
            content,
            compile_cmd,
        )
        .await
    }
}

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
