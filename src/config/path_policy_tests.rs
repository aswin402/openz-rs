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
