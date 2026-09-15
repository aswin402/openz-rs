use super::{
    list_registered_children, register_child_group_with_metadata, sigint_action,
    stop_registered_child, RegisteredChildInfo, SigintAction,
};

#[test]
fn test_sigint_decision_cancels_only_active_turns() {
    assert_eq!(sigint_action(true, false), SigintAction::CancelTurn);
    assert_eq!(sigint_action(true, true), SigintAction::Shutdown);
    assert_eq!(sigint_action(false, false), SigintAction::Shutdown);
}

#[test]
fn registered_children_can_be_listed_and_stopped() {
    let _ = stop_registered_child("all");
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "sleep 30"]);
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let child = command.spawn().expect("spawn sleep child");
    let id = register_child_group_with_metadata(child, "sleep 30", "dev_server");
    let active = list_registered_children();
    assert!(active.iter().any(|p: &RegisteredChildInfo| p.id == id));
    assert_eq!(stop_registered_child(&id.to_string()).unwrap(), 1);
    assert!(!list_registered_children().iter().any(|p| p.id == id));
}
