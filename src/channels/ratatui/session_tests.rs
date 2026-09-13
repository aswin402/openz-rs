use super::*;

#[test]
fn test_process_is_alive_self() {
    assert!(process_is_alive(std::process::id()));
}

#[test]
fn test_process_is_alive_invalid_pid() {
    assert!(!process_is_alive(u32::MAX));
}

#[test]
fn test_tui_marker_lifecycle() {
    let temp_dir = std::env::temp_dir().join(format!("test_tui_marker_{}", uuid::Uuid::new_v4()));
    let pid = 999999;
    let session_key = "test:session";
    let model = "test-model";
    let provider = "test-provider";

    assert!(write_tui_marker_in_dir(&temp_dir, pid, session_key, model, provider).is_ok());

    let marker_path = tui_marker_path_in_dir(&temp_dir, pid);
    assert!(marker_path.exists());

    let content = std::fs::read_to_string(&marker_path).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(val["pid"], pid);
    assert_eq!(val["session_key"], session_key);
    assert_eq!(val["model"], model);
    assert_eq!(val["provider"], provider);

    remove_tui_marker_in_dir(&temp_dir, pid);
    assert!(!marker_path.exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_is_last_live_tui_in_dir() {
    let temp_dir = std::env::temp_dir().join(format!("test_last_live_{}", uuid::Uuid::new_v4()));
    let current_pid = std::process::id();

    // Empty directory
    assert!(is_last_live_tui_in_dir(&temp_dir, current_pid));

    // Write marker for current pid
    let _ = write_tui_marker_in_dir(&temp_dir, current_pid, "s1", "m1", "p1");
    assert!(is_last_live_tui_in_dir(&temp_dir, current_pid));

    // Write marker for dead pid (e.g. u32::MAX)
    let dead_pid = u32::MAX;
    let _ = write_tui_marker_in_dir(&temp_dir, dead_pid, "s2", "m2", "p2");
    // is_last_live_tui_in_dir cleans up dead marker and returns true
    assert!(is_last_live_tui_in_dir(&temp_dir, current_pid));
    assert!(!tui_marker_path_in_dir(&temp_dir, dead_pid).exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
