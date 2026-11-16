use super::*;
use serde_json::json;

#[test]
fn resolve_action_and_url_defaults_to_navigate_when_url_present() {
    let (action, url) =
        resolve_browser_object_action_and_url(&json!({ "href": "https://rust-lang.org" }), "status");
    assert_eq!(action, "navigate");
    assert_eq!(url.as_deref(), Some("https://rust-lang.org"));
}

#[test]
fn resolve_action_and_url_falls_back_when_empty() {
    let (action, url) = resolve_browser_object_action_and_url(&json!({}), "status");
    assert_eq!(action, "status");
    assert!(url.is_none());
}
