use crate::config::resolve_path;
use crate::tools::Tool;
use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
struct PathArg {
    #[serde(alias = "file_path", alias = "filePath")]
    path: String,
}

#[derive(Deserialize)]
struct ReadFileArgs {
    #[serde(alias = "file_path", alias = "filePath")]
    path: String,
    #[serde(default, alias = "startLine")]
    start_line: Option<usize>,
    #[serde(default, alias = "endLine")]
    end_line: Option<usize>,
}

#[derive(Deserialize)]
struct WriteFileArgs {
    #[serde(alias = "file_path", alias = "filePath")]
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct PatchFileArgs {
    #[serde(alias = "file_path", alias = "filePath")]
    path: String,
    patch: String,
}

#[derive(Deserialize)]
struct ReplaceLinesArgs {
    #[serde(alias = "file_path", alias = "filePath")]
    path: String,
    #[serde(alias = "startLine")]
    start_line: usize,
    #[serde(alias = "endLine")]
    end_line: usize,
    #[serde(alias = "content")]
    replacement: String,
}

#[derive(Deserialize)]
struct FindFilesArgs {
    #[serde(alias = "glob")]
    pattern: String,
    #[serde(default = "default_find_dir", alias = "directory", alias = "root")]
    dir: String,
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
                "path": { "type": "string", "description": "Absolute or relative path to the file" },
                "start_line": { "type": "integer", "description": "Start line (1-indexed, inclusive)" },
                "end_line": { "type": "integer", "description": "End line (1-indexed, inclusive)" }
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
                "path": { "type": "string", "description": "Absolute or relative path to the file" },
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
                "path": { "type": "string", "description": "Absolute or relative path to the directory" }
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
                "path": { "type": "string", "description": "Absolute or relative path to the file to edit" },
                "start_line": { "type": "integer", "description": "Start line number (1-indexed, inclusive)" },
                "end_line": { "type": "integer", "description": "End line number (1-indexed, inclusive)" },
                "replacement": { "type": "string", "description": "The new replacement text content" }
            },
            "required": ["path", "start_line", "end_line", "replacement"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let args: ReplaceLinesArgs = serde_json::from_value(arguments.clone())
            .map_err(|e| anyhow!("Invalid replace_lines arguments: {}", e))?;
        let start_line = args.start_line;
        let end_line = args.end_line;
        let replacement = args.replacement;

        if start_line == 0 || end_line == 0 || start_line > end_line {
            return Err(anyhow!(
                "Invalid line range: {} to {}",
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
                "pattern": { "type": "string", "description": "The search pattern (e.g. '*.rs', 'Cargo.toml', 'index.*')" },
                "dir": { "type": "string", "description": "The root directory to search in (defaults to '.')" }
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

        let run_cmd = |cmd: String| async move {
            let mut command = tokio::process::Command::new("sh");
            crate::config::loader::set_tokio_command_cwd(&mut command);
            command.arg("-c").arg(&cmd);
            let output = command.output().await?;
            let status = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Ok::<_, anyhow::Error>((status, format!("{}{}", stdout, stderr)))
        };

        let in_git = run_cmd("git rev-parse --is-inside-work-tree".to_string())
            .await
            .map(|(code, _)| code == 0)
            .unwrap_or(false);

        let mut committed = false;
        let original_content = fs::read_to_string(&path).ok();

        if in_git {
            let escaped_path = {
                let s = path.to_string_lossy();
                let mut escaped = String::with_capacity(s.len() + 2);
                escaped.push('\'');
                for c in s.chars() {
                    if c == '\'' {
                        escaped.push_str("'\\''");
                    } else {
                        escaped.push(c);
                    }
                }
                escaped.push('\'');
                escaped
            };
            let _ = run_cmd(format!("git add -- {}", escaped_path)).await;
            if let Ok((code, _)) =
                run_cmd("git commit -m \"Zenflow pre-edit backup\" --no-verify".to_string()).await
            {
                if code == 0 {
                    committed = true;
                }
            }
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;

        let (mut status, mut output_str) = run_cmd(compile_cmd.to_string()).await?;

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

            if let Ok(resp) = self
                .provider
                .chat(system_prompt, &messages, &[], &settings)
                .await
            {
                if let Some(healed_content) = resp.content {
                    let mut cleaned = healed_content.trim();
                    if cleaned.starts_with("```") {
                        if let Some(pos) = cleaned.find('\n') {
                            cleaned = &cleaned[pos + 1..];
                        }
                    }
                    if cleaned.ends_with("```") {
                        cleaned = cleaned[..cleaned.len() - 3].trim();
                    }
                    let cleaned_str = cleaned.trim().to_string();
                    if !cleaned_str.is_empty() {
                        fs::write(&path, &cleaned_str)?;
                        if let Ok((h_status, h_output)) = run_cmd(compile_cmd.to_string()).await {
                            status = h_status;
                            output_str = h_output;
                        }
                    }
                }
            }
        }

        if status == 0 {
            if committed {
                let _ = run_cmd("git reset HEAD~1".to_string()).await;
            }
            Ok(serde_json::json!({
                "status": "success",
                "message": "File written and verified successfully."
            }))
        } else {
            if let Some(orig) = original_content {
                fs::write(&path, orig)?;
            } else {
                let _ = fs::remove_file(&path);
            }
            if committed {
                let _ = run_cmd("git reset --mixed HEAD~1".to_string()).await;
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

    #[test]
    fn find_files_source_does_not_follow_symlinks_with_fd() {
        let source = std::fs::read_to_string("src/tools/filesystem.rs").unwrap();
        let follow_symlinks_arg = ["cmd.arg(\"", "-L", "\")"].join("");
        assert!(
            !source.contains(&follow_symlinks_arg),
            "find_files must not pass fd -L because it crosses symlink boundaries"
        );
    }

    fn run_git(dir: &std::path::Path, args: &[&str]) -> Result<()> {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()?;
        if !output.status.success() {
            return Err(anyhow!(
                "git {:?} failed: {}{}",
                args,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }

    #[tokio::test]
    async fn zenflow_edit_failure_preserves_unrelated_dirty_file() -> Result<()> {
        let repo =
            std::env::temp_dir().join(format!("openz_zenflow_rollback_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&repo)?;
        run_git(&repo, &["init"])?;
        run_git(
            &repo,
            &["config", "user.email", "openz-test@example.invalid"],
        )?;
        run_git(&repo, &["config", "user.name", "OpenZ Test"])?;

        let target = repo.join("target.txt");
        let unrelated = repo.join("unrelated.txt");
        std::fs::write(&target, "target initial\n")?;
        std::fs::write(&unrelated, "unrelated initial\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "-m", "initial"])?;

        std::fs::write(&target, "target dirty before edit\n")?;
        std::fs::write(&unrelated, "unrelated dirty must survive\n")?;

        let provider = std::sync::Arc::new(
            crate::providers::mock::MockProvider::new()
                .with_default(crate::providers::mock::MockResponse::text("")),
        );
        let tool = ZenflowEditTool { provider };
        let result = crate::config::loader::ACTIVE_WORKSPACE
            .scope(repo.clone(), async {
                tool.call(&serde_json::json!({
                    "path": "target.txt",
                    "content": "target attempted edit\n",
                    "compile_command": "false"
                }))
                .await
            })
            .await?;

        assert_eq!(result["status"], "error");
        assert_eq!(
            std::fs::read_to_string(&target)?,
            "target dirty before edit\n"
        );
        assert_eq!(
            std::fs::read_to_string(&unrelated)?,
            "unrelated dirty must survive\n"
        );

        let _ = std::fs::remove_dir_all(&repo);
        Ok(())
    }

    #[tokio::test]
    async fn filesystem_tools_accept_argument_aliases() -> Result<()> {
        let temp_dir =
            std::env::temp_dir().join(format!("openz_fs_alias_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        let file_path = temp_dir.join("alias_match.txt");

        let write = WriteFileTool;
        let write_res = write
            .call(&serde_json::json!({
                "filePath": file_path.to_str().unwrap(),
                "content": "one\ntwo\nthree\n"
            }))
            .await?;
        assert_eq!(write_res["status"], "success");

        let read = ReadFileTool;
        let read_res = read
            .call(&serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "startLine": 2,
                "endLine": 2
            }))
            .await?;
        assert_eq!(read_res, serde_json::Value::String("two".to_string()));

        let replace = ReplaceLinesTool;
        let replace_res = replace
            .call(&serde_json::json!({
                "filePath": file_path.to_str().unwrap(),
                "startLine": 2,
                "endLine": 2,
                "content": "TWO"
            }))
            .await?;
        assert_eq!(replace_res["status"], "success");
        assert_eq!(std::fs::read_to_string(&file_path)?, "one\nTWO\nthree\n");

        let patch = diffy::create_patch("one\nTWO\nthree\n", "one\nTWO\nTHREE\n");
        let patch_res = PatchFileTool
            .call(&serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "patch": patch.to_string()
            }))
            .await?;
        assert_eq!(patch_res["status"], "success");
        assert_eq!(std::fs::read_to_string(&file_path)?, "one\nTWO\nTHREE\n");

        let list = ListDirTool;
        let list_res = list
            .call(&serde_json::json!({
                "filePath": temp_dir.to_str().unwrap()
            }))
            .await?;
        assert!(
            list_res
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["name"] == "alias_match.txt")
        );

        let find = FindFilesTool;
        let find_res = find
            .call(&serde_json::json!({
                "glob": "alias_*.txt",
                "directory": temp_dir.to_str().unwrap()
            }))
            .await?;
        assert_eq!(find_res["status"], "success");
        assert!(find_res["results"].as_array().unwrap().iter().any(|entry| {
            entry
                .as_str()
                .unwrap_or_default()
                .ends_with("alias_match.txt")
        }));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_find_files() -> Result<()> {
        let temp_dir =
            std::env::temp_dir().join(format!("openz_find_test_{}", uuid::Uuid::new_v4()));
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
        let temp_dir =
            std::env::temp_dir().join(format!("openz_replace_test_{}", uuid::Uuid::new_v4()));
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

    #[test]
    fn zenflow_edit_source_does_not_use_hard_reset() {
        let source = std::fs::read_to_string("src/tools/filesystem.rs").unwrap();
        let hard_reset = ["git reset", "--hard", "HEAD~1"].join(" ");
        assert!(
            !source.contains(&format!("run_cmd(\"{}", hard_reset)),
            "zenflow_edit must not use destructive worktree-wide rollback"
        );
    }
}
