use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

pub(crate) fn telegram_lock_path_in(data_dir: &Path, bot_token: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(bot_token.as_bytes());
    let fingerprint = hex::encode(&hasher.finalize()[..8]);
    data_dir
        .join("locks")
        .join(format!("telegram-{fingerprint}.lock"))
}

pub(crate) fn acquire_telegram_poll_lock_at(
    data_dir: &Path,
    bot_token: &str,
) -> anyhow::Result<Option<File>> {
    let lock_path = telegram_lock_path_in(data_dir, bot_token);
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub(crate) fn acquire_telegram_poll_lock(bot_token: &str) -> anyhow::Result<Option<File>> {
    acquire_telegram_poll_lock_at(&crate::config::loader::runtime_data_dir(), bot_token)
}

#[cfg(test)]
mod tests {
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
}
