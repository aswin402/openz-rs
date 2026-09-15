#[test]
fn email_uses_shared_stop_command_detection() {
    assert!(crate::channels::is_stop_command("/stop"));
    assert!(!crate::channels::is_stop_command("subject says stop"));
}
