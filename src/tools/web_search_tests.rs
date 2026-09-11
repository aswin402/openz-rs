use super::*;

#[tokio::test]
async fn test_web_search() -> Result<()> {
    let tool = WebSearchTool::new();
    assert_eq!(tool.name(), "web_search");
    Ok(())
}

#[test]
fn web_search_policy_defaults_to_native_only() {
    std::env::remove_var("OPENZ_WEB_SEARCH_POLICY");
    assert_eq!(WebSearchPolicy::parse(None), WebSearchPolicy::NativeOnly);
}

#[test]
fn default_native_policy_uses_provider_free_browser_fallback() {
    std::env::remove_var("OPENZ_WEB_SEARCH_POLICY");
    let policy = WebSearchPolicy::parse(None);
    assert!(policy.allows_native());
    assert!(policy.allows_browser());
    assert!(!policy.allows_external());
}

#[test]
fn web_search_policy_accepts_native_then_browser() {
    assert_eq!(
        WebSearchPolicy::parse(Some("native_then_browser")),
        WebSearchPolicy::NativeThenBrowser
    );
    assert!(WebSearchPolicy::NativeThenBrowser.allows_native());
    assert!(WebSearchPolicy::NativeThenBrowser.allows_browser());
    assert!(!WebSearchPolicy::NativeThenBrowser.allows_external());
}

#[test]
fn web_search_schema_exposes_browser_fallback_policy() {
    let tool = WebSearchTool::new();
    let params = tool.parameters();
    let policy_enum = params["properties"]["search_policy"]["enum"]
        .as_array()
        .expect("policy enum");
    assert!(policy_enum.iter().any(|v| v == "native_then_browser"));
}

#[test]
fn web_search_auto_reads_research_style_queries() {
    assert!(web_search_should_auto_read_top_results(
        "research ai agent marketplaces",
        &json!({})
    ));
    assert!(web_search_should_auto_read_top_results(
        "latest rust async runtime docs",
        &json!({})
    ));
    assert!(web_search_should_auto_read_top_results(
        "compare crawlee crawl4ai scrapling",
        &json!({})
    ));
    assert!(!web_search_should_auto_read_top_results(
        "rust homepage",
        &json!({})
    ));
}

#[test]
fn web_search_read_controls_allow_explicit_override() {
    assert!(!web_search_should_auto_read_top_results(
        "research ai agent marketplaces",
        &json!({ "read_top_results": false })
    ));
    assert!(web_search_should_auto_read_top_results(
        "rust homepage",
        &json!({ "read_top_results": true })
    ));
    assert_eq!(web_search_auto_read_max_pages(&json!({}), true), 3);
    assert_eq!(
        web_search_auto_read_max_pages(&json!({ "max_pages": 99 }), true),
        5
    );
    assert_eq!(web_search_auto_read_max_pages(&json!({}), false), 0);
}

#[test]
fn web_search_failure_diagnostics_default_on_with_explicit_opt_out() {
    assert!(web_search_should_diagnose_on_failure(&json!({})));
    assert!(web_search_should_diagnose_on_failure(
        &json!({ "diagnose_on_failure": true })
    ));
    assert!(!web_search_should_diagnose_on_failure(
        &json!({ "diagnose_on_failure": false })
    ));
}

#[test]
fn web_search_archive_text_includes_browser_read_results() {
    let archive = web_search_archive_text(&json!({
        "results": [{
            "title": "Example",
            "url": "https://example.com",
            "snippet": "result snippet"
        }],
        "read_results": [{
            "status": "success",
            "url": "https://example.com",
            "content": { "content": "full page text from browser-discovered result" }
        }]
    }))
    .expect("archive text");

    assert!(archive.contains("result snippet"));
    assert!(archive.contains("full page text from browser-discovered result"));
}

#[test]
fn web_search_browser_fallback_engines_include_bing_retry() {
    assert_eq!(browser_fallback_engines(), ["duckduckgo", "bing"]);
}

#[test]
fn web_search_policy_accepts_external_fallback_alias() {
    assert_eq!(
        WebSearchPolicy::parse(Some("native_then_external")),
        WebSearchPolicy::NativeThenExternal
    );
    assert_eq!(
        WebSearchPolicy::parse(Some("fallback")),
        WebSearchPolicy::NativeThenExternal
    );
    assert_eq!(
        WebSearchPolicy::parse(Some("external_only")),
        WebSearchPolicy::ExternalOnly
    );
}

#[test]
fn web_search_schema_exposes_diagnose_on_failure() {
    let schema = WebSearchTool::new().parameters();
    assert!(schema["properties"].get("diagnose_on_failure").is_some());
}

#[test]
fn native_only_failure_mentions_doctor_and_external_policy() {
    let message = format_native_only_failure(Some("all backends exhausted"));
    assert!(message.contains("searchxyz_doctor"));
    assert!(message.contains("SEARCHXYZ_SEARXNG_URL"));
    assert!(message.contains("diagnose_on_failure=false"));
    assert!(message.contains("search_policy=native_then_external"));
    assert!(message.contains("all backends exhausted"));
}

#[test]
fn weak_native_results_trigger_merge_retry_for_multi_term_queries() {
    let results = vec![
        json!({"title":"Google Gemini","url":"https://gemini.google.com/app","snippet":"Google AI chat app"}),
        json!({"title":"Gemini Help","url":"https://support.google.com/gemini","snippet":"Help center"}),
        json!({"title":"Google AI Studio","url":"https://aistudio.google.com","snippet":"Build with Gemini"}),
    ];

    assert!(native_search_results_need_merge_retry(
        "sakana fugu pricing",
        &results
    ));
}

#[test]
fn relevant_native_results_skip_merge_retry() {
    let results = vec![
        json!({"title":"Fugu Pricing - Sakana AI","url":"https://sakana.ai/fugu/pricing","snippet":"Pricing for Fugu subscriptions and usage tiers"}),
        json!({"title":"Fugu Docs","url":"https://sakana.ai/fugu/docs","snippet":"Sakana Fugu API docs"}),
        json!({"title":"Sakana AI","url":"https://sakana.ai","snippet":"Fugu is a multi-agent model"}),
    ];

    assert!(!native_search_results_need_merge_retry(
        "sakana fugu pricing",
        &results
    ));
}

#[test]
fn native_rescue_returns_docs_for_tokio_rust_query() {
    let results = native_rescue_results("rust tokio async runtime");
    assert!(results.iter().any(|r| r["url"] == "https://docs.rs/tokio"));
    assert!(results
        .iter()
        .any(|r| r["url"] == "https://crates.io/crates/tokio"));
}

#[test]
fn native_rescue_ignores_unknown_rust_query() {
    let results = native_rescue_results("rust totallyunknowncrate async runtime");
    assert!(results.is_empty());
}
