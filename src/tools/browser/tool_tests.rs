use super::*;
use serde_json::json;

#[test]
fn native_browser_tool_metadata_and_schema() {
    let tool = NativeBrowserTool::new();
    assert_eq!(tool.name(), "browser");
    let schema = tool.parameters();
    assert!(schema["properties"].get("action").is_some());
    assert!(schema["properties"].get("url").is_some());
    assert!(schema["properties"].get("mode").is_some());
    assert!(schema["properties"].get("index").is_some());
    assert!(schema["properties"].get("fields").is_some());
}

#[test]
fn browser_action_resolution_supports_legacy_and_shorthand_forms() {
    assert_eq!(
        resolve_canonical_browser_action(&json!("https://example.com")),
        "render"
    );
    assert_eq!(resolve_canonical_browser_action(&json!({})), "status");
    assert_eq!(
        resolve_canonical_browser_action(&json!({ "url": "docs.rs/tokio" })),
        "navigate"
    );
    assert_eq!(
        resolve_canonical_browser_action(&json!({ "script": "document.title" })),
        "eval_js"
    );
    assert_eq!(
        resolve_canonical_browser_action(&json!({ "action": "snapshot" })),
        "ui_tree"
    );
    assert_eq!(
        resolve_canonical_browser_action(&json!({ "action": "fill", "text": "hello" })),
        "type"
    );
    assert_eq!(
        resolve_canonical_browser_action(&json!({ "action": "pdf" })),
        "save_pdf"
    );
    assert_eq!(
        resolve_canonical_browser_action(&json!({ "action": "inspect_browsers" })),
        "status"
    );
}

#[test]
fn browser_url_extraction_normalizes_schemeless_and_aliases() {
    assert_eq!(
        extract_browser_url(&json!("example.com")).as_deref(),
        Some("https://example.com")
    );
    assert_eq!(
        extract_browser_url(&json!({ "href": "https://crates.io" })).as_deref(),
        Some("https://crates.io")
    );
    assert_eq!(
        extract_browser_url(&json!({ "domain": "rust-lang.org" })).as_deref(),
        Some("https://rust-lang.org")
    );
    assert!(extract_browser_url(&json!({})).is_none());
}

#[test]
fn browser_ssrf_blocks_cloud_metadata_and_sensitive_files_while_allowing_localhost_dev() {
    assert!(validate_browser_url_target("http://169.254.169.254/latest/meta-data/", true).is_err());
    assert!(validate_browser_url_target("http://metadata.google.internal/", true).is_err());
    assert!(validate_browser_url_target("file:///etc/shadow", true).is_err());
    assert!(validate_browser_url_target("http://10.0.0.5:8080/", false).is_err());
    assert!(validate_browser_url_target("http://10.0.0.5:8080/", true).is_ok());
    assert!(validate_browser_url_target("http://localhost:3000", false).is_ok());
    assert!(validate_browser_url_target("http://127.0.0.1:5173/app", false).is_ok());
    assert!(validate_browser_url_target("https://docs.rs/tokio", false).is_ok());
}

#[tokio::test]
async fn native_browser_tool_live_headless_session_roundtrip() {
    let probe = crate::tools::browser::webkit_ffi::probe_webkit_runtime();
    let bin_present = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().and_then(|d| d.parent()).map(|t| t.join("openz")))
        .is_some_and(|b| b.exists());
    match probe.available && bin_present {
        false => return,
        true => {}
    }

    let tool = NativeBrowserTool::new();
    let sess = "rust_unit_e2e";

    let nav = tool
        .call(&json!({
            "action": "navigate",
            "session": sess,
            "mode": "headless",
            "html": "<!doctype html><html><head><title>Tool Roundtrip</title></head><body><h1>OpenZ Ready</h1><input type='password' value='super-secret-999' aria-label='Token'/><button onclick='document.querySelector(\"h1\").textContent=\"Dispatched!\"'>Trigger</button></body></html>"
        }))
        .await
        .expect("navigate html");
    assert_eq!(nav["status"], "success");

    let snap = tool
        .call(&json!({
            "action": "ui_tree",
            "session": sess
        }))
        .await
        .expect("ui_tree");
    let tree_str = snap["output"].as_str().unwrap_or("");
    assert!(!tree_str.contains("super-secret-999"));
    assert!(tree_str.contains("••••••••"));
    assert!(tree_str.contains("Trigger"));

    let clicked = tool
        .call(&json!({
            "action": "click",
            "session": sess,
            "text": "Trigger"
        }))
        .await
        .expect("click Trigger");
    assert_eq!(clicked["status"], "success");

    let rendered = tool
        .call(&json!({
            "action": "render",
            "session": sess,
            "format": "markdown"
        }))
        .await
        .expect("render markdown");
    let md = rendered["output"].as_str().unwrap_or("");
    assert!(md.contains("Dispatched!"));

    let closed = tool
        .call(&json!({
            "action": "close",
            "session": sess
        }))
        .await
        .expect("close session");
    assert_eq!(closed["status"], "success");
}
