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
