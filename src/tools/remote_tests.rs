use super::is_self_target;

#[test]
fn rejects_current_cli_and_direct_alias_targets() {
    assert!(is_self_target(Some("cli:abc"), "cli:abc"));
    assert!(is_self_target(Some("cli:abc"), "cli:direct"));
    assert!(!is_self_target(Some("telegram:123"), "cli:direct"));
}
