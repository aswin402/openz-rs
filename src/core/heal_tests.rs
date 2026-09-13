use super::*;

#[test]
fn test_validate_compile_command() {
    assert!(validate_compile_command("cargo check").is_ok());
    assert!(validate_compile_command("npm run build").is_ok());
    assert!(validate_compile_command("python3 -m pytest").is_ok());
    assert!(validate_compile_command("false").is_ok());
    assert!(validate_compile_command("true").is_ok());
    assert!(validate_compile_command("rm -rf /").is_err());
    assert!(validate_compile_command("curl http://example.com").is_err());
}

#[test]
fn test_strip_code_fence() {
    assert_eq!(
        strip_code_fence("```rust\nfn main() {}\n```"),
        "fn main() {}"
    );
    assert_eq!(
        strip_code_fence("```\nconsole.log('test');\n```"),
        "console.log('test');"
    );
    assert_eq!(
        strip_code_fence("plain code without fences"),
        "plain code without fences"
    );
}

#[tokio::test]
async fn test_run_compile_check_status() {
    let (success, _) = run_compile_check("true").await.unwrap();
    assert!(success);

    let (failure, _) = run_compile_check("false").await.unwrap();
    assert!(!failure);
}

#[test]
fn test_file_backup_guard_rollback_on_drop() {
    let temp_dir = std::env::temp_dir().join(format!("heal_guard_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let target_file = temp_dir.join("test.txt");
    std::fs::write(&target_file, "original content").unwrap();

    {
        let mut _guard = FileBackupGuard::create(&target_file, false).unwrap();
        std::fs::write(&target_file, "modified content").unwrap();
        assert_eq!(std::fs::read_to_string(&target_file).unwrap(), "modified content");
        // guard dropped here without defuse
    }

    assert_eq!(std::fs::read_to_string(&target_file).unwrap(), "original content");
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_file_backup_guard_defuse_preserves_modification() {
    let temp_dir = std::env::temp_dir().join(format!("heal_guard_defuse_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let target_file = temp_dir.join("test.txt");
    std::fs::write(&target_file, "original content").unwrap();

    {
        let mut guard = FileBackupGuard::create(&target_file, false).unwrap();
        std::fs::write(&target_file, "modified content").unwrap();
        guard.defuse();
    }

    assert_eq!(std::fs::read_to_string(&target_file).unwrap(), "modified content");
    let _ = std::fs::remove_dir_all(&temp_dir);
}
