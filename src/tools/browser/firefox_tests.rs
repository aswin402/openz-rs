use super::*;

#[test]
fn geckodriver_missing_message_mentions_preflight_and_inspection() {
    let msg = geckodriver_missing_message();
    assert!(msg.contains("Browser preflight failed"));
    assert!(msg.contains("inspect_browsers"));
    assert!(msg.contains("Chrome CDP"));
}

#[test]
fn geckodriver_missing_preflight_error_is_structured() {
    let payload = geckodriver_missing_preflight_error();
    assert_eq!(
        payload["browser_preflight"]["health"]["geckodriver"],
        "missing"
    );
    assert!(payload["error"]
        .as_str()
        .expect("error string")
        .contains("Browser preflight failed"));
}

#[test]
fn firefox_mode_defaults_and_validates() {
    assert_eq!(
        requested_mode(&json!({})).expect("default mode"),
        "headless"
    );
    assert_eq!(
        requested_mode(&json!({"mode": "visible"})).expect("visible mode"),
        "visible"
    );
    assert_eq!(
        requested_mode(&json!({"mode": "attach"})).expect("attach mode"),
        "attach"
    );
    assert!(requested_mode(&json!({"mode": "unknown"})).is_err());
}

#[test]
fn firefox_attach_mode_uses_separate_webdriver_port() {
    assert_eq!(webdriver_port("headless"), 4444);
    assert_eq!(webdriver_port("visible"), 4444);
    assert_eq!(webdriver_port("attach"), 4445);
}

#[tokio::test]
async fn test_firefox_browser_tool_metadata() -> Result<()> {
    let tool = FirefoxBrowserTool::new();
    assert_eq!(tool.name(), "firefox_browser");
    let params = tool.parameters();
    assert!(params.get("properties").is_some());
    let modes = params["properties"]["mode"]["enum"]
        .as_array()
        .expect("mode enum");
    assert!(modes.iter().any(|mode| mode == "attach"));
    Ok(())
}
