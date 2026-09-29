use super::*;

#[test]
fn test_centered_rect_bounds() {
    let parent = Rect::new(0, 0, 100, 100);
    let centered = centered_rect(50, 50, parent);
    assert_eq!(centered.width, 50);
    assert_eq!(centered.height, 50);
    assert_eq!(centered.x, 25);
    assert_eq!(centered.y, 25);
}

#[test]
fn test_security_approval_modal_state() {
    let (tx, _rx) = tokio::sync::oneshot::channel();
    let modal = ModalState::SecurityApproval {
        tool_name: "exec_command".to_string(),
        description: "run bash script".to_string(),
        session_key: "cli:session_123".to_string(),
        options: vec![
            "Approve (Allow once)".to_string(),
            "Approve & Trust for this session".to_string(),
            "Deny (Abort tool)".to_string(),
        ],
        selected_idx: 1,
        tx: std::sync::Arc::new(tokio::sync::Mutex::new(Some(tx))),
    };
    assert!(modal.is_active());
    let debug_repr = format!("{:?}", modal);
    assert!(debug_repr.contains("exec_command"));
    assert!(debug_repr.contains('1'));
}
