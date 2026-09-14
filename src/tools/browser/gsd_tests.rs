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
