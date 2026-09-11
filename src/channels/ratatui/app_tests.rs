use super::*;

fn app_with_scroll(max_scroll: u32) -> RatatuiApp {
    let mut app = RatatuiApp::new(
        "test-model".into(),
        "test-provider".into(),
        "cli:test".into(),
    );
    app.max_scroll = max_scroll;
    app
}

#[test]
fn branch_cache_is_keyed_by_workspace() {
    let first = std::env::temp_dir().join(format!("openz_branch_a_{}", uuid::Uuid::new_v4()));
    let second = std::env::temp_dir().join(format!("openz_branch_b_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();

    let first_key = first.canonicalize().unwrap();
    let second_key = second.canonicalize().unwrap();

    {
        let mut guard = super::BRANCH_CACHE.lock().unwrap();
        guard.clear();
        guard.insert(
            first_key.clone(),
            (Instant::now(), Some("main".to_string())),
        );
        guard.insert(
            second_key.clone(),
            (Instant::now(), Some("feature".to_string())),
        );
    }

    assert_eq!(RatatuiApp::get_git_branch(&first).as_deref(), Some("main"));
    assert_eq!(
        RatatuiApp::get_git_branch(&second).as_deref(),
        Some("feature")
    );

    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);
}

#[test]
fn scroll_up_from_auto_scroll_lands_near_bottom() {
    let mut app = app_with_scroll(100);
    assert!(app.auto_scroll);
    app.scroll_up(3);
    assert!(!app.auto_scroll);
    assert_eq!(app.scroll_offset, 97);
}

#[test]
fn scroll_up_clamps_at_zero() {
    let mut app = app_with_scroll(10);
    app.scroll_up(2); // leave auto mode
    app.scroll_up(50); // way past the top
    assert_eq!(app.scroll_offset, 0);
    assert!(!app.auto_scroll);
}

#[test]
fn scroll_down_reengages_auto_scroll_at_bottom() {
    let mut app = app_with_scroll(100);
    app.scroll_up(10);
    assert_eq!(app.scroll_offset, 90);
    app.scroll_down(5);
    assert_eq!(app.scroll_offset, 95);
    assert!(!app.auto_scroll);
    app.scroll_down(5); // reaches the bottom edge
    assert_eq!(app.scroll_offset, 100);
    assert!(app.auto_scroll);
}

#[test]
fn scroll_down_is_noop_when_auto_scrolling() {
    let mut app = app_with_scroll(100);
    app.scroll_to_bottom(); // anchor the field at max, auto mode on
    app.scroll_down(40); // auto mode absorbs it — no manual scroll starts
    assert_eq!(app.scroll_offset, 100);
    assert!(app.auto_scroll);
}

#[test]
fn scroll_to_top_and_bottom_edges() {
    let mut app = app_with_scroll(100);
    app.scroll_to_top();
    assert_eq!(app.scroll_offset, 0);
    assert!(!app.auto_scroll);
    app.scroll_to_bottom();
    assert_eq!(app.scroll_offset, 100);
    assert!(app.auto_scroll);
}

#[test]
fn apply_sync_session_preserves_recent_notices_only() {
    let mut app = app_with_scroll(50);
    for i in 0..7 {
        app.messages
            .push(ChatMessage::notice(format!("note {}", i)));
    }
    app.messages
        .push(ChatMessage::simple("user", "hello".into()));
    app.messages
        .push(ChatMessage::simple("assistant", "hi".into()));
    app.scroll_to_top();

    let disk = vec![
        ChatMessage::simple("user", "from disk".into()),
        ChatMessage::simple("assistant", "disk reply".into()),
    ];
    app.apply_sync_session(disk);

    assert_eq!(app.messages.len(), 7); // 2 disk + 5 kept notices (capped)
    assert_eq!(app.messages[0].content, "from disk");
    assert!(!app.messages[0].ephemeral);
    // Oldest two notices dropped, newest five kept in order
    let notices: Vec<&str> = app.messages[2..]
        .iter()
        .map(|m| m.content.as_str())
        .collect();
    assert_eq!(
        notices,
        vec!["note 2", "note 3", "note 4", "note 5", "note 6"]
    );
    assert!(app.auto_scroll); // re-anchored to bottom
}

#[test]
fn from_session_message_extracts_extras() {
    let mut extra = serde_json::Map::new();
    extra.insert("tool_name".into(), serde_json::json!("read_file"));
    extra.insert("tool_details".into(), serde_json::json!("path=src/main.rs"));
    extra.insert(
        "reasoning_content".into(),
        serde_json::json!("let me think"),
    );
    extra.insert("thinking_time_secs".into(), serde_json::json!(1.25));
    let msg = crate::session::Message {
        role: "tool".into(),
        content: "file contents".into(),
        timestamp: None,
        extra,
    };

    let chat = ChatMessage::from_session_message(&msg);
    assert!(chat.is_tool);
    assert_eq!(chat.tool_name.as_deref(), Some("read_file"));
    assert_eq!(chat.tool_details.as_deref(), Some("path=src/main.rs"));
    assert_eq!(chat.reasoning.as_deref(), Some("let me think"));
    assert_eq!(chat.thinking_time, Some(1.25));
    assert!(!chat.ephemeral);
}

#[test]
fn notice_is_ephemeral_and_defaults_are_not() {
    let notice = ChatMessage::notice("keep me".into());
    assert!(notice.ephemeral);
    assert!(!ChatMessage::simple("user", "x".into()).ephemeral);
    assert!(!ChatMessage::tool_start("t".into(), "d".into()).ephemeral);
}
