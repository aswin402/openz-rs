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

#[test]
fn test_centered_rect_exact_bounds() {
    let parent = Rect::new(0, 0, 120, 80);
    let centered = centered_rect_exact(50, 20, parent);
    assert_eq!(centered.width, 50);
    assert_eq!(centered.height, 20);
    assert_eq!(centered.x, 35);
    assert_eq!(centered.y, 30);

    // If requested dimensions exceed parent, clamp to parent (with 1-cell border margin on each side)
    let clamped = centered_rect_exact(200, 150, parent);
    assert_eq!(clamped.width, 118);
    assert_eq!(clamped.height, 78);
    assert_eq!(clamped.x, 1);
    assert_eq!(clamped.y, 1);
}

#[test]
fn test_compute_scroll_offset_behavior() {
    // When visible height is 5, item 2 should be at offset 0
    assert_eq!(compute_scroll_offset(2, 5), 0);
    // When selected index is 6 with height 5, offset should scroll to keep it in view
    assert_eq!(compute_scroll_offset(6, 5), 2);
    // When total items is small, offset is 0
    assert_eq!(compute_scroll_offset(1, 10), 0);
}

#[test]
fn test_exit_confirm_modal_state() {
    let modal_no = ModalState::ExitConfirm { selected_yes: false };
    assert!(modal_no.is_active());
    let repr_no = format!("{:?}", modal_no);
    assert!(repr_no.contains("ExitConfirm"));
    assert!(repr_no.contains("false"));

    let modal_yes = ModalState::ExitConfirm { selected_yes: true };
    assert!(modal_yes.is_active());
}

#[test]
fn test_command_catalog_modal_lifecycle() {
    use crate::channels::ratatui::app;

    let mut catalog = ModalState::new_command_catalog();
    assert!(catalog.is_active());

    if let ModalState::CommandCatalog {
        ref filtered_indices,
        selected_index,
        ref filter,
    } = catalog
    {
        assert_eq!(selected_index, 0);
        assert!(filter.is_empty());
        assert!(!filtered_indices.is_empty());
        assert_eq!(filtered_indices.len(), app::PALETTE_COMMANDS.len());
    } else {
        panic!("Expected ModalState::CommandCatalog");
    }

    // Filter by query "model"
    if let ModalState::CommandCatalog { ref mut filter, .. } = catalog {
        *filter = "model".to_string();
    }
    catalog.update_command_catalog_filter();
    if let ModalState::CommandCatalog {
        ref filtered_indices,
        selected_index,
        ref filter,
    } = catalog
    {
        assert_eq!(selected_index, 0);
        assert_eq!(filter, "model");
        assert!(!filtered_indices.is_empty());
        // Verify filtered results contain "model"
        for &idx in filtered_indices {
            let cmd = &app::PALETTE_COMMANDS[idx];
            let matches = cmd.slash_name.to_lowercase().contains("model")
                || cmd.title.to_lowercase().contains("model")
                || cmd.description.to_lowercase().contains("model");
            assert!(matches);
        }
    } else {
        panic!("Expected ModalState::CommandCatalog");
    }
}

#[test]
fn test_format_security_reason_lines_single_line() {
    let theme = crate::channels::ratatui::theme::Theme::aura_dark();
    let lines = format_security_reason_lines("Run cargo check on openz", 50, &theme);
    assert_eq!(lines.len(), 1);
    let line_str = lines[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
    assert!(line_str.contains("Reason:"));
    assert!(line_str.contains("Run cargo check on openz"));
}

#[test]
fn test_format_security_reason_lines_two_lines_full() {
    let theme = crate::channels::ratatui::theme::Theme::aura_dark();
    let desc = "Executes command with sandboxed environment";
    let lines = format_security_reason_lines(desc, 25, &theme);
    assert_eq!(lines.len(), 2);
    let line1_str = lines[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
    let line2_str = lines[1].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
    assert!(line1_str.contains("Reason:"));
    assert!(!line2_str.contains("...."));
}

#[test]
fn test_format_security_reason_lines_three_plus_lines_truncated_with_dots() {
    let theme = crate::channels::ratatui::theme::Theme::aura_dark();
    let desc = "High risk security tool execution requested by model which alters files and requires manual confirmation from user";
    let lines = format_security_reason_lines(desc, 20, &theme);
    assert_eq!(lines.len(), 2);
    let line2_str = lines[1].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
    assert!(line2_str.ends_with("...."));
}

#[test]
fn test_format_security_reason_lines_strips_resource_policy_prefix() {
    let theme = crate::channels::ratatui::theme::Theme::aura_dark();
    let desc = "resource_policy_reason: \"Disk quota threshold reached: 500MB free\", arguments: {\"CommandLine\":\"cargo clean\"}";
    let lines = format_security_reason_lines(desc, 60, &theme);
    assert_eq!(lines.len(), 1);
    let line_str = lines[0].spans.iter().map(|s| s.content.as_ref()).collect::<String>();
    assert!(!line_str.contains("resource_policy_reason:"));
    assert!(!line_str.contains("arguments:"));
    assert!(line_str.contains("Disk quota threshold reached: 500MB free"));
}

