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
        .truncate(false)
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
#[path = "lock_tests.rs"]
mod tests;

