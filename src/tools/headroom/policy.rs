use anyhow::{anyhow, Result};
use crate::config::path_policy::PathPolicy;
use std::path::{Path, PathBuf};

pub const MAX_CACHE_ALIGN_PADDING: usize = 65_536;
pub const MAX_RUN_OUTPUT_BYTES: usize = 512_000;
pub const MAX_RUN_TIMEOUT_SECS: u64 = 120;

pub fn resolve_user_path(input: &str) -> Result<PathBuf> {
    let raw = input.trim_start_matches("file://");
    let path = Path::new(raw);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| anyhow!("failed to get cwd: {}", e))?
            .join(path)
    };
    absolute
        .canonicalize()
        .map_err(|e| anyhow!("failed to resolve path '{}': {}", input, e))
}

pub fn ensure_path_is_safe_for_headroom(path: &Path) -> Result<()> {
    PathPolicy::headroom_sensitive().validate(path).map(|_| ())
}

pub fn command_is_allowed(command: &str) -> bool {
    let base = Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);
    matches!(
        base,
        "cargo"
            | "git"
            | "rg"
            | "grep"
            | "sed"
            | "ls"
            | "find"
            | "pwd"
            | "cat"
            | "wc"
            | "head"
            | "tail"
            | "sort"
            | "uniq"
            | "du"
            | "df"
    )
}

pub fn resolve_output_path(input: &str) -> Result<PathBuf> {
    let raw = input.trim_start_matches("file://");
    let path = Path::new(raw);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| anyhow!("failed to get cwd: {}", e))?
            .join(path)
    };

    if let Some(parent) = absolute.parent() {
        parent.canonicalize().map_err(|e| {
            anyhow!(
                "failed to resolve parent directory '{}': {}",
                parent.display(),
                e
            )
        })?;
    }

    ensure_path_is_safe_for_headroom(&absolute)?;
    Ok(absolute)
}
