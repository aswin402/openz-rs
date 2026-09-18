use super::*;

#[tokio::test]
async fn test_gsd_browser_struct() -> Result<()> {
    let tool = GsdBrowserTool;
    assert_eq!(tool.name(), "gsd_browser");
    Ok(())
}

#[test]
fn gsd_browser_missing_error_payload_is_structured() {
    let payload = crate::tools::browser_status::browser_preflight_error_value(
        "gsd-browser startup failed: not found",
        crate::tools::browser_status::BrowserHealth {
            chrome_cdp: crate::tools::browser_status::BrowserBackendStatus::Stopped,
            gsd_browser: crate::tools::browser_status::BrowserBackendStatus::Missing,
            geckodriver: crate::tools::browser_status::BrowserBackendStatus::Stopped,
        },
    );
    assert_eq!(
        payload["browser_preflight"]["health"]["gsd_browser"],
        "missing"
    );
}

#[test]
fn gsd_browser_schema_exposes_fill_aliases() {
    let tool = GsdBrowserTool;
    let params = tool.parameters();
    let props = params
        .get("properties")
        .and_then(|v| v.as_object())
        .expect("properties object");
    assert!(props.contains_key("text"));
    assert!(props.contains_key("value"));
    assert!(props.contains_key("query"));
}

#[test]
fn gsd_browser_description_marks_gui_as_last_resort() {
    let tool = GsdBrowserTool;
    let description = tool.description().to_lowercase();
    assert!(description.contains("last-resort"));
    assert!(description.contains("headless"));
}

#[test]
fn gsd_browser_detects_stale_receiver_errors() {
    assert!(is_gsd_browser_disconnected_error(
        "snapshot error: send failed because receiver is gone"
    ));
    assert!(is_gsd_browser_disconnected_error("browser disconnected"));
    assert!(is_gsd_browser_disconnected_error("Target closed"));
    assert!(!is_gsd_browser_disconnected_error("selector not found"));
}

#[test]
fn gsd_browser_disconnected_payload_is_machine_readable() {
    let payload = gsd_browser_disconnected_error_value("receiver is gone", true);
    assert_eq!(payload["error_kind"], "browser_disconnected");
    assert_eq!(payload["recovered"], true);
    assert_eq!(payload["retryable"], false);
    assert!(payload["next_step"]
        .as_str()
        .unwrap()
        .contains("inspect_browsers"));
}

#[test]
fn test_build_gsd_browser_command_aliases_and_action_normalization() {
    let bin = PathBuf::from("gsd-browser");

    // goto action + target_url alias
    let cmd1 = build_gsd_browser_command(&bin, &json!({
        "action": "goto",
        "target_url": "https://example.com"
    })).expect("valid navigate command");
    let debug1 = format!("{:?}", cmd1);
    assert!(debug1.contains("navigate"));
    assert!(debug1.contains("https://example.com"));

    // eval_js + expression alias
    let cmd2 = build_gsd_browser_command(&bin, &json!({
        "action": "eval_js",
        "expression": "document.title"
    })).expect("valid eval command");
    let debug2 = format!("{:?}", cmd2);
    assert!(debug2.contains("eval"));
    assert!(debug2.contains("document.title"));

    // click + refId alias
    let cmd3 = build_gsd_browser_command(&bin, &json!({
        "action": "click",
        "refId": "@v1:e2"
    })).expect("valid click command");
    let debug3 = format!("{:?}", cmd3);
    assert!(debug3.contains("click-ref"));
    assert!(debug3.contains("@v1:e2"));

    // screenshot + output_path alias
    let cmd4 = build_gsd_browser_command(&bin, &json!({
        "action": "screenshot",
        "output_path": "/tmp/shot.png"
    })).expect("valid screenshot command");
    let debug4 = format!("{:?}", cmd4);
    assert!(debug4.contains("screenshot"));
    assert!(debug4.contains("--output"));
}
