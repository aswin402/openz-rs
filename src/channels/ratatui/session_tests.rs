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

#[test]
fn test_allocate_tui_session_key_multi_tab_isolation() {
    let temp_dir = std::env::temp_dir().join(format!("test_multi_tab_{}", uuid::Uuid::new_v4()));
    let live_pid_1 = std::process::id();
    let live_pid_2 = live_pid_1.saturating_add(1_000_000);
    let base_key = "cli:workspace123";

    // Tab 1 claims base_key
    let allocated_1 = allocate_tui_session_key_in_dir(&temp_dir, live_pid_1, base_key);
    assert_eq!(allocated_1, base_key);
    assert!(write_tui_marker_in_dir(&temp_dir, live_pid_1, &allocated_1, "gpt-4o", "openai").is_ok());

    // Tab 2 in the same directory detects live_pid_1 holding base_key and gets a tab-isolated key
    assert!(is_session_claimed_by_other_live_tui_in_dir(
        &temp_dir,
        live_pid_2,
        base_key
    ));
    let allocated_2 = allocate_tui_session_key_in_dir(&temp_dir, live_pid_2, base_key);
    assert_eq!(allocated_2, format!("{base_key}_tab_{live_pid_2}"));

    // Tab 1 re-checking its own session key sees it is not claimed by *another* live TUI
    assert!(!is_session_claimed_by_other_live_tui_in_dir(
        &temp_dir,
        live_pid_1,
        base_key
    ));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_ratatui_session_active_scoping() {
    crate::channels::ratatui::IS_RATATUI_ACTIVE
        .store(true, std::sync::atomic::Ordering::SeqCst);
    crate::channels::ratatui::set_active_ratatui_session_key(Some(
        "cli:tab_1".to_string(),
    ));
    assert!(crate::channels::ratatui::is_ratatui_session_active(None));
    assert!(crate::channels::ratatui::is_ratatui_session_active(Some(
        "cli:tab_1"
    )));
    assert!(!crate::channels::ratatui::is_ratatui_session_active(Some(
        "cli:tab_2"
    )));
    assert!(!crate::channels::ratatui::is_ratatui_session_active(Some(
        "ws:webui_session"
    )));
    assert!(!crate::channels::ratatui::is_ratatui_session_active(Some(
        "telegram:12345"
    )));

    crate::channels::ratatui::set_active_ratatui_session_key(None);
    crate::channels::ratatui::IS_RATATUI_ACTIVE
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(!crate::channels::ratatui::is_ratatui_session_active(None));
    assert!(!crate::channels::ratatui::is_ratatui_session_active(Some(
        "cli:tab_1"
    )));
}

#[tokio::test]
async fn test_per_session_overrides_isolation() {
    let temp_dir = std::env::temp_dir().join(format!("test_session_overrides_{}", uuid::Uuid::new_v4()));
    let manager = crate::session::SessionManager::new(temp_dir.clone());

    let mut tab1_overrides = serde_json::Map::new();
    tab1_overrides.insert("model".to_string(), serde_json::json!("claude-3-7-sonnet"));
    tab1_overrides.insert("provider".to_string(), serde_json::json!("anthropic"));
    tab1_overrides.insert("temperature".to_string(), serde_json::json!(0.2));
    tab1_overrides.insert("streaming".to_string(), serde_json::json!(true));
    crate::agent::agent_loop::save_session_overrides(&manager, "cli:tab_1", &tab1_overrides)
        .await
        .unwrap();

    let mut webui_overrides = serde_json::Map::new();
    webui_overrides.insert("model".to_string(), serde_json::json!("deepseek-reasoner"));
    webui_overrides.insert("provider".to_string(), serde_json::json!("deepseek"));
    webui_overrides.insert("temperature".to_string(), serde_json::json!(0.7));
    webui_overrides.insert("streaming".to_string(), serde_json::json!(false));
    crate::agent::agent_loop::save_session_overrides(&manager, "ws:webui_1", &webui_overrides)
        .await
        .unwrap();

    let s1 = manager.get_or_create_async("cli:tab_1").await;
    let s2 = manager.get_or_create_async("ws:webui_1").await;

    assert_eq!(s1.metadata.get("model").and_then(|v| v.as_str()), Some("claude-3-7-sonnet"));
    assert_eq!(s1.metadata.get("provider").and_then(|v| v.as_str()), Some("anthropic"));
    assert_eq!(s1.metadata.get("temperature").and_then(|v| v.as_f64()), Some(0.2));
    assert_eq!(s1.metadata.get("streaming").and_then(|v| v.as_bool()), Some(true));

    assert_eq!(s2.metadata.get("model").and_then(|v| v.as_str()), Some("deepseek-reasoner"));
    assert_eq!(s2.metadata.get("provider").and_then(|v| v.as_str()), Some("deepseek"));
    assert_eq!(s2.metadata.get("temperature").and_then(|v| v.as_f64()), Some(0.7));
    assert_eq!(s2.metadata.get("streaming").and_then(|v| v.as_bool()), Some(false));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

