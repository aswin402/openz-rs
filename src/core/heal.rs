//! Core self-healing compilation and reflection engine.
//!
//! Provides unified compile-check command validation, code fence extraction,
//! automated pre-edit backups and rollback guards, and LLM reflection loops
//! for repairing syntax, compilation, and test failures.

use crate::agent::style::colors::{AURA_GOLD, AURA_PURPLE, COLOR_RESET, EMERALD_GREEN};
use crate::core::process::{host_tokio_shell_command, quote_shell_arg};
use crate::providers::{GenerationSettings, LLMProvider};
use crate::session::Message;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Default allowed compile / test / build command prefixes to protect against
/// unsafe shell command injection.
pub const ALLOWED_COMPILE_PREFIXES: &[&str] = &[
    "cargo ", "rustc ", "gcc ", "g++ ", "clang ", "clang++ ", "make ", "cmake ", "ninja ",
    "go ", "go build", "go test", "go run", "npm ", "npx ", "yarn ", "pnpm ", "node ",
    "python ", "python3 ", "pip ", "pip3 ", "javac ", "java ", "mvn ", "gradle ", "swift ",
    "swiftc ", "dotnet ", "msbuild ", "pytest ", "jest ", "vitest ", "tsc ",
    "false", "true", "sh ", "bash ", "echo ",
];

/// Validates that a compile or test verification command starts with a recognized, safe prefix.
pub fn validate_compile_command(cmd: &str) -> Result<()> {
    let cmd_lower = cmd.trim().to_lowercase();
    let is_allowed = ALLOWED_COMPILE_PREFIXES
        .iter()
        .any(|p| cmd_lower.starts_with(p) || cmd_lower == p.trim());
    match is_allowed {
        true => Ok(()),
        false => Err(anyhow!(
            "Compile command '{}' is not in the allowed list. Allowed: cargo, rustc, gcc, g++, clang, make, cmake, ninja, go, npm, npx, yarn, pnpm, node, python, javac, java, mvn, gradle, swift, dotnet, pytest, jest, vitest, tsc",
            cmd
        )),
    }
}

/// Strips surrounding markdown code block fences (e.g. ```rust\n...``` or ```\n...```)
/// and returns the inner code content.
pub fn strip_code_fence(text: &str) -> String {
    let mut cleaned = text.trim();
    match (cleaned.starts_with("```"), cleaned.find('\n')) {
        (true, Some(pos)) => cleaned = &cleaned[pos + 1..],
        (true, None) => cleaned = "",
        (false, _) => {}
    }
    match cleaned.ends_with("```") {
        true => cleaned = cleaned[..cleaned.len().saturating_sub(3)].trim_end(),
        false => {}
    }
    cleaned.to_string()
}

/// Executes a compilation or test command and returns `(success, combined_output)`.
pub async fn run_compile_check(compile_command: &str) -> Result<(bool, String)> {
    let mut cmd = host_tokio_shell_command(compile_command);
    let output = cmd.output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    Ok((output.status.success(), combined))
}

/// RAII file backup and rollback guard.
pub struct FileBackupGuard {
    target_path: PathBuf,
    original_content: Option<String>,
    backup_file_path: Option<PathBuf>,
    defused: bool,
}

impl FileBackupGuard {
    pub fn create(target_path: &Path, write_bak_file: bool) -> Result<Self> {
        let original_content = std::fs::read_to_string(target_path).ok();
        let backup_file_path = match write_bak_file && target_path.exists() {
            true => {
                let bak_path = target_path.with_extension(format!(
                    "{}.bak",
                    target_path.extension().and_then(|e| e.to_str()).unwrap_or("")
                ));
                match std::fs::copy(target_path, &bak_path) {
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Failed to create backup of {:?}: {}", target_path, e),
                }
                Some(bak_path)
            }
            false => None,
        };

        Ok(Self {
            target_path: target_path.to_path_buf(),
            original_content,
            backup_file_path,
            defused: false,
        })
    }

    pub fn defuse(&mut self) {
        self.defused = true;
    }

    pub fn rollback(&mut self) -> Result<()> {
        match (&self.original_content, self.target_path.exists()) {
            (Some(orig), _) => std::fs::write(&self.target_path, orig)?,
            (None, true) => {
                let _ = std::fs::remove_file(&self.target_path);
            }
            (None, false) => {}
        }
        self.defused = true;
        Ok(())
    }

    pub fn backup_file_path(&self) -> Option<&Path> {
        self.backup_file_path.as_deref()
    }
}

