use super::*;

#[tokio::test]
async fn test_obscura_browser_tool_metadata() -> Result<()> {
    let tool = ObscuraBrowserTool::new();
    assert_eq!(tool.name(), "obscura_browser");
    let params = tool.parameters();
    assert!(params.get("properties").is_some());
    Ok(())
}

#[test]
fn test_obscura_action_recognition() {
    let eval_actions = [
        "eval",
        "eval_js",
        "eval-js",
        "evaluate",
        "js",
        "script",
        "expr",
        "EVAL_JS",
        " Eval ",
    ];
    for act in eval_actions {
        assert!(
            is_obscura_eval_action(act),
            "action '{act}' should be recognized as eval"
        );
    }

    let render_actions = [
        "render",
        "RENDER",
        "view",
        "markdown",
        "html",
        "content",
        "read",
        "get",
        "",
    ];
    for act in render_actions {
        assert!(
            !is_obscura_eval_action(act),
            "action '{act}' should not be eval"
        );
    }
}

#[test]
fn test_obscura_timeout_and_aliases() {
    let args1 = json!({ "timeout": "25" });
    let timeout1 = args1
        .get("timeout")
        .or_else(|| args1.get("timeout_secs"))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
        })
        .unwrap_or(15);
    assert_eq!(timeout1, 25);

    let args2 = json!({ "timeout_secs": 42 });
    let timeout2 = args2
        .get("timeout")
        .or_else(|| args2.get("timeout_secs"))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
        })
        .unwrap_or(15);
    assert_eq!(timeout2, 42);
}

#[test]
fn test_extract_obscura_url_supports_schemeless_and_aliases() {
    assert_eq!(
        extract_obscura_url(&json!("example.com")).unwrap(),
        "https://example.com"
    );
    assert_eq!(
        extract_obscura_url(&json!({ "url": "docs.rs/tokio" })).unwrap(),
        "https://docs.rs/tokio"
    );
    assert_eq!(
        extract_obscura_url(&json!({ "href": "https://crates.io" })).unwrap(),
        "https://crates.io"
    );
    assert_eq!(
        extract_obscura_url(&json!({ "page": "my-page.org" })).unwrap(),
        "https://my-page.org"
    );
    assert_eq!(
        extract_obscura_url(&json!({ "endpoint": "https://api.github.com" })).unwrap(),
        "https://api.github.com"
    );
    assert_eq!(
        extract_obscura_url(&json!({ "website": "https://rust-lang.org" })).unwrap(),
        "https://rust-lang.org"
    );
    assert_eq!(
        extract_obscura_url(&json!({ "domain": "crates.io" })).unwrap(),
        "https://crates.io"
    );
    assert_eq!(
        extract_obscura_url(&json!({ "host": "github.com" })).unwrap(),
        "https://github.com"
    );
    assert!(extract_obscura_url(&json!({})).is_err());
    assert!(extract_obscura_url(&json!("")).is_err());
}

