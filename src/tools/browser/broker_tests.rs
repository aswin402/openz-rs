use super::*;

#[test]
fn browser_backend_priority_uses_openz_native_headless() {
    assert_eq!(
        browser_backend_priority(),
        [BrowserBackendChoice::OpenzNativeHeadless]
    );
    assert_eq!(
        BrowserBackendChoice::OpenzNativeHeadless.as_str(),
        "openz_native_headless"
    );
}

#[test]
fn native_broker_cleanup_label_reflects_ephemeral_worker() {
    assert_eq!(
        cleanup_label(BrowserBackendChoice::OpenzNativeHeadless),
        "closed_ephemeral_worker"
    );
}

#[test]
fn broker_result_records_backend_and_cleanup() {
    let result = BrowserBrokerResult {
        backend: BrowserBackendChoice::OpenzNativeHeadless,
        status: "success".to_string(),
        output: "content".to_string(),
        cleanup: "closed_ephemeral_worker".to_string(),
        fallbacks_tried: vec![BrowserBackendChoice::OpenzNativeHeadless],
        errors: Vec::new(),
    };

    assert_eq!(result.backend, BrowserBackendChoice::OpenzNativeHeadless);
    assert_eq!(result.cleanup, "closed_ephemeral_worker");
}
