use super::*;

#[test]
fn browser_preflight_recommends_native_webkit_when_ready_or_running() {
    let ready_health = BrowserHealth {
        native_webkit: BrowserBackendStatus::Ready,
        active_sessions: 0,
    };
    assert_eq!(
        recommended_browser_backend(&ready_health),
        Some(BrowserBackend::OpenzNativeWebKit)
    );
    assert!(ready_health.actionable_summary().contains("0 MB idle RAM"));

    let running_health = BrowserHealth {
        native_webkit: BrowserBackendStatus::Running,
        active_sessions: 2,
    };
    assert_eq!(
        recommended_browser_backend(&running_health),
        Some(BrowserBackend::OpenzNativeWebKit)
    );
}

#[test]
fn browser_preflight_reports_missing_native_runtime() {
    let health = BrowserHealth {
        native_webkit: BrowserBackendStatus::Missing,
        active_sessions: 0,
    };
    assert_eq!(recommended_browser_backend(&health), None);
    assert!(health
        .actionable_summary()
        .contains("Native WebKit2GTK runtime unavailable"));
}

#[test]
fn browser_preflight_error_payload_is_machine_readable() {
    let payload = browser_preflight_error_value(
        "display missing",
        BrowserHealth {
            native_webkit: BrowserBackendStatus::Missing,
            active_sessions: 0,
        },
    );
    assert_eq!(payload["error"], "display missing");
    assert_eq!(
        payload["browser_preflight"]["health"]["openz_webkit2gtk"],
        "missing"
    );
}

#[tokio::test]
async fn collect_browser_status_report_returns_structured_payload() -> Result<()> {
    let res = collect_browser_status_report().await?;
    assert_eq!(res["engine"], "openz_webkit2gtk");
    assert!(res.get("runtime_probe").is_some());
    assert!(res.get("browser_preflight").is_some());
    assert!(res.get("active_sessions").is_some());
    Ok(())
}