impl Drop for FileBackupGuard {
    fn drop(&mut self) {
        match self.defused {
            true => {}
            false => {
                let _ = self.rollback();
            }
        }
    }
}

/// Optional git snapshot helper to protect worktrees during speculative code edits.
pub struct GitSnapshot {
    committed: bool,
}

impl GitSnapshot {
    pub async fn try_create(path: &Path) -> Self {
        let is_git = async {
            let mut cmd = host_tokio_shell_command("git rev-parse --is-inside-work-tree");
            let out = cmd.output().await.ok()?;
            Some(out.status.success())
        }
        .await
        .unwrap_or(false);

        match is_git {
            false => return Self { committed: false },
            true => {}
        }

        let escaped = quote_shell_arg(&path.to_string_lossy());
        let mut add_cmd = host_tokio_shell_command(&format!("git add -- {escaped}"));
        let _ = add_cmd.output().await;

        let mut commit_cmd =
            host_tokio_shell_command("git commit -m \"Zenflow pre-edit backup\" --no-verify");
        let committed = match commit_cmd.output().await {
            Ok(out) => out.status.success(),
            Err(_) => false,
        };

        Self { committed }
    }

    pub async fn on_success(self) {
        match self.committed {
            true => {
                let mut reset_cmd = host_tokio_shell_command("git reset HEAD~1");
                let _ = reset_cmd.output().await;
            }
            false => {}
        }
    }

    pub async fn on_failure(self) {
        match self.committed {
            true => {
                let mut reset_cmd = host_tokio_shell_command("git reset --mixed HEAD~1");
                let _ = reset_cmd.output().await;
            }
            false => {}
        }
    }
}

/// Invokes the LLM provider to repair compilation or test errors in a file.
pub async fn heal_code_with_llm(
    provider: &dyn LLMProvider,
    file_path: &Path,
    content: &str,
    compile_error: &str,
) -> Result<String> {
    let system_prompt =
        "You are a Self-Healing Code Assistant. Fix compile/test errors in the provided file.";
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
        file_path.to_string_lossy(),
        content,
        compile_error
    );

    let messages = vec![Message {
        role: "user".to_string(),
        content: user_prompt,
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra: serde_json::Map::new(),
    }];

    let settings = GenerationSettings {
        temperature: 0.1,
        max_tokens: 4096,
        reasoning_effort: None,
    };

    let resp = provider.chat(system_prompt, &messages, &[], &settings).await?;
    let raw = resp
        .content
        .ok_or_else(|| anyhow!("No content returned from AI"))?;

    Ok(strip_code_fence(&raw))
}

fn is_binary_file_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "docx"
            | "xlsx"
            | "pdf"
            | "pptx"
            | "zip"
            | "png"
            | "jpg"
            | "jpeg"
            | "bin"
            | "exe"
            | "tar"
            | "gz"
            | "so"
            | "dylib"
    )
}

