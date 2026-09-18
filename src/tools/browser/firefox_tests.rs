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

#[test]
fn test_firefox_action_normalization_and_timeout_parsing() {
    let actions = ["goto", "navigate", "open", "NAVIGATE"];
    for act in actions {
        let norm = act.trim().to_lowercase().replace('-', "_");
        assert!(matches!(norm.as_str(), "navigate" | "goto" | "open"));
    }

    let eval_actions = ["eval", "eval_js", "eval-js", "evaluate", "js"];
    for act in eval_actions {
        let norm = act.trim().to_lowercase().replace('-', "_");
        assert!(matches!(norm.as_str(), "eval" | "eval_js" | "evaluate" | "js"));
    }

    let args = json!({ "timeoutSecs": "30" });
    let timeout = args.get("timeout_secs").or_else(|| args.get("timeout")).or_else(|| args.get("timeoutSecs")).and_then(|v| {
        v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
    }).unwrap_or(10);
    assert_eq!(timeout, 30);
}
