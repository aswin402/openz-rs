use super::*;

#[test]
fn test_browser_cdp_port_default() {
    let port = browser_cdp_port();
    assert!(port > 0);
}

#[test]
fn test_kill_browser_on_port_non_existent() {
    // Port 59999 should safely execute without error or panics
    kill_browser_on_port(59999);
}
