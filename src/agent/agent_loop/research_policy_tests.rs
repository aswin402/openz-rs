use super::*;

#[test]
fn local_operational_now_queries_are_not_live_research() {
    assert!(!has_live_research_intent("in which dir we are now"));
    assert!(!has_live_research_intent(
        "Where is orchestrate_workflow implemented in this repo?"
    ));
}

#[test]
fn current_external_queries_still_require_live_research() {
    assert!(has_live_research_intent(
        "What is the latest Rust stable version today?"
    ));
}

#[test]
fn default_research_runtime_policy_has_bounded_budgets() {
    let config = crate::config::schema::Config::default();
    let policy = ResearchRuntimePolicy::from_config(&config);

    assert_eq!(policy.budget.default_time_budget_secs, 120);
    assert_eq!(policy.budget.max_search_attempts, 2);
    assert_eq!(policy.budget.max_browser_fallbacks, 1);
    assert!(policy.budget.require_sources_for_current_claims);
    assert!(policy.budget.stop_on_captcha);
}

#[test]
fn research_failure_classification_stops_browser_fallback_for_terminal_browser_errors() {
    assert_eq!(
        classify_research_failure("captcha challenge detected"),
        ResearchFailureKind::Captcha
    );
    assert_eq!(
        classify_research_failure("Failed to start geckodriver on port 4444"),
        ResearchFailureKind::BrowserDependencyMissing
    );
    assert_eq!(
        classify_research_failure("gsd-browser error: receiver is gone"),
        ResearchFailureKind::BrowserSessionLost
    );
    assert!(classify_research_failure("HTTP 503 service unavailable").is_retryable());
    assert!(!classify_research_failure("captcha challenge detected").is_retryable());
}

#[test]
fn explicit_research_request_detection_catches_link_analysis() {
    assert!(is_explicit_research_request(
        "research about this https://github.com/tinyhumansai/openhuman and tell me about this"
    ));
    assert!(is_explicit_research_request(
        "please read this https://github.com/mem0ai/mem0"
    ));
    assert!(is_explicit_research_request(
        "check this https://example.com/path"
    ));
    assert!(asks_to_revalidate_saved_research("go and check again"));
    assert!(!is_explicit_research_request("what is openhuman"));
    assert!(!is_explicit_research_request("hey whats new"));
}

#[test]
fn current_year_is_dynamic() {
    let current_year = chrono::Local::now().year().to_string();
    assert!(is_current_or_latest_query(&format!(
        "best ai agents in {current_year}"
    )));
    assert!(!is_current_or_latest_query("best ai agents in 1999"));
}

#[test]
fn tool_argument_urls_do_not_override_stable_user_intent() {
    assert!(!should_force_live_research_lookup(
        "what is this",
        &serde_json::json!({ "url": "https://example.com/pricing" })
    ));
    assert!(should_force_live_research_lookup(
        "check again https://example.com/pricing",
        &serde_json::json!({ "url": "https://example.com/pricing" })
    ));
}
