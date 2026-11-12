use super::is_self_target;

#[test]
fn rejects_current_cli_and_direct_alias_targets() {
    assert!(is_self_target(Some("cli:abc"), "cli:abc"));
    assert!(is_self_target(Some("cli:abc"), "cli:direct"));
    assert!(!is_self_target(Some("telegram:123"), "cli:direct"));
}

#[test]
fn rejects_current_ws_and_allows_cross_surface_targets() {
    assert!(is_self_target(Some("ws:chat-1"), "ws:chat-1"));
    assert!(is_self_target(Some("ws:chat-1"), "ws:direct"));
    assert!(!is_self_target(Some("cli:abc"), "ws:direct"));
    assert!(!is_self_target(Some("cli:abc"), "ws:new"));
    assert!(!is_self_target(Some("ws:chat-1"), "cli:direct"));
    assert!(!is_self_target(Some("telegram:123"), "ws:new"));
}
