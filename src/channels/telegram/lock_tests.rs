use super::*;

#[test]
fn telegram_lock_path_does_not_leak_bot_token() {
    let token = "123456:secret-token-value";
    let dir =
        std::env::temp_dir().join(format!("openz_telegram_path_test_{}", uuid::Uuid::new_v4()));
    let path = telegram_lock_path_in(&dir, token);
    let path_str = path.to_string_lossy();

    assert!(path_str.contains("telegram-"));
    assert!(!path_str.contains(token));
    assert!(!path_str.contains("secret-token-value"));
}

#[test]
fn telegram_poll_lock_rejects_duplicate_holder() {
    let dir =
        std::env::temp_dir().join(format!("openz_telegram_lock_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();

    let token = "123456:test-lock-token";
    let first = acquire_telegram_poll_lock_at(&dir, token)
        .expect("first lock attempt should not error")
        .expect("first lock should be acquired");
    let second = acquire_telegram_poll_lock_at(&dir, token)
        .expect("second lock attempt should not error");
    assert!(second.is_none(), "duplicate poll lock should be rejected");

    drop(first);
    let third = acquire_telegram_poll_lock_at(&dir, token)
        .expect("third lock attempt should not error");
    assert!(
        third.is_some(),
        "lock should be reusable after first holder drops"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
