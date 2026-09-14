use super::*;

#[test]
fn browser_preflight_prefers_running_chrome_cdp() {
    let health = BrowserHealth {
        chrome_cdp: BrowserBackendStatus::Running,
        gsd_browser: BrowserBackendStatus::Stopped,
        geckodriver: BrowserBackendStatus::Missing,
    };

    assert_eq!(
        recommended_browser_backend(&health),
        Some(BrowserBackend::ChromeCdp)
    );
}

#[test]
fn browser_preflight_reports_missing_all_backends() {
    let health = BrowserHealth {
        chrome_cdp: BrowserBackendStatus::Missing,
        gsd_browser: BrowserBackendStatus::Missing,
        geckodriver: BrowserBackendStatus::Missing,
    };

    assert_eq!(recommended_browser_backend(&health), None);
    assert!(health
        .actionable_summary()
        .contains("No browser backend available"));
}

#[test]
fn browser_preflight_error_payload_is_machine_readable() {
    let payload = browser_preflight_error_value(
        "geckodriver missing",
        BrowserHealth {
            chrome_cdp: BrowserBackendStatus::Stopped,
            gsd_browser: BrowserBackendStatus::Stopped,
            geckodriver: BrowserBackendStatus::Missing,
        },
    );

    assert_eq!(payload["error"], "geckodriver missing");
    assert_eq!(
        payload["browser_preflight"]["health"]["geckodriver"],
        "missing"
    );
    assert!(payload["browser_preflight"]["summary"]
        .as_str()
        .expect("summary string")
        .contains("No browser backend available"));
}

#[tokio::test]
async fn test_inspect_browsers_metadata() -> Result<()> {
    let tool = InspectBrowsersTool;
    assert_eq!(tool.name(), "inspect_browsers");
    assert_eq!(tool.metadata().domain, "browser");
    Ok(())
}

#[tokio::test]
async fn test_inspect_browsers_execution() -> Result<()> {
    let tool = InspectBrowsersTool;
    let res = tool.call(&serde_json::json!({})).await?;
    assert!(res.get("firefox_geckodriver").is_some());
    assert!(res.get("chrome_obscura").is_some());
    assert!(res.get("gsd_browser").is_some());
    assert!(res.get("browser_preflight").is_some());
    assert!(res.get("recent_browser_errors").is_some());
    Ok(())
}
