//! Shared path resolution and containment policy.
//!
//! This module owns the common, symlink-aware mechanics. Callers choose a
//! policy mode for their existing contract instead of reimplementing path
//! canonicalization and missing-leaf handling.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub const SENSITIVE_EXACT: &[&str] = &[
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

pub const SENSITIVE_COMPONENTS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".azure",
    ".gcloud",
    ".docker",
    ".kube",
    "browser_profiles",
];

pub const FORBIDDEN_ROOTS: &[&str] = &["/proc", "/sys", "/dev"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPolicyMode {
    /// Workspace plus OpenZ runtime and temporary roots, matching the native
    /// filesystem/database safety contract.
    Workspace,
    /// Only the configured OpenZ runtime data root.
    RuntimeData,
    /// Headroom’s workspace-aware sensitive-file policy.
    HeadroomSensitive,
    /// A path supplied to an approval prompt. Validation is containment-only;
    /// the caller remains responsible for requesting approval.
    ApprovalTarget,
}

#[derive(Debug, Clone, Copy)]
pub struct PathPolicy {
    mode: PathPolicyMode,
}

impl PathPolicy {
    pub const fn new(mode: PathPolicyMode) -> Self {
        Self { mode }
    }

    pub const fn workspace() -> Self {
        Self::new(PathPolicyMode::Workspace)
    }

    pub const fn runtime_data() -> Self {
        Self::new(PathPolicyMode::RuntimeData)
    }

    pub const fn headroom_sensitive() -> Self {
        Self::new(PathPolicyMode::HeadroomSensitive)
    }

    pub const fn approval_target() -> Self {
        Self::new(PathPolicyMode::ApprovalTarget)
    }

    pub fn resolve_and_validate(&self, input: &str) -> Result<PathBuf> {
        let resolved = crate::config::loader::resolve_path(input);
        self.validate(&resolved)
    }

    pub fn validate(&self, path: &Path) -> Result<PathBuf> {
        let canonical = canonicalize_with_missing_leaf(path);

        match self.mode {
            PathPolicyMode::Workspace | PathPolicyMode::ApprovalTarget => {
                let allowed = [
                    crate::config::loader::active_workspace_or_current_dir(),
                    crate::config::loader::config_dir(),
                    std::env::temp_dir(),
                ];
                if !allowed
                    .iter()
                    .any(|root| canonical.starts_with(canonicalize_with_missing_leaf(root)))
                {
                    return Err(anyhow!(
                        "Path traversal prevention: Path {:?} is not allowed (must be inside workspace, ~/.openz, or temp)",
                        path
                    ));
                }
            }
            PathPolicyMode::RuntimeData => {
                let runtime_root = canonicalize_with_missing_leaf(
                    &crate::config::loader::runtime_data_dir(),
                );
                if !canonical.starts_with(&runtime_root) {
                    return Err(anyhow!(
                        "runtime path {:?} is outside OpenZ runtime data directory",
                        path
                    ));
                }
            }
            PathPolicyMode::HeadroomSensitive => {
                if FORBIDDEN_ROOTS.iter().any(|root| {
                    canonical == Path::new(root)
                        || canonical.starts_with(Path::new(root).join(""))
                }) {
                    return Err(anyhow!(
                        "path '{}' is blocked by headroom path policy",
                        canonical.display()
                    ));
                }

                if let Ok(root) = std::env::var("HEADROOM_WORKSPACE") {
                    if !root.trim().is_empty()
                        && !canonical.starts_with(canonicalize_with_missing_leaf(Path::new(&root)))
                    {
                        return Err(anyhow!(
                            "path '{}' is outside workspace root '{}'",
                            canonical.display(),
                            root
                        ));
                    }
                }

                for component in canonical.components() {
                    let lower = component.as_os_str().to_string_lossy().to_lowercase();
                    if SENSITIVE_COMPONENTS.iter().any(|name| lower == *name) {
                        return Err(anyhow!("sensitive path component '{}' is blocked", lower));
                    }
                }

                if let Some(name) = canonical.file_name().and_then(|name| name.to_str()) {
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
            }
        }

        Ok(path.to_path_buf())
    }
}

pub(crate) fn canonicalize_with_missing_leaf(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }

    let mut cursor = path.to_path_buf();
    let mut missing = Vec::new();
    while !cursor.exists() {
        if let Some(name) = cursor.file_name() {
            missing.push(name.to_os_string());
        }
        if !cursor.pop() {
            return path.to_path_buf();
        }
    }

    let mut canonical = cursor.canonicalize().unwrap_or(cursor);
    for component in missing.iter().rev() {
        canonical.push(component);
    }
    canonical
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_policy_preserves_workspace_boundaries() {
        let workspace = crate::config::loader::active_workspace_or_current_dir();
        let allowed = workspace.join("path-policy-test.txt");
        assert!(PathPolicy::workspace().validate(&allowed).is_ok());

        #[cfg(not(target_os = "windows"))]
        assert!(PathPolicy::workspace()
            .validate(Path::new("/etc/hosts"))
            .is_err());
    }

    #[test]
    fn path_policy_accepts_missing_leaf_inside_workspace() {
        let workspace = crate::config::loader::active_workspace_or_current_dir();
        let missing = workspace
            .join("path-policy-missing-parent")
            .join("new-file.txt");

        assert!(PathPolicy::workspace().validate(&missing).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn path_policy_rejects_symlink_escape_from_allowed_root() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "openz-path-policy-symlink-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("create path-policy test root");
        let link = root.join("workspace-link");
        symlink("/etc", &link).expect("create path-policy symlink");

        let result = PathPolicy::workspace().validate(&link.join("hosts"));

        std::fs::remove_dir_all(&root).expect("remove path-policy test root");
        assert!(result.is_err());
    }

    #[test]
    fn path_policy_preserves_headroom_sensitive_rejections() {
        assert!(PathPolicy::headroom_sensitive()
            .validate(Path::new("/tmp/.env"))
            .is_err());
        assert!(PathPolicy::headroom_sensitive()
            .validate(Path::new("/tmp/.ssh/id_ed25519"))
            .is_err());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn path_policy_rejects_forbidden_headroom_roots() {
        assert!(PathPolicy::headroom_sensitive()
            .validate(Path::new("/proc/self/status"))
            .is_err());
        assert!(PathPolicy::headroom_sensitive()
            .validate(Path::new("/sys/kernel"))
            .is_err());
    }
}
