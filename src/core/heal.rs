//! Core self-healing compilation and reflection engine.
//!
//! Provides unified compile-check command validation, code fence extraction,
//! automated pre-edit backups and rollback guards, and LLM reflection loops
//! for repairing syntax, compilation, and test failures.

use crate::agent::style::colors::{AURA_GOLD, AURA_PURPLE, COLOR_RESET, EMERALD_GREEN};
use crate::core::process::host_tokio_shell_command;
use crate::providers::{GenerationSettings, LLMProvider};
use crate::session::Message;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Default allowed compile / test / build command prefixes to protect against
/// unsafe shell command injection.
pub const ALLOWED_COMPILE_PREFIXES: &[&str] = &[
    "cargo ", "rustc ", "gcc ", "g++ ", "clang ", "clang++ ", "make ", "cmake ", "ninja ",
    "go ", "go build", "go test", "go run", "npm ", "npx ", "yarn ", "pnpm ", "node ",
    "bun ", "bunx ", "just ", "uv ", "deno ", "zig ",
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
            "Compile command '{}' is not in the allowed list. Allowed: cargo, rustc, gcc, g++, clang, make, cmake, ninja, go, npm, npx, yarn, pnpm, node, bun, bunx, just, uv, deno, zig, python, javac, java, mvn, gradle, swift, dotnet, pytest, jest, vitest, tsc",
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

pub fn is_binary_file_extension(ext: &str) -> bool {
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
            | "gif"
            | "ico"
            | "bin"
            | "exe"
            | "dll"
            | "tar"
            | "gz"
            | "7z"
            | "rar"
            | "so"
            | "dylib"
            | "db"
            | "sqlite"
            | "wasm"
            | "woff"
            | "woff2"
            | "ttf"
            | "eot"
            | "mp4"
            | "mp3"
            | "wav"
            | "avi"
            | "mov"
            | "out"
            | "o"
            | "a"
    )
}

pub fn has_binary_file_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(is_binary_file_extension)
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

/// Multi-file RAII backup and atomic rollback guard.
///
/// Snapshots original contents (or non-existence) for an entire batch of files
/// in memory without creating temporary `git commit` / `git reset` mutations on
/// the active branch.
pub struct MultiFileBackupGuard {
    guards: Vec<FileBackupGuard>,
}

impl MultiFileBackupGuard {
    pub fn create(paths: &[PathBuf]) -> Result<Self> {
        let mut seen = std::collections::BTreeSet::new();
        let mut guards = Vec::with_capacity(paths.len());
        for path in paths {
            let inserted = seen.insert(path.clone());
            match inserted {
                true => guards.push(FileBackupGuard::create(path, false)?),
                false => {}
            }
        }
        Ok(Self { guards })
    }

    pub fn defuse(&mut self) {
        for guard in &mut self.guards {
            guard.defuse();
        }
    }

    pub fn rollback(&mut self) -> Result<()> {
        for guard in self.guards.iter_mut().rev() {
            guard.rollback()?;
        }
        Ok(())
    }
}

fn select_heal_target_index(edits: &[(PathBuf, String)], compiler_output: &str) -> Option<usize> {
    match edits.len() {
        0 => None,
        1 => Some(0),
        _ => edits.iter().enumerate().find_map(|(idx, (path, _))| {
            let full = path.to_string_lossy();
            let base = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let hit = compiler_output.contains(full.as_ref())
                || (!base.is_empty() && compiler_output.contains(base));
            hit.then_some(idx)
        }),
    }
}

/// Writes one or more `(path, content)` edits atomically, executes `compile_command`,
/// attempts targeted LLM self-healing on compilation failure, and rolls back every
/// modified or newly created file in the batch when verification fails.
pub async fn run_transactional_multi_heal_edit(
    provider: &dyn LLMProvider,
    edits: &[(PathBuf, String)],
    compile_command: &str,
) -> Result<serde_json::Value> {
    validate_compile_command(compile_command)?;
    (!edits.is_empty())
        .then_some(())
        .ok_or_else(|| anyhow!("zenflow_edit requires at least one file edit"))?;

    for (path, _) in edits {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .filter(|ext| is_binary_file_extension(ext))
        {
            Some(ext) => {
                return Err(anyhow!(
                    "zenflow_edit only supports text/code source files, not binary format '.{ext}'"
                ));
            }
            None => {}
        }
    }

    // Acquire exclusive per-file async locks in deterministic sorted path order
    // so concurrent multi-file batches can never deadlock.
    let mut sorted_paths: Vec<PathBuf> = edits.iter().map(|(p, _)| p.clone()).collect();
    sorted_paths.sort();
    sorted_paths.dedup();

    let mut lock_guards = Vec::with_capacity(sorted_paths.len());
    for p in &sorted_paths {
        lock_guards.push(crate::tools::filesystem::acquire_file_mutation_lock(p).await);
    }

    let mut backup = MultiFileBackupGuard::create(&sorted_paths)?;
    let mut current_edits = edits.to_vec();

    for (path, content) in &current_edits {
        match path.parent() {
            Some(parent) => std::fs::create_dir_all(parent)?,
            None => {}
        }
        std::fs::write(path, content)?;
    }

    let (mut success, mut output_str) = run_compile_check(compile_command).await?;
    let mut healed = false;

    match success {
        true => {}
        false => match select_heal_target_index(&current_edits, &output_str) {
            Some(target_idx) => {
                let (target_path, target_content) = &current_edits[target_idx];
                match heal_code_with_llm(provider, target_path, target_content, &output_str)
                    .await
                    .ok()
                    .filter(|candidate| !candidate.is_empty())
                {
                    Some(healed_content) => {
                        std::fs::write(target_path, &healed_content)?;
                        current_edits[target_idx].1 = healed_content;
                        match run_compile_check(compile_command).await {
                            Ok((h_success, h_output)) => {
                                healed = h_success;
                                success = h_success;
                                output_str = h_output;
                            }
                            Err(_) => {}
                        }
                    }
                    None => {}
                }
            }
            None => {}
        },
    }

    drop(lock_guards);

    match success {
        true => {
            backup.defuse();
            Ok(serde_json::json!({
                "status": "success",
                "files_verified": current_edits.len(),
                "healed": healed,
                "message": "File(s) written and verified successfully."
            }))
        }
        false => {
            let _ = backup.rollback();
            Ok(serde_json::json!({
                "status": "error",
                "rolled_back_files": current_edits.len(),
                "error": format!("Compilation failed, self-healing failed. Rolled back changes. Error output:\n{output_str}")
            }))
        }
    }
}

/// Writes `new_content` to `file_path`, executes `compile_command`, and when compilation
/// fails, attempts self-healing with the LLM. Persists on success; rolls back on failure.
pub async fn run_transactional_heal_edit(
    provider: &dyn LLMProvider,
    file_path: &Path,
    new_content: &str,
    compile_command: &str,
) -> Result<serde_json::Value> {
    run_transactional_multi_heal_edit(
        provider,
        &[(file_path.to_path_buf(), new_content.to_string())],
        compile_command,
    )
    .await
}

#[cfg(test)]
#[path = "heal_tests.rs"]
mod tests;
