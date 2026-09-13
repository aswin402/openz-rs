use super::*;

#[test]
fn scrub_secret_text_redacts_full_and_partial_key_patterns() {
    let input = "curl https://api.telegram.org/bot1234567890:AAEabcDEFghiJKLmnopQRSTuvwxYZ012345/getWebhookInfo\nprovider sk-AbCdEfGhIjKlMnOpQrStUvWxYz1234567890 and sk-AbCdEf...";

    let result = scrub_secret_text(input);

    assert_eq!(result.replacements, 3);
    assert!(result.text.contains("bot[REDACTED_SECRET]"));
    assert!(result.text.contains("provider [REDACTED_SECRET]"));
    assert!(!result.text.contains("1234567890:AAEabc"));
    assert!(!result.text.contains("sk-AbCdEf"));
}

#[test]
fn scrub_secret_file_preserves_json_validity() -> Result<()> {
    let dir =
        std::env::temp_dir().join(format!("openz_doctor_scrub_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("session.json");
    std::fs::write(
        &path,
        r#"{"cmd":"curl https://api.telegram.org/bot1234567890:AAEabcDEFghiJKLmnopQRSTuvwxYZ012345/getMe","key":"sk-AbCdEfGhIjKlMnOpQrStUvWxYz1234567890"}"#,
    )?;

    let changed = scrub_secret_file(&path)?;

    assert_eq!(changed.replacements, 2);
    let updated = std::fs::read_to_string(&path)?;
    serde_json::from_str::<serde_json::Value>(&updated)?;
    assert!(!updated.contains("1234567890:AAEabc"));
    assert!(!updated.contains("sk-AbCdEf"));
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn format_bytes_uses_binary_units() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1024), "1.0 KiB");
    assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
    assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.0 GiB");
}

#[test]
fn disk_item_status_respects_thresholds() {
    let base = DiskItem::new("test", Path::new("target").to_path_buf(), 20, 50, None);
    let missing = base.clone();
    assert_eq!(missing.status(), DiskStatus::Missing);

    let mut ok = base.clone();
    ok.size_bytes = Some(gib(1));
    assert_eq!(ok.status(), DiskStatus::Ok);

    let mut warn = base.clone();
    warn.size_bytes = Some(gib(20));
    assert_eq!(warn.status(), DiskStatus::Warn);

    let mut critical = base;
    critical.size_bytes = Some(gib(50));
    assert_eq!(critical.status(), DiskStatus::Critical);
}

#[test]
fn test_clean_target_directory() -> Result<()> {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_clean_test_{}", uuid::Uuid::new_v4()));
    let target = temp_dir.join("target");
    std::fs::create_dir_all(&target)?;
    std::fs::write(target.join("dummy.bin"), vec![0u8; 1024 * 1024])?;

    assert!(target.exists());
    let reclaimed = clean_target_cache_at(&target)?;
    assert!(reclaimed >= 1024 * 1024);
    assert!(!target.exists());

    let reclaimed_again = clean_target_cache_at(&target)?;
    assert_eq!(reclaimed_again, 0);

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
