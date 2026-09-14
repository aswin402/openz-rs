use super::*;

#[test]
fn browser_backend_priority_prefers_obscura_then_firefox_then_gsd() {
    assert_eq!(
        browser_backend_priority(),
        [
            BrowserBackendChoice::ObscuraHeadless,
            BrowserBackendChoice::FirefoxHeadless,
            BrowserBackendChoice::GsdChromeGui,
        ]
    );
}

#[test]
fn gsd_fallback_cleanup_stops_daemon() {
    assert_eq!(
        cleanup_label(BrowserBackendChoice::GsdChromeGui),
        "stopped_daemon"
    );
}

#[test]
fn broker_result_records_backend_and_cleanup() {
    let result = BrowserBrokerResult {
        backend: BrowserBackendChoice::ObscuraHeadless,
        status: "success".to_string(),
        output: "content".to_string(),
        cleanup: "closed_tab".to_string(),
        fallbacks_tried: vec![BrowserBackendChoice::ObscuraHeadless],
        errors: Vec::new(),
    };

    assert_eq!(result.backend, BrowserBackendChoice::ObscuraHeadless);
    assert_eq!(result.cleanup, "closed_tab");
}
