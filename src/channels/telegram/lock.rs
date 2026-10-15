use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

pub(crate) fn channel_singleton_lock_path_in(
    data_dir: &Path,
    channel_prefix: &str,
    identity: &str,
) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(identity.as_bytes());
    let fingerprint = hex::encode(&hasher.finalize()[..8]);
    data_dir
        .join("locks")
        .join(format!("{channel_prefix}-{fingerprint}.lock"))
}

pub(crate) fn acquire_channel_singleton_lock_at(
    data_dir: &Path,
    channel_prefix: &str,
    identity: &str,
) -> anyhow::Result<Option<File>> {
    let lock_path = channel_singleton_lock_path_in(data_dir, channel_prefix, identity);
    match lock_path.parent() {
        Some(parent) => std::fs::create_dir_all(parent)?,
        None => {}
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(err) => match err.kind() == std::io::ErrorKind::WouldBlock {
            true => Ok(None),
            false => Err(err.into()),
        },
    }
}

pub(crate) fn acquire_channel_singleton_lock(
    channel_prefix: &str,
    identity: &str,
) -> anyhow::Result<Option<File>> {
    acquire_channel_singleton_lock_at(
        &crate::config::loader::runtime_data_dir(),
        channel_prefix,
        identity,
    )
}

pub(crate) fn telegram_lock_path_in(data_dir: &Path, bot_token: &str) -> PathBuf {
    channel_singleton_lock_path_in(data_dir, "telegram", bot_token)
}

pub(crate) fn acquire_telegram_poll_lock_at(
    data_dir: &Path,
    bot_token: &str,
) -> anyhow::Result<Option<File>> {
    acquire_channel_singleton_lock_at(data_dir, "telegram", bot_token)
}

pub(crate) fn acquire_telegram_poll_lock(bot_token: &str) -> anyhow::Result<Option<File>> {
    acquire_channel_singleton_lock("telegram", bot_token)
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;

