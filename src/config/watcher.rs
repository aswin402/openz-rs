//! Background file watcher for configuration updates.

use anyhow::Result;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::config::schema::Config;

/// Handle to the running configuration watcher.
/// Dropping this guard signals the background watcher task to terminate.
pub struct ConfigWatcherGuard {
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl ConfigWatcherGuard {
    /// Explicitly stop the watcher.
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

impl Drop for ConfigWatcherGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
    }
}

/// Spawn a background file watcher targeting `~/.openz/config.json` (or active config path).
pub fn spawn_default_config_watcher<F>(
    live_config: Arc<RwLock<Config>>,
    on_change: F,
) -> Result<ConfigWatcherGuard>
where
    F: Fn(&Config) + Send + Sync + 'static,
{
    let target = crate::config::loader::config_path();
    spawn_config_watcher(target, live_config, on_change)
}

/// Spawn a background file watcher targeting the given configuration path.
pub fn spawn_config_watcher<F>(
    target_path: PathBuf,
    live_config: Arc<RwLock<Config>>,
    on_change: F,
) -> Result<ConfigWatcherGuard>
where
    F: Fn(&Config) + Send + Sync + 'static,
{
    let watch_dir = if target_path.is_dir() {
        target_path.clone()
    } else {
        target_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    };

    if !watch_dir.exists() {
        let _ = std::fs::create_dir_all(&watch_dir);
    }

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    let (event_tx, mut event_rx) = mpsc::channel::<()>(100);

    let target_name = target_path.file_name().map(|n| n.to_os_string());
    let event_tx_clone = event_tx.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let is_relevant = if let Some(ref target) = target_name {
                    event.paths.iter().any(|p| {
                        p.file_name().is_some_and(|name| name == target)
                    })
                } else {
                    true
                };
                if is_relevant && (event.kind.is_modify() || event.kind.is_create() || event.kind.is_other()) {
                    let _ = event_tx_clone.try_send(());
                }
            }
        },
        NotifyConfig::default(),
    )?;

    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;

    let target_path_clone = target_path.clone();
    tokio::spawn(async move {
        let _watcher = watcher;
        let debounce = Duration::from_millis(150);
        let mut last_processed = Instant::now() - Duration::from_secs(10);

        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.recv() => {
                    debug!("Config watcher received shutdown signal, terminating.");
                    break;
                }
                Some(_) = event_rx.recv() => {
                    let now = Instant::now();
                    if now.duration_since(last_processed) < debounce {
                        continue;
                    }
                    last_processed = now;

                    // Allow a brief window for atomic rename/file write flush to complete
                    tokio::time::sleep(Duration::from_millis(60)).await;

                    // Drain any additional events buffered during sleep
                    while event_rx.try_recv().is_ok() {}

                    let reloaded = match crate::config::loader::load_config_from_path(&target_path_clone) {
                        Ok(cfg) => cfg,
                        Err(err) => {
                            warn!(
                                "Failed to reload config from {}: {:#}",
                                target_path_clone.display(),
                                err
                            );
                            continue;
                        }
                    };

                    let changed = match live_config.write() {
                        Ok(mut guard) => {
                            let current_json = serde_json::to_string(&*guard).unwrap_or_default();
                            let new_json = serde_json::to_string(&reloaded).unwrap_or_default();
                            if current_json != new_json {
                                *guard = reloaded.clone();
                                true
                            } else {
                                false
                            }
                        }
                        Err(poisoned) => {
                            let mut guard = poisoned.into_inner();
                            *guard = reloaded.clone();
                            true
                        }
                    };

                    if changed {
                        info!("Live configuration reloaded from {}", target_path_clone.display());
                        on_change(&reloaded);
                    }
                }
            }
        }
    });

    Ok(ConfigWatcherGuard {
        shutdown_tx: Some(shutdown_tx),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDir(PathBuf);
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn test_config_watcher_reloads_on_file_change() {
        let temp_dir = std::env::temp_dir().join(format!(
            "openz_config_watcher_test_{}",
            uuid::Uuid::new_v4()
        ));
        let _guard = TestDir(temp_dir.clone());
        std::fs::create_dir_all(&temp_dir).unwrap();

        let config_file = temp_dir.join("config.json");
        let initial_config = Config::default();
        crate::config::loader::save_config_to_path(&config_file, &initial_config).unwrap();

        let live_config = Arc::new(RwLock::new(initial_config.clone()));
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let watcher = spawn_config_watcher(config_file.clone(), live_config.clone(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();

        // Update the file on disk
        let mut updated = initial_config.clone();
        updated.agents.defaults.model = "test-reloaded-model".to_string();
        crate::config::loader::save_config_to_path(&config_file, &updated).unwrap();

        // Wait up to 2 seconds for the watcher to detect and apply the change
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if call_count.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            live_config.read().unwrap().agents.defaults.model,
            "test-reloaded-model"
        );

        drop(watcher);
    }

    #[tokio::test]
    async fn test_config_watcher_ignores_redundant_write() {
        let temp_dir = std::env::temp_dir().join(format!(
            "openz_config_watcher_redundant_{}",
            uuid::Uuid::new_v4()
        ));
        let _guard = TestDir(temp_dir.clone());
        std::fs::create_dir_all(&temp_dir).unwrap();

        let config_file = temp_dir.join("config.json");
        let initial_config = Config::default();
        crate::config::loader::save_config_to_path(&config_file, &initial_config).unwrap();

        let live_config = Arc::new(RwLock::new(initial_config.clone()));
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let watcher = spawn_config_watcher(config_file.clone(), live_config.clone(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();

        // Write the EXACT same config again
        crate::config::loader::save_config_to_path(&config_file, &initial_config).unwrap();

        // Wait a bit to ensure debouncing and parsing completed
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Count should remain 0 because content was identical
        assert_eq!(call_count.load(Ordering::SeqCst), 0);

        drop(watcher);
    }

    #[tokio::test]
    async fn test_config_watcher_handles_corrupt_json_safely() {
        let temp_dir = std::env::temp_dir().join(format!(
            "openz_config_watcher_corrupt_{}",
            uuid::Uuid::new_v4()
        ));
        let _guard = TestDir(temp_dir.clone());
        std::fs::create_dir_all(&temp_dir).unwrap();

        let config_file = temp_dir.join("config.json");
        let initial_config = Config::default();
        crate::config::loader::save_config_to_path(&config_file, &initial_config).unwrap();

        let live_config = Arc::new(RwLock::new(initial_config.clone()));
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let watcher = spawn_config_watcher(config_file.clone(), live_config.clone(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();

        // Write invalid JSON
        std::fs::write(&config_file, "{\"invalid_json\": [").unwrap();

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Should not panic, should not trigger on_change, and live_config remains intact
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
        assert_eq!(
            live_config.read().unwrap().agents.defaults.model,
            initial_config.agents.defaults.model
        );

        drop(watcher);
    }
}
