use super::*;

#[test]
fn live_claim_confidence_requires_two_successful_sources() {
    let mut ledger = SourceLedger::default();
    assert_eq!(
        ledger.confidence_for_live_claims(),
        SourceConfidence::Missing
    );

    ledger.add_success("https://example.com/a", Some("A"));
    assert_eq!(ledger.confidence_for_live_claims(), SourceConfidence::Weak);

    ledger.add_success("https://example.com/b", Some("B"));
    assert_eq!(
        ledger.confidence_for_live_claims(),
        SourceConfidence::Sufficient
    );
}

#[test]
fn failed_attempts_do_not_count_as_successful_sources() {
    let mut ledger = SourceLedger::default();
    ledger.add_failure("https://example.com/a", "HTTP 403");
    ledger.add_failure("https://example.com/b", "CAPTCHA");

    assert_eq!(ledger.successful_sources().count(), 0);
    assert_eq!(ledger.failed_sources().count(), 2);
    assert_eq!(
        ledger.confidence_for_live_claims(),
        SourceConfidence::Missing
    );
}

#[test]
fn source_urls_are_deduplicated_by_normalized_url() {
    let mut ledger = SourceLedger::default();
    ledger.add_success("https://example.com/a#section", Some("A"));
    ledger.add_success("https://example.com/a", Some("A duplicate"));

    assert_eq!(ledger.successful_sources().count(), 1);
}

#[test]
fn records_successful_urls_from_nested_tool_result() {
    let mut ledger = SourceLedger::default();
    let result = serde_json::json!({
        "results": [
            { "title": "A", "url": "https://example.com/a" },
            { "title": "B", "source_url": "https://example.com/b" }
        ],
        "page_attempts": [
            { "url": "https://example.com/c", "status": "error", "detail": "HTTP 403" }
        ]
    });

    ledger.record_tool_result("searchxyz_search_and_read", &serde_json::json!({}), &result);

    assert_eq!(ledger.successful_sources().count(), 2);
    assert_eq!(ledger.failed_sources().count(), 1);
    assert_eq!(
        ledger.confidence_for_live_claims(),
        SourceConfidence::Sufficient
    );
}

#[test]
fn appends_live_research_caveat_when_sources_are_missing_or_weak() {
    let missing = SourceLedger::default();
    let answer = missing.append_live_research_caveat_if_needed("answer".to_string(), true);
    assert!(answer.contains("Live source verification is incomplete"));

    let mut enough = SourceLedger::default();
    enough.add_success("https://example.com/a", Some("A"));
    enough.add_success("https://example.com/b", Some("B"));
    assert_eq!(
        enough.append_live_research_caveat_if_needed("answer".to_string(), true),
        "answer"
    );
}

#[test]
fn records_successful_source_url_from_tool_arguments() {
    let mut ledger = SourceLedger::default();
    ledger.record_tool_result(
        "obscura_browser",
        &serde_json::json!({ "url": "https://example.com/live" }),
        &serde_json::json!({ "status": "success", "output": "rendered page" }),
    );

    assert_eq!(ledger.successful_sources().count(), 1);
}