/// Runs an iterative edit-and-compile reflection loop modifying `file_path`
/// until `compile_command` succeeds or `max_iterations` is reached.
pub async fn run_compiler_auto_heal(
    provider: &dyn LLMProvider,
    file_path: &Path,
    instruction: &str,
    compile_command: &str,
    max_iterations: usize,
) -> Result<serde_json::Value> {
    validate_compile_command(compile_command)?;

    match file_path.exists() {
        false => return Err(anyhow!("File does not exist: {:?}", file_path)),
        true => {}
    }

    match file_path
        .extension()
        .and_then(|e| e.to_str())
        .filter(|ext| is_binary_file_extension(ext))
    {
        Some(ext) => {
            return Err(anyhow!(
                "compiler_auto_heal only supports text/code source files, not binary format '.{ext}'"
            ));
        }
        None => {}
    }

    let mut backup = FileBackupGuard::create(file_path, true)?;
    let mut file_content = std::fs::read_to_string(file_path)?;
    let mut current_error = String::new();
    let mut iteration = 0;
    let mut compile_success = false;

    let system_prompt = "You are an expert compiler auto-healing agent. Your goal is to modify the provided file content based on the instruction and ensure it compiles without errors.\n\
    You must return the COMPLETE updated file content. Do not truncate, do not use comments for unchanged parts, do not output any explanation text or greetings.\n\
    Output your response inside a single markdown code block (e.g. ```rust ... ``` or ```javascript ... ```).";

    while iteration < max_iterations {
        iteration += 1;
        crate::tui_println!(
            "{}🔧 [Compiler Auto-Heal] Iteration {}/{} for {}...{}",
            AURA_PURPLE,
            iteration,
            max_iterations,
            file_path.file_name().unwrap_or_default().to_string_lossy(),
            COLOR_RESET
        );

        let user_prompt = match current_error.is_empty() {
            true => format!(
                "TARGET FILE: {:?}\n\n\
                 CURRENT CONTENT:\n\
                 ```\n\
                 {}\n\
                 ```\n\n\
                 INSTRUCTION: {}\n\n\
                 Please edit the file content to satisfy the instruction, and output the complete new file content.",
                file_path, file_content, instruction
            ),
            false => format!(
                "TARGET FILE: {:?}\n\n\
                 CURRENT CONTENT:\n\
                 ```\n\
                 {}\n\
                 ```\n\n\
                 The previous code failed compilation with the following error:\n\
                 ```\n\
                 {}\n\
                 ```\n\n\
                 Please fix this compilation error and output the complete corrected file content.",
                file_path, file_content, current_error
            ),
        };

        let messages = vec![Message {
            role: "user".to_string(),
            content: user_prompt,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            extra: serde_json::Map::new(),
        }];

        let settings = GenerationSettings {
            temperature: 0.1,
            max_tokens: 4096,
            reasoning_effort: None,
        };

        let resp = provider.chat(system_prompt, &messages, &[], &settings).await?;
        let response_text = resp
            .content
            .ok_or_else(|| anyhow!("No content returned from AI"))?;

        let updated_content_str = strip_code_fence(&response_text);
        std::fs::write(file_path, &updated_content_str)?;
        file_content = updated_content_str;

        let (success, output_str) = run_compile_check(compile_command).await?;
        match success {
            true => {
                compile_success = true;
                crate::tui_println!(
                    "{}✓ [Compiler Auto-Heal] Compilation succeeded! (Iteration {}){}",
                    EMERALD_GREEN,
                    iteration,
                    COLOR_RESET
                );
                break;
            }
            false => {
                current_error = output_str;
                crate::tui_println!(
                    "{}▲ [Compiler Auto-Heal] Compilation failed. Error output captured.{}",
                    AURA_GOLD,
                    COLOR_RESET
                );
            }
        }
    }

    match compile_success {
        true => {
            backup.defuse();
            Ok(serde_json::json!({
                "status": "success",
                "message": "File edited and compiled successfully",
                "iterations": iteration
            }))
        }
        false => Ok(serde_json::json!({
            "status": "failed",
            "message": "Failed to compile within the maximum number of iterations",
            "error": current_error,
            "iterations": iteration
        })),
    }
}

/// Writes `new_content` to `file_path`, executes `compile_command`, and if compilation
/// fails, attempts self-healing with the LLM. If healing succeeds, persists changes;
/// otherwise rolls back changes.
pub async fn run_transactional_heal_edit(
    provider: &dyn LLMProvider,
    file_path: &Path,
    new_content: &str,
    compile_command: &str,
) -> Result<serde_json::Value> {
    validate_compile_command(compile_command)?;

    let git_snapshot = GitSnapshot::try_create(file_path).await;
    let mut backup = FileBackupGuard::create(file_path, false)?;

    match file_path.parent() {
        Some(parent) => std::fs::create_dir_all(parent)?,
        None => {}
    }
    std::fs::write(file_path, new_content)?;

    let (mut success, mut output_str) = run_compile_check(compile_command).await?;

    match success {
        true => {}
        false => match heal_code_with_llm(provider, file_path, new_content, &output_str)
            .await
            .ok()
            .filter(|healed| !healed.is_empty())
        {
            Some(healed_content) => {
                std::fs::write(file_path, &healed_content)?;
                match run_compile_check(compile_command).await {
                    Ok((h_success, h_output)) => {
                        success = h_success;
                        output_str = h_output;
                    }
                    Err(_) => {}
                }
            }
            None => {}
        },
    }

    match success {
        true => {
            backup.defuse();
            git_snapshot.on_success().await;
            Ok(serde_json::json!({
                "status": "success",
                "message": "File written and verified successfully."
            }))
        }
        false => {
            let _ = backup.rollback();
            git_snapshot.on_failure().await;
            Ok(serde_json::json!({
                "status": "error",
                "error": format!("Compilation failed, self-healing failed. Rolled back changes. Error output:\n{output_str}")
            }))
        }
    }
}

#[cfg(test)]
#[path = "heal_tests.rs"]
mod tests;
