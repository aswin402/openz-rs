use super::*;

#[test]
fn remote_session_selection_round_trips() {
    let chat_id = 4242;
    clear_remote_session(chat_id);
    assert_eq!(selected_remote_session(chat_id), None);

    set_remote_session(chat_id, "cli:test-session".to_string());
    assert_eq!(
        selected_remote_session(chat_id).as_deref(),
        Some("cli:test-session")
    );

    clear_remote_session(chat_id);
    assert_eq!(selected_remote_session(chat_id), None);
}

#[test]
fn remote_session_button_label_contains_context() {
    let session = crate::agent::activity::ActiveTuiSession {
        session_key: "cli:test".to_string(),
        pid: std::process::id(),
        cwd: "/tmp/openz-client-work".to_string(),
        started_at: "2026-07-16T08:30:00Z".to_string(),
        last_seen_at: "2026-07-16T08:31:00Z".to_string(),
        model: "model".to_string(),
        provider: "provider".to_string(),
        preview: "plan client workflow".to_string(),
    };

    let label = remote_session_button_label(&session);
    assert!(label.contains("openz-client-work"));
    assert!(label.contains("plan client workflow"));
}
