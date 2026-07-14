use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub const MAX_CACHE_ALIGN_PADDING: usize = 65_536;
pub const MAX_RUN_OUTPUT_BYTES: usize = 512_000;
pub const MAX_RUN_TIMEOUT_SECS: u64 = 120;

const SENSITIVE_EXACT: &[&str] = &[
    ".env",
    ".env.local",
    ".envrc",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    "credentials",
    "credentials.json",
    "secrets.json",
    "known_hosts",
];

const SENSITIVE_COMPONENTS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".azure",
    ".gcloud",
    ".docker",
    ".kube",
    "browser_profiles",
];

const FORBIDDEN_ROOTS: &[&str] = &["/proc", "/sys", "/dev"];

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
    let display = path.to_string_lossy();
    if let Ok(root) = std::env::var("HEADROOM_WORKSPACE") {
        if !root.trim().is_empty() {
            let workspace = Path::new(&root)
                .canonicalize()
                .map_err(|e| anyhow!("failed to resolve HEADROOM_WORKSPACE '{}': {}", root, e))?;
            let canonical = if path.exists() {
                path.canonicalize()?
            } else if let Some(parent) = path.parent() {
                parent
                    .canonicalize()?
                    .join(path.file_name().unwrap_or_default())
            } else {
                path.to_path_buf()
            };
            if !canonical.starts_with(&workspace) {
                return Err(anyhow!(
                    "path '{}' is outside workspace root '{}'",
                    canonical.display(),
                    workspace.display()
                ));
            }
        }
    }
    if FORBIDDEN_ROOTS
        .iter()
        .any(|root| display == *root || display.starts_with(&format!("{}/", root)))
    {
        return Err(anyhow!(
            "path '{}' is blocked by headroom path policy",
            display
        ));
    }

    for component in path.components() {
        let lower = component.as_os_str().to_string_lossy().to_lowercase();
        if SENSITIVE_COMPONENTS.iter().any(|name| lower == *name) {
            return Err(anyhow!("sensitive path component '{}' is blocked", lower));
        }
    }

    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let lower = name.to_lowercase();
        if SENSITIVE_EXACT.iter().any(|blocked| lower == *blocked)
            || lower.contains("secret")
            || lower.contains("token")
            || lower.ends_with(".pem")
            || lower.ends_with(".key")
            || lower.ends_with(".p12")
            || lower.ends_with(".pfx")
        {
            return Err(anyhow!("sensitive file '{}' is blocked", name));
        }
    }

    Ok(())
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
