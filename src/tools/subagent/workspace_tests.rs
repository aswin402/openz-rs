use super::*;
use anyhow::Result;
use crate::tools::subagent::attach_workspace_fields;

static TEST_WORKTREE_REGISTRY_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> =
    std::sync::OnceLock::new();

fn worktree_registry_test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_WORKTREE_REGISTRY_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn test_worktree_guard_unregisters_on_drop() -> Result<()> {
    let _guard = worktree_registry_test_guard();
    let parent = std::env::temp_dir().join(format!(
        "openz_worktree_guard_parent_{}",
        uuid::Uuid::new_v4()
    ));
    let worktree = std::env::temp_dir().join(format!(
        "openz_worktree_guard_child_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&parent)?;
    std::fs::create_dir_all(&worktree)?;

    {
        let _guard = WorktreeGuard::new(parent.clone(), worktree.clone());
        assert!(
            has_registered_worktree_cleanup_for_test(&worktree),
            "guard should register shutdown cleanup"
        );
    }

    assert!(
        !worktree.exists(),
        "normal guard drop should clean the worktree"
    );
    assert!(
        !has_registered_worktree_cleanup_for_test(&worktree),
        "normal drop should unregister shutdown cleanup"
    );

    let _ = std::fs::remove_dir_all(&parent);
    Ok(())
}

#[test]
fn test_registered_worktree_cleanup_handles_forced_shutdown_path() -> Result<()> {
    let _guard = worktree_registry_test_guard();
    let parent = std::env::temp_dir().join(format!(
        "openz_worktree_forced_parent_{}",
        uuid::Uuid::new_v4()
    ));
    let worktree = std::env::temp_dir().join(format!(
        "openz_worktree_forced_child_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&parent)?;
    std::fs::create_dir_all(&worktree)?;

    let mut guard = WorktreeGuard::new(parent.clone(), worktree.clone());
    assert!(
        has_registered_worktree_cleanup_for_test(&worktree),
        "guard should register shutdown cleanup"
    );

    cleanup_registered_worktrees();

    assert!(
        !worktree.exists(),
        "shutdown cleanup should remove registered worktree before guard drop"
    );
    assert!(
        !has_registered_worktree_cleanup_for_test(&worktree),
        "shutdown cleanup should drain registry entry"
    );

    guard.deactivate();
    drop(guard);
    let _ = std::fs::remove_dir_all(&parent);
    Ok(())
}

#[test]
fn test_worktree_cleanup_removes_old_openz_worktrees() -> Result<()> {
    let root = std::env::temp_dir().join(format!(
        "openz_worktree_cleanup_age_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root)?;
    let old_dir = root.join("openz_worktree_old");
    let fresh_dir = root.join("openz_worktree_fresh");
    let unrelated_dir = root.join("not_openz_worktree_old");
    std::fs::create_dir_all(&old_dir)?;
    std::fs::create_dir_all(&fresh_dir)?;
    std::fs::create_dir_all(&unrelated_dir)?;
    std::fs::write(old_dir.join("data.txt"), b"old")?;
    std::fs::write(fresh_dir.join("data.txt"), b"fresh")?;
    std::fs::write(unrelated_dir.join("data.txt"), b"keep")?;

    let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
    set_directory_modified_time_for_test(&old_dir, old_time)?;

    cleanup_worktrees_dir(
        &root,
        WorktreeCleanupPolicy {
            max_age: std::time::Duration::from_secs(3600),
            max_count: 10,
            max_total_bytes: 1024 * 1024,
            min_free_bytes: 0,
        },
    );

    assert!(!old_dir.exists(), "old OpenZ worktree should be deleted");
    assert!(fresh_dir.exists(), "fresh OpenZ worktree should be kept");
    assert!(
        unrelated_dir.exists(),
        "non-OpenZ directory must never be deleted"
    );
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn test_worktree_cleanup_enforces_total_size_quota_oldest_first() -> Result<()> {
    let root = std::env::temp_dir().join(format!(
        "openz_worktree_cleanup_quota_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root)?;
    let old_dir = root.join("openz_worktree_old");
    let mid_dir = root.join("openz_worktree_mid");
    let new_dir = root.join("openz_worktree_new");
    for dir in [&old_dir, &mid_dir, &new_dir] {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("blob.bin"), vec![0_u8; 1024])?;
    }

    let now = std::time::SystemTime::now();
    set_directory_modified_time_for_test(
        &old_dir,
        now - std::time::Duration::from_secs(300),
    )?;
    set_directory_modified_time_for_test(
        &mid_dir,
        now - std::time::Duration::from_secs(200),
    )?;
    set_directory_modified_time_for_test(
        &new_dir,
        now - std::time::Duration::from_secs(100),
    )?;

    cleanup_worktrees_dir(
        &root,
        WorktreeCleanupPolicy {
            max_age: std::time::Duration::from_secs(3600),
            max_count: 10,
            max_total_bytes: 2048,
            min_free_bytes: 0,
        },
    );

    assert!(
        !old_dir.exists(),
        "oldest worktree should be deleted to satisfy size quota"
    );
    assert!(mid_dir.exists(), "middle worktree should remain");
    assert!(new_dir.exists(), "newest worktree should remain");
    assert!(directory_size_bytes(&root) <= 2048);
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn test_scratch_workspace_teardown_message_skips_branch_commit_wording() {
    let msg = simulation_space_teardown_message(true, "branch_test", true);

    assert!(msg.contains("Scratch workspace completed"));
    assert!(msg.contains("sync-back skipped"));
    assert!(!msg.contains("Committed simulation space branch"));
}

#[tokio::test]
async fn test_create_isolated_workspace_uses_scratch_for_home_like_non_git_root() -> Result<()> {
    let _lock = crate::tools::graph_memory::test_lock().lock().await;
    let temp_root = std::env::temp_dir().join(format!(
        "openz_home_like_worktree_guard_{}",
        uuid::Uuid::new_v4()
    ));
    let fake_home = temp_root.join("home");
    let config_dir = temp_root.join("openz_config");
    std::fs::create_dir_all(fake_home.join(".cache/big"))?;
    std::fs::create_dir_all(&config_dir)?;
    std::fs::write(fake_home.join(".cache/big/blob.bin"), vec![0_u8; 1024])?;

    let old_home = std::env::var_os("HOME");
    std::env::set_var("HOME", &fake_home);

    let workspace = crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(config_dir.clone(), async {
            create_isolated_workspace(&fake_home)
        })
        .await?;

    if let Some(old_home) = old_home {
        std::env::set_var("HOME", old_home);
    } else {
        std::env::remove_var("HOME");
    }

    assert_ne!(
        workspace, fake_home,
        "home-like roots must not run subagents in the active workspace"
    );
    assert!(
        workspace.starts_with(config_dir.join("worktrees")),
        "scratch workspace should be created under OpenZ worktrees dir: {workspace:?}"
    );
    assert!(
        workspace.exists(),
        "scratch workspace should exist for subagents launched from arbitrary directories"
    );
    assert!(
        !workspace.join(".cache").exists(),
        "scratch workspace must not recursively copy user cache directories"
    );
    assert!(
        !workspace.join(".cache/big/blob.bin").exists(),
        "scratch workspace must not copy home contents"
    );

    let _ = std::fs::remove_dir_all(&temp_root);
    Ok(())
}

#[test]
fn test_fallback_copy_skips_heavy_user_cache_dirs() -> Result<()> {
    let src =
        std::env::temp_dir().join(format!("openz_filtered_copy_src_{}", uuid::Uuid::new_v4()));
    let dst =
        std::env::temp_dir().join(format!("openz_filtered_copy_dst_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(src.join("src"))?;
    std::fs::create_dir_all(src.join(".cache/huge"))?;
    std::fs::create_dir_all(src.join(".local/share"))?;
    std::fs::create_dir_all(src.join(".openz/worktrees"))?;
    std::fs::create_dir_all(src.join("Downloads"))?;
    std::fs::write(src.join("src/main.rs"), "fn main() {}")?;
    std::fs::write(src.join(".cache/huge/blob.bin"), vec![0_u8; 1024])?;
    std::fs::write(src.join(".local/share/blob.bin"), vec![0_u8; 1024])?;
    std::fs::write(src.join(".openz/worktrees/blob.bin"), vec![0_u8; 1024])?;
    std::fs::write(src.join("Downloads/blob.bin"), vec![0_u8; 1024])?;

    copy_dir_recursive_filtered(&src, &dst)?;

    assert!(dst.join("src/main.rs").exists());
    assert!(!dst.join(".cache").exists());
    assert!(!dst.join(".local").exists());
    assert!(!dst.join(".openz").exists());
    assert!(!dst.join("Downloads").exists());

    let _ = std::fs::remove_dir_all(&src);
    let _ = std::fs::remove_dir_all(&dst);
    Ok(())
}

#[test]
fn attach_workspace_fields_merges_isolation_outcome() {
    let base = serde_json::json!({"status": "cancelled", "session_id": "subagent:x"});
    let merged = attach_workspace_fields(
        base,
        "scratch_workspace",
        &Some("unsafe to copy".to_string()),
    );
    assert_eq!(merged["workspaceIsolation"], "scratch_workspace");
    assert_eq!(merged["workspaceIsolationReason"], "unsafe to copy");
    assert_eq!(merged["status"], "cancelled");

    let merged_null = attach_workspace_fields(
        serde_json::json!({"status": "cancelled"}),
        "isolated_worktree",
        &None,
    );
    assert_eq!(
        merged_null["workspaceIsolationReason"],
        serde_json::Value::Null
    );
}
