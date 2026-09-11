use super::*;

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_test_env() -> std::sync::MutexGuard<'static, ()> {
    TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner())
}

fn reset_activity_write_state_for_test() {
    let mut state = activity_write_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.last_written_at = None;
    state.pending = None;
    state.flush_scheduled = false;
}

fn temp_activity_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "openz_activity_{}_{}.json",
        name,
        uuid::Uuid::new_v4()
    ))
}

fn activity(session_id: &str, status: &str, current_tool: Option<&str>) -> AgentActivity {
    AgentActivity {
        session_id: session_id.to_string(),
        status: status.to_string(),
        current_tool: current_tool.map(|tool| tool.to_string()),
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

fn read_activity_at(path: &Path) -> AgentActivity {
    let content = fs::read_to_string(path).expect("activity file should exist");
    serde_json::from_str(&content).expect("activity file should deserialize")
}

#[test]
fn activity_updates_are_throttled_and_coalesced() {
    let _guard = lock_test_env();
    reset_activity_write_state_for_test();
    let path = temp_activity_path("coalesce");

    update_activity_at_path(path.clone(), activity("s1", "Processing user prompt", None));
    assert_eq!(read_activity_at(&path).status, "Processing user prompt");

    update_activity_at_path(
        path.clone(),
        activity("s1", "Executing tool", Some("grep_search")),
    );
    let immediate = read_activity_at(&path);
    assert_eq!(immediate.status, "Processing user prompt");
    assert_eq!(immediate.current_tool, None);

    let start = std::time::Instant::now();
    let mut flushed = read_activity_at(&path);
    while flushed.status != "Executing tool" && start.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(50));
        flushed = read_activity_at(&path);
    }
    assert_eq!(flushed.status, "Executing tool");
    assert_eq!(flushed.current_tool.as_deref(), Some("grep_search"));

    let _ = fs::remove_file(path);
}

#[test]
fn idle_activity_forces_immediate_write() {
    let _guard = lock_test_env();
    reset_activity_write_state_for_test();
    let path = temp_activity_path("idle");

    update_activity_at_path(path.clone(), activity("s1", "Processing user prompt", None));
    update_activity_at_path(path.clone(), activity("s1", "Idle", None));

    assert_eq!(read_activity_at(&path).status, "Idle");
    let _ = fs::remove_file(path);
}

#[test]
fn session_preview_uses_latest_user_message_and_truncates() {
    let mut messages = Vec::new();
    messages.push(crate::session::Message {
        role: "user".to_string(),
        content: "first prompt".to_string(),
        timestamp: None,
        extra: serde_json::Map::new(),
    });
    messages.push(crate::session::Message {
        role: "assistant".to_string(),
        content: "answer".to_string(),
        timestamp: None,
        extra: serde_json::Map::new(),
    });
    messages.push(crate::session::Message {
        role: "user".to_string(),
        content: "this is the latest prompt with many words that should be used for preview because it is the newest".to_string(),
        timestamp: None,
        extra: serde_json::Map::new(),
    });

    let preview = session_preview_from_messages(&messages);
    assert!(preview.starts_with("this is the latest prompt"));
    assert!(preview.len() <= 67);
}

#[test]
fn direct_target_requires_exactly_one_session() {
    assert!(resolve_direct_target_from_keys(&[]).is_err());
    assert_eq!(
        resolve_direct_target_from_keys(&["cli:one".to_string()]).unwrap(),
        "cli:one"
    );
    assert!(
        resolve_direct_target_from_keys(&["cli:one".to_string(), "cli:two".to_string()])
            .is_err()
    );
}

#[test]
fn inbox_expiry_rejects_old_messages() {
    let old = InboxMessage {
        message: "old".to_string(),
        sender: "test".to_string(),
        timestamp: (chrono::Utc::now() - chrono::Duration::minutes(6)).to_rfc3339(),
    };
    assert!(inbox_message_is_expired(&old));
}

#[tokio::test]
async fn enqueue_cleans_expired_entries_for_other_targets() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_inbox_cleanup_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            let stale = InboxMessage {
                message: "stale".to_string(),
                sender: "test".to_string(),
                timestamp: (chrono::Utc::now() - chrono::Duration::minutes(6)).to_rfc3339(),
            };
            std::fs::write(
                temp_dir.join("inbox_telegram_direct_stale.json"),
                serde_json::to_string(&stale).unwrap(),
            )
            .unwrap();

            send_inbox_message("cli:test", "fresh", "test").unwrap();

            assert!(!temp_dir.join("inbox_telegram_direct_stale.json").exists());
            assert_eq!(pop_inbox_message("cli:test").unwrap().message, "fresh");
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn inbox_queue_preserves_fifo_order() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_inbox_fifo_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            send_inbox_message("cli:test", "first", "test").unwrap();
            tokio::time::sleep(Duration::from_millis(2)).await;
            send_inbox_message("cli:test", "second", "test").unwrap();

            assert_eq!(pop_inbox_message("cli:test").unwrap().message, "first");
            assert_eq!(pop_inbox_message("cli:test").unwrap().message, "second");
            assert!(pop_inbox_message("cli:test").is_none());
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn inbox_queue_quarantines_malformed_entries() {
    let temp_dir =
        std::env::temp_dir().join(format!("openz_inbox_invalid_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    crate::config::loader::CONFIG_DIR_OVERRIDE
        .scope(temp_dir.clone(), async {
            std::fs::write(
                temp_dir.join("inbox_cli_test_invalid.json"),
                "{not valid json",
            )
            .unwrap();
            send_inbox_message("cli:test", "valid", "test").unwrap();

            assert_eq!(pop_inbox_message("cli:test").unwrap().message, "valid");
            let quarantined = std::fs::read_dir(&temp_dir)
                .unwrap()
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().contains(".invalid."));
            assert!(quarantined);
        })
        .await;

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn active_tui_stale_when_pid_is_dead_or_timestamp_invalid() {
    let now = chrono::Utc::now();
    let mut session = ActiveTuiSession {
        session_key: "cli:test".to_string(),
        pid: 0,
        cwd: "/tmp".to_string(),
        started_at: now.to_rfc3339(),
        last_seen_at: now.to_rfc3339(),
        model: "model".to_string(),
        provider: "provider".to_string(),
        preview: "preview".to_string(),
    };
    assert!(active_tui_is_stale(&session, now));

    session.pid = std::process::id();
    session.last_seen_at = "not-a-date".to_string();
    assert!(active_tui_is_stale(&session, now));
}
